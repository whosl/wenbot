//! EIP-712 typed data signing for Polymarket CLOB orders
//!
//! Implements the EIP-712 standard for signing structured data.
//! Verified against py_order_utils / poly_eip712_structs Python SDK.
//!
//! Key encoding rules:
//! - All uint256 fields: 32 bytes, big-endian, left-padded with zeros
//! - Address fields: 20 bytes, left-padded to 32 bytes with zeros
//! - uint8 fields: 1 byte, left-padded to 32 bytes with zeros
//! - Domain string fields (name, version): keccak256 hash of the UTF-8 string, then padded to 32 bytes
//! - Struct hash = keccak256(type_hash || encode_value) where encode_value is the ABI encoding of all fields
//! - Signable = b"\x19\x01" || domain_separator || struct_hash

use k256::{ecdsa::SigningKey, SecretKey as K256SecretKey};
use sha3::{Keccak256, Digest};
use num_bigint::BigUint;

/// Polymarket CTF Exchange contract on Polygon mainnet (standard)
const EXCHANGE_ADDRESS: &str = "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E";

/// Neg-risk exchange contract on Polygon mainnet
const NEG_RISK_EXCHANGE_ADDRESS: &str = "0xC5d563A36AE78145C45a50134d48A1215220f80a";

const CHAIN_ID: u64 = 137;

/// Signature type constants (matching py_order_utils)
pub const SIG_EOA: u8 = 0;
pub const SIG_POLY_PROXY: u8 = 1;
pub const SIG_POLY_GNOSIS_SAFE: u8 = 2;

/// Order type hash for EIP-712
fn order_type_hash() -> [u8; 32] {
    keccak256(b"Order(uint256 salt,address maker,address signer,address taker,uint256 tokenId,uint256 makerAmount,uint256 takerAmount,uint256 expiration,uint256 nonce,uint256 feeRateBps,uint8 side,uint8 signatureType)")
}

/// EIP-712 Domain Separator (standard exchange)
fn domain_separator(neg_risk: bool) -> [u8; 32] {
    domain_separator_for_exchange(if neg_risk { NEG_RISK_EXCHANGE_ADDRESS } else { EXCHANGE_ADDRESS })
}

fn domain_separator_for_exchange(exchange: &str) -> [u8; 32] {
    let type_hash = keccak256(b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)");
    let name_hash = keccak256(b"Polymarket CTF Exchange");
    let version_hash = keccak256(b"1");
    let chain_id_bytes = chain_id_to_bytes(CHAIN_ID);
    let contract_bytes = address_to_32bytes(exchange);

    // Domain data: each string field is its keccak256 hash (32 bytes), chainId and address are ABI encoded
    let domain_data = [name_hash, version_hash, chain_id_bytes, contract_bytes].concat();

    // Domain separator = keccak256(type_hash || domain_data)
    let mut hasher = Keccak256::new();
    hasher.update(type_hash);
    hasher.update(&domain_data);
    hasher.finalize().into()
}

/// Build the EIP-712 struct hash for an order
///
/// struct_hash = keccak256(type_hash || field_encodings)
/// where field_encodings is the ABI encoding of all 12 fields, each 32 bytes
fn order_struct_hash(fields: &OrderFields) -> [u8; 32] {
    let type_hash = order_type_hash();

    // ABI encode all fields in order (each 32 bytes, big-endian left-padded)
    let enc = [
        biguint_to_32bytes(&fields.salt),
        address_to_32bytes(&fields.maker),
        address_to_32bytes(&fields.signer),
        address_to_32bytes(&fields.taker),
        biguint_to_32bytes(&fields.token_id),
        biguint_to_32bytes(&fields.maker_amount),
        biguint_to_32bytes(&fields.taker_amount),
        biguint_to_32bytes(&fields.expiration),
        biguint_to_32bytes(&fields.nonce),
        biguint_to_32bytes(&fields.fee_rate_bps),
        uint8_to_32bytes(fields.side),
        uint8_to_32bytes(fields.signature_type),
    ]
    .concat();

    let mut hasher = Keccak256::new();
    hasher.update(type_hash);
    hasher.update(&enc);
    hasher.finalize().into()
}

/// EIP-712 signing hash: keccak256("\x19\x01" || domain_separator || struct_hash)
fn eip712_hash(fields: &OrderFields, neg_risk: bool) -> [u8; 32] {
    let domain = domain_separator(neg_risk);
    let struct_hash = order_struct_hash(fields);

    let mut hasher = Keccak256::new();
    hasher.update(b"\x19\x01");
    hasher.update(domain);
    hasher.update(struct_hash);
    let hash: [u8; 32] = hasher.finalize().into();
    tracing::info!("🔐 EIP-712 hash: 0x{}", hex::encode(hash));
    tracing::info!("🔐 domain_sep: 0x{}", hex::encode(domain));
    tracing::info!("🔐 struct_hash: 0x{}", hex::encode(struct_hash));
    hash
}

/// Sign an EIP-712 hash with a private key (using k256 secp256k1)
fn sign_hash(hash: &[u8; 32], private_key_bytes: &[u8]) -> String {
    let secret_key = K256SecretKey::from_slice(private_key_bytes)
        .expect("valid secp256k1 private key");
    let signing_key = SigningKey::from(secret_key);
    let (signature, recover_id) = signing_key.sign_prehash_recoverable(hash)
        .expect("signing failed");

    let sig_bytes = signature.to_bytes();
    let v = recover_id.to_byte() + 27;

    // Concatenate r (32) + s (32) + v (1) = 65 bytes
    let mut full_sig = [0u8; 65];
    full_sig[0..32].copy_from_slice(&sig_bytes[0..32]);
    full_sig[32..64].copy_from_slice(&sig_bytes[32..64]);
    full_sig[64] = v;

    format!("0x{}", hex::encode(full_sig))
}

/// Generate a random salt (matching Python SDK: int(time.time() * random.random()))
/// Python SDK uses a ~32-bit integer (timestamp * random), so we match that range.
pub fn generate_salt() -> BigUint {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    BigUint::from(rng.gen::<u32>())
}

/// Order fields for signing
pub struct OrderFields {
    pub salt: BigUint,
    /// Maker address — for proxy wallets, this should be the PROXY wallet address (funder)
    pub maker: String,
    /// Signer address — the EOA that owns the proxy wallet (MetaMask address)
    pub signer: String,
    /// Taker address — zero address for public orders
    pub taker: String,
    pub token_id: BigUint,
    /// Maker amount in token decimals (USDC * 10^6)
    pub maker_amount: BigUint,
    /// Taker amount in token decimals (shares * 10^6)
    pub taker_amount: BigUint,
    pub expiration: BigUint,
    pub nonce: BigUint,
    pub fee_rate_bps: BigUint,
    /// 0 = BUY, 1 = SELL
    pub side: u8,
    /// 0 = EOA, 1 = POLY_PROXY, 2 = POLY_GNOSIS_SAFE
    pub signature_type: u8,
}

/// Build a complete signed order payload for Polymarket CLOB API
///
/// `neg_risk` should be true for neg-risk markets (uses different exchange contract)
pub fn build_signed_order(fields: &OrderFields, private_key: &str, neg_risk: bool) -> crate::types::SignedOrder {
    let hash = eip712_hash(fields, neg_risk);
    let pk_bytes = hex::decode(private_key.strip_prefix("0x").unwrap_or(private_key))
        .expect("valid hex private key");
    let signature = sign_hash(&hash, &pk_bytes);

    let side_str = match fields.side {
        0 => crate::types::Side::Buy,
        1 => crate::types::Side::Sell,
        _ => crate::types::Side::Buy,
    };

    crate::types::SignedOrder {
        salt: fields.salt.clone(),
        maker: checksum_address(&fields.maker),
        signer: checksum_address(&fields.signer),
        taker: checksum_address(&fields.taker),
        token_id: fields.token_id.to_string(),
        maker_amount: fields.maker_amount.to_string(),
        taker_amount: fields.taker_amount.to_string(),
        expiration: fields.expiration.to_string(),
        nonce: fields.nonce.to_string(),
        fee_rate_bps: fields.fee_rate_bps.to_string(),
        side: side_str,
        signature_type: fields.signature_type,
        signature,
    }
}

/// Normalize an address to EIP-55 checksum format
/// Polymarket requires EIP-55 mixed-case addresses in the signed order payload.
/// The server recovers the signer address from the EIP-712 signature and compares
/// it as a string with the "signer" and "maker" fields — lowercase won't match.
fn checksum_address(addr: &str) -> String {
    let hex_str = addr.strip_prefix("0x").unwrap_or(addr).to_lowercase();
    let hash = keccak256(hex_str.as_bytes());
    let mut result = String::with_capacity(42);
    result.push_str("0x");
    for (i, c) in hex_str.chars().enumerate() {
        let byte = hash[i / 2];
        let nibble = if i % 2 == 0 { byte >> 4 } else { byte & 0x0f };
        if c.is_ascii_hexdigit() && (c.is_ascii_uppercase() || (nibble > 7)) {
            result.push(c.to_ascii_uppercase());
        } else {
            result.push(c);
        }
    }
    result
}

/// Keccak256 hash
fn keccak256(data: &[u8]) -> [u8; 32] {
    Keccak256::digest(data).into()
}

/// Encode address to 32 bytes (20 bytes left-padded with zeros)
fn address_to_32bytes(addr: &str) -> [u8; 32] {
    let hex_str = addr.strip_prefix("0x").unwrap_or(addr).to_lowercase();
    let addr_bytes = hex::decode(format!("{:0>40}", hex_str))
        .expect("valid address hex");
    let mut bytes = [0u8; 32];
    bytes[12..32].copy_from_slice(&addr_bytes);
    bytes
}

/// Encode BigUint to 32 big-endian bytes (left-padded with zeros)
fn biguint_to_32bytes(val: &BigUint) -> [u8; 32] {
    let be = val.to_bytes_be();
    let mut bytes = [0u8; 32];
    let offset = 32 - be.len().min(32);
    bytes[offset..].copy_from_slice(&be[be.len().saturating_sub(32 - offset)..]);
    bytes
}

/// Encode uint8 to 32 bytes (left-padded with zeros)
fn uint8_to_32bytes(val: u8) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes[31] = val;
    bytes
}

/// Encode chain_id as 32-byte big-endian (left-padded)
fn chain_id_to_bytes(chain_id: u64) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes[24..32].copy_from_slice(&chain_id.to_be_bytes());
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_separator() {
        let ds = domain_separator(false);
        assert_ne!(ds, [0u8; 32]);
        // Should match Python SDK: 0x1a573e3617c78403b5b4b892827992f027b03d4eaf570048b8ee8cdd84d151be
        let expected = "1a573e3617c78403b5b4b892827992f027b03d4eaf570048b8ee8cdd84d151be";
        assert_eq!(hex::encode(ds), expected, "Domain separator mismatch with Python SDK!");
    }

    #[test]
    fn test_order_struct_hash() {
        let fields = OrderFields {
            salt: BigUint::from(123456789u64),
            maker: "0x3A56ce8622ae9E4626ec7D18f3e8B92Bd63E7F15".to_string(),
            signer: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".to_string(),
            taker: "0x0000000000000000000000000000000000000000".to_string(),
            token_id: BigUint::from(123456789012345678901234567890u128),
            maker_amount: BigUint::from(500000u64),
            taker_amount: BigUint::from(1000000u64),
            expiration: BigUint::from(0u64),
            nonce: BigUint::from(0u64),
            fee_rate_bps: BigUint::from(0u64),
            side: 0,
            signature_type: 1,
        };
        let hash = order_struct_hash(&fields);
        // Should match Python SDK: 0x86a1f7a3ecd6f41238e09b9c6589a5acbbf681fc31bd10e5e4a9ebcdb7f1a766
        let expected = "86a1f7a3ecd6f41238e09b9c6589a5acbbf681fc31bd10e5e4a9ebcdb7f1a766";
        assert_eq!(hex::encode(hash), expected, "Struct hash mismatch with Python SDK!");
    }

    #[test]
    fn test_signing_produces_valid_signature() {
        let test_pk = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let fields = OrderFields {
            salt: BigUint::from(123456789u64),
            maker: "0x3A56ce8622ae9E4626ec7D18f3e8B92Bd63E7F15".to_string(),
            signer: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".to_string(),
            taker: "0x0000000000000000000000000000000000000000".to_string(),
            token_id: BigUint::from(123456789012345678901234567890u128),
            maker_amount: BigUint::from(500000u64),
            taker_amount: BigUint::from(1000000u64),
            expiration: BigUint::from(0u64),
            nonce: BigUint::from(0u64),
            fee_rate_bps: BigUint::from(0u64),
            side: 0,
            signature_type: 1,
        };
        let hash = eip712_hash(&fields, false);
        let pk_bytes = hex::decode(test_pk.strip_prefix("0x").unwrap()).unwrap();
        let sig = sign_hash(&hash, &pk_bytes);
        // Signature should be 65 bytes (0x prefix + 130 hex chars)
        assert_eq!(sig.len(), 132); // "0x" + 130 hex = 132
        assert!(sig.starts_with("0x"));
    }

    #[test]
    fn test_address_encoding() {
        let addr = "0x3A56ce8622ae9E4626ec7D18f3e8B92Bd63E7F15";
        let bytes = address_to_32bytes(addr);
        // First 12 bytes should be zero (padding)
        assert!(bytes[0..12].iter().all(|&b| b == 0));
        // Last 20 bytes should be the address
        let expected_suffix = hex::decode("3a56ce8622ae9e4626ec7d18f3e8b92bd63e7f15").unwrap();
        assert_eq!(&bytes[12..32], expected_suffix.as_slice());
    }

    #[test]
    fn test_generate_salt() {
        let salt = generate_salt();
        // Salt should be a ~32-bit integer (matching Python SDK)
        assert!(salt < BigUint::from(u32::MAX as u64) + 1u64);
    }
}
