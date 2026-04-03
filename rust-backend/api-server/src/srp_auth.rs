use crate::crypto::{decrypt_credentials, derive_file_key_hex, encrypt_credentials};
use crate::{AppState, SharedState};
use anyhow::{anyhow, Context, Result};
use axum::{extract::State, http::StatusCode, response::Json};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use chrono::Utc;
use num_bigint::BigUint;
use num_traits::{Num, Zero};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::sync::Arc;
use tracing::{error, info, warn};

const USERNAME: &str = "polymarket";
const SRP_N_HEX: &str = concat!(
    "FFFFFFFFFFFFFFFFC90FDAA22168C234C4C6628B80DC1CD129024E088A67CC74",
    "020BBEA63B139B22514A08798E3404DDEF9519B3CD3A431B302B0A6DF25F143",
    "74FE1356D6D51C245E485B576625E7EC6F44C42E9A637ED6B0BFF5CB6F406B7",
    "EDEE386BFB5A899FA5AE9F24117C4B1FE649286651ECE45B3DC2007CB8A163B",
    "F0598DA48361C55D39A69163FA8FD24CF5F83655D23DCA3AD961C62F35620855",
    "2BB9ED529077096966D670C354E4ABC9804F1746C08CA18217C32905E462E36CE",
    "3BE39E772C180E86039B2783A2EC07A28FB5C55DF06F4C52C9DE2BCBF695581",
    "7183995497CEA956AE515D2261898FA051015728E5A8AACAA68FFFFFFFFFFFFFFFF"
);
// The standard RFC 5054 2048-bit prime above, but concat formatting can be hard to eyeball.
// Use the exact TS copy as well; both sides must match.

const SRP_N_HEX_FULL: &str = "FFFFFFFFFFFFFFFFC90FDAA22168C234C4C6628B80DC1CD129024E088A67CC74020BBEA63B139B22514A08798E3404DDEF9519B3CD3A431B302B0A6DF25F14374FE1356D6D51C245E485B576625E7EC6F44C42E9A637ED6B0BFF5CB6F406B7EDEE386BFB5A899FA5AE9F24117C4B1FE649286651ECE45B3DC2007CB8A163BF0598DA48361C55D39A69163FA8FD24CF5F83655D23DCA3AD961C62F356208552BB9ED529077096966D670C354E4ABC9804F1746C08CA18217C32905E462E36CE3BE39E772C180E86039B2783A2EC07A28FB5C55DF06F4C52C9DE2BCBF6955817183995497CEA956AE515D2261898FA051015728E5A8AACAA68FFFFFFFFFFFFFFFF";
const SRP_G: u32 = 2;

#[derive(Debug, Clone)]
pub struct WalletConfigRow {
    pub srp_salt: String,
    pub srp_verifier: String,
    pub age_ciphertext: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct PendingSrpSession {
    pub salt_hex: String,
    pub verifier_hex: String,
    pub b_hex: String,
    pub b_pub_hex: String,
    pub created_at_ms: i64,
}

#[derive(Debug, Deserialize)]
pub struct ConnectStartRequest {
    pub username: String,
}

#[derive(Debug, Serialize)]
pub struct ConnectStartResponse {
    pub session_id: String,
    pub salt: String,
    #[serde(rename = "B")]
    pub b_pub: String,
    pub n_hex: String,
    pub g_hex: String,
}

#[derive(Debug, Deserialize)]
pub struct ConnectVerifyRequest {
    pub session_id: String,
    #[serde(rename = "A")]
    pub a_pub: String,
    #[serde(rename = "M1")]
    pub m1: String,
    pub wrapped_file_key: String,
    pub wrapped_file_key_iv: String,
}

#[derive(Debug, Serialize)]
pub struct ConnectVerifyResponse {
    #[serde(rename = "M2")]
    pub m2: String,
    pub connected: bool,
}

#[derive(Debug, Serialize)]
pub struct WalletConnectionStatusResponse {
    pub configured: bool,
    pub connected: bool,
}

pub async fn ensure_wallet_config_table(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS wallet_config (
            id INTEGER PRIMARY KEY,
            wallet_type TEXT NOT NULL DEFAULT 'polymarket',
            srp_salt TEXT NOT NULL,
            srp_verifier TEXT NOT NULL,
            age_ciphertext BLOB NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT
        )
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn upsert_wallet_config(
    pool: &SqlitePool,
    srp_salt: &str,
    srp_verifier: &str,
    age_ciphertext: &[u8],
) -> Result<()> {
    ensure_wallet_config_table(pool).await?;
    sqlx::query(
        r#"
        INSERT INTO wallet_config (id, wallet_type, srp_salt, srp_verifier, age_ciphertext, updated_at)
        VALUES (1, 'polymarket', ?1, ?2, ?3, datetime('now'))
        ON CONFLICT(id) DO UPDATE SET
            srp_salt = excluded.srp_salt,
            srp_verifier = excluded.srp_verifier,
            age_ciphertext = excluded.age_ciphertext,
            updated_at = datetime('now')
        "#,
    )
    .bind(srp_salt)
    .bind(srp_verifier)
    .bind(age_ciphertext)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn load_wallet_config(pool: &SqlitePool) -> Result<Option<WalletConfigRow>> {
    ensure_wallet_config_table(pool).await?;
    let row = sqlx::query(
        "SELECT srp_salt, srp_verifier, age_ciphertext FROM wallet_config WHERE wallet_type = 'polymarket' ORDER BY id DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| WalletConfigRow {
        srp_salt: r.get::<String, _>("srp_salt"),
        srp_verifier: r.get::<String, _>("srp_verifier"),
        age_ciphertext: r.get::<Vec<u8>, _>("age_ciphertext"),
    }))
}

pub async fn wallet_status(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    let configured = match load_wallet_config(&state.db_pool).await {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(e) => return internal_error(e),
    };

    let connected = state.polymarket_credentials.lock().await.is_some();
    (
        StatusCode::OK,
        Json(serde_json::to_value(WalletConnectionStatusResponse { configured, connected }).unwrap()),
    )
}

pub async fn connect_start(
    State(state): State<SharedState>,
    Json(req): Json<ConnectStartRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if req.username != USERNAME {
        return bad_request("unsupported username");
    }

    let Some(cfg) = (match load_wallet_config(&state.db_pool).await {
        Ok(v) => v,
        Err(e) => return internal_error(e),
    }) else {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Polymarket wallet is not configured yet"})));
    };

    let mut b = [0u8; 32];
    OsRng.fill_bytes(&mut b);
    let b_num = BigUint::from_bytes_be(&b);
    let v = parse_hex_biguint(&cfg.srp_verifier);
    let b_pub = compute_b_pub(&v, &b_num);
    let session_id = uuid::Uuid::new_v4().to_string();

    state.srp_sessions.lock().await.insert(
        session_id.clone(),
        PendingSrpSession {
            salt_hex: cfg.srp_salt.clone(),
            verifier_hex: cfg.srp_verifier,
            b_hex: hex::encode(b),
            b_pub_hex: to_hex(&b_pub),
            created_at_ms: Utc::now().timestamp_millis(),
        },
    );

    let response = ConnectStartResponse {
        session_id,
        salt: cfg.srp_salt,
        b_pub: to_hex(&b_pub),
        n_hex: SRP_N_HEX_FULL.to_string(),
        g_hex: format!("{:x}", SRP_G),
    };

    (StatusCode::OK, Json(serde_json::to_value(response).unwrap()))
}

pub async fn connect_verify(
    State(state): State<SharedState>,
    Json(req): Json<ConnectVerifyRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let pending = match state.srp_sessions.lock().await.remove(&req.session_id) {
        Some(v) => v,
        None => return bad_request("SRP session expired or not found"),
    };

    if Utc::now().timestamp_millis() - pending.created_at_ms > 5 * 60 * 1000 {
        return bad_request("SRP session expired");
    }

    let Some(cfg) = (match load_wallet_config(&state.db_pool).await {
        Ok(v) => v,
        Err(e) => return internal_error(e),
    }) else {
        return bad_request("Polymarket wallet is not configured yet");
    };

    let a_pub = parse_hex_biguint(&req.a_pub);
    if a_pub.is_zero() || (&a_pub % srp_n()).is_zero() {
        return bad_request("invalid SRP A value");
    }

    let v = parse_hex_biguint(&pending.verifier_hex);
    let b = parse_hex_biguint(&pending.b_hex);
    let b_pub = parse_hex_biguint(&pending.b_pub_hex);
    let u = hash_hex_biguints(&[&a_pub, &b_pub]);
    let vu = v.modpow(&u, &srp_n());
    let avu = (&a_pub * vu) % srp_n();
    let s = avu.modpow(&b, &srp_n());
    let k = hash_biguint(&s);
    let expected_m1 = compute_m1(&a_pub, &b_pub, &k);

    if !constant_time_eq_hex(&expected_m1, &req.m1) {
        warn!("Polymarket SRP proof mismatch");
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid password"})));
    }

    let file_key_hex = match decrypt_wrapped_file_key(&k, &req.wrapped_file_key_iv, &req.wrapped_file_key) {
        Ok(v) => v,
        Err(e) => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": format!("failed to unwrap file key: {e}")}))),
    };

    let creds = match decrypt_credentials(&cfg.age_ciphertext, &file_key_hex) {
        Ok(v) => v,
        Err(e) => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": format!("failed to decrypt wallet credentials: {e}")}))),
    };

    info!("🔓 Wallet decrypted: has_private_key={}, has_funder={}, addr={}", creds.private_key.is_some(), creds.funder_address.is_some(), &creds.address[..10]);

    *state.polymarket_credentials.lock().await = Some(creds.clone());

    // Create Polymarket CLOB client for live trading
    let poly_config = polymarket_client::ClientConfig {
        host: "https://clob.polymarket.com".to_string(),
        chain_id: 137,
        credentials: polymarket_client::ApiCredentials {
            api_key: creds.api_key.clone(),
            api_secret: creds.api_secret.clone(),
            api_passphrase: creds.api_passphrase.clone(),
        },
        address: creds.address.clone(),
        private_key: creds.private_key.clone(),
        funder_address: creds.funder_address.clone(),
        proxy: std::env::var("HTTPS_PROXY")
            .or_else(|_| std::env::var("https_proxy"))
            .or_else(|_| std::env::var("HTTP_PROXY"))
            .or_else(|_| std::env::var("http_proxy"))
            .ok(),
    };
    match polymarket_client::PolymarketClient::new(poly_config) {
        Ok(client) => {
            info!("✅ Polymarket CLOB client ready for live trading");
            *state.polymarket_client.lock().await = Some(client);
        }
        Err(e) => {
            error!("⚠️ Failed to create Polymarket client: {}", e);
        }
    }

    let m2 = compute_m2(&a_pub, &req.m1, &k);
    (
        StatusCode::OK,
        Json(serde_json::to_value(ConnectVerifyResponse { m2, connected: true }).unwrap()),
    )
}

pub async fn disconnect(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    *state.polymarket_credentials.lock().await = None;
    *state.polymarket_client.lock().await = None;
    info!("Polymarket wallet disconnected");
    (StatusCode::OK, Json(serde_json::json!({"connected": false})))
}

/// Debug endpoint: directly unlock wallet with password (bypasses SRP)
/// Only available when compiled with debug assertions
pub async fn debug_unlock_wallet(
    State(state): State<SharedState>,
    Json(body): Json<serde_json::Value>,
) -> (StatusCode, Json<serde_json::Value>) {
    let password = body.get("password").and_then(|v| v.as_str()).unwrap_or("");
    if password.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "password required"})));
    }

    let cfg = match load_wallet_config(&state.db_pool).await {
        Ok(Some(c)) => c,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "wallet not configured"}))),
        Err(e) => return internal_error(e),
    };

    let file_key_hex = derive_file_key_hex(password, &cfg.srp_salt);
    let creds = match decrypt_credentials(&cfg.age_ciphertext, &file_key_hex) {
        Ok(v) => v,
        Err(e) => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": format!("decryption failed: {e}")}))),
    };

    info!("🔓 [DEBUG] Wallet unlocked: has_pk={}, has_funder={}, addr={}", 
          creds.private_key.is_some(), creds.funder_address.is_some(), &creds.address[..10.min(creds.address.len())]);

    *state.polymarket_credentials.lock().await = Some(creds.clone());

    let host = std::env::var("POLYMARKET_HOST").unwrap_or_else(|_| "https://clob.polymarket.com".to_string());
    let chain_id = std::env::var("POLYMARKET_CHAIN_ID").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(137);
    let proxy = std::env::var("POLYMARKET_PROXY").ok();
    let poly_config = polymarket_client::ClientConfig {
        host, chain_id, proxy,
        credentials: polymarket_client::ApiCredentials {
            api_key: creds.api_key.clone(),
            api_secret: creds.api_secret.clone(),
            api_passphrase: creds.api_passphrase.clone(),
        },
        address: creds.address.clone(),
        private_key: creds.private_key.clone(),
        funder_address: creds.funder_address.clone(),
    };

    match polymarket_client::PolymarketClient::new(poly_config) {
        Ok(client) => {
            info!("✅ [DEBUG] Polymarket CLOB client ready");
            *state.polymarket_client.lock().await = Some(client);
        }
        Err(e) => {
            error!("⚠️ [DEBUG] Failed to create Polymarket client: {}", e);
        }
    }

    (StatusCode::OK, Json(serde_json::json!({"connected": true, "address": creds.address, "has_private_key": creds.private_key.is_some(), "has_funder": creds.funder_address.is_some()})))
}

pub async fn polymarket_balance(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    let client: polymarket_client::PolymarketClient = match build_client_from_state(&state).await {
        Ok(c) => c,
        Err(status) => return status,
    };

    match client.get_balance().await {
        Ok(balance) => (StatusCode::OK, Json(serde_json::to_value(balance).unwrap_or_default())),
        Err(e) => (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn polymarket_orders(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    let client: polymarket_client::PolymarketClient = match build_client_from_state(&state).await {
        Ok(c) => c,
        Err(status) => return status,
    };

    match client.get_open_orders().await {
        Ok(data) => (StatusCode::OK, Json(serde_json::to_value(data).unwrap_or_default())),
        Err(e) => (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))),
    }
}

pub async fn polymarket_positions(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    if state.polymarket_credentials.lock().await.is_none() {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "wallet not connected"})));
    }
    (StatusCode::OK, Json(serde_json::json!([])))
}

pub async fn polymarket_trades(State(state): State<SharedState>) -> (StatusCode, Json<serde_json::Value>) {
    if state.polymarket_credentials.lock().await.is_none() {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "wallet not connected"})));
    }
    (StatusCode::OK, Json(serde_json::json!([])))
}

pub async fn setup_polymarket_interactive(pool: &SqlitePool) -> Result<()> {
    use std::io::{self, Write};

    ensure_wallet_config_table(pool).await?;

    fn prompt(label: &str) -> Result<String> {
        print!("{label}: ");
        io::stdout().flush()?;
        let mut s = String::new();
        io::stdin().read_line(&mut s)?;
        Ok(s.trim().to_string())
    }

    let api_key = prompt("Polymarket API key")?;
    let api_secret = prompt("Polymarket API secret")?;
    let api_passphrase = prompt("Polymarket API passphrase")?;
    let address = prompt("Wallet address (0x...)")?;
    let private_key_input = prompt("Private key (for EIP-712 order signing, enter to skip)")?;
    let private_key = if private_key_input.is_empty() { None } else { Some(private_key_input) };
    let funder_input = prompt("Proxy wallet (funder) address (for POLY_PROXY signing, enter to skip)")?;
    let funder_address = if funder_input.is_empty() { None } else { Some(funder_input) };
    let password = rpassword::prompt_password("Wallet unlock password: ")?;
    let confirm = rpassword::prompt_password("Confirm wallet unlock password: ")?;
    if password != confirm {
        anyhow::bail!("passwords do not match");
    }

    let salt_hex = random_hex(16);
    let verifier_hex = compute_verifier_hex(USERNAME, &password, &salt_hex);
    let file_key_hex = derive_file_key_hex(&password, &salt_hex);
    let age_ciphertext = encrypt_credentials(&api_key, &api_secret, &api_passphrase, &address, &file_key_hex, private_key.as_deref(), funder_address.as_deref())?;
    upsert_wallet_config(pool, &salt_hex, &verifier_hex, &age_ciphertext).await?;
    println!("✅ Polymarket wallet configured.");
    Ok(())
}

async fn build_client_from_state(state: &Arc<AppState>) -> Result<polymarket_client::PolymarketClient, (StatusCode, Json<serde_json::Value>)> {
    let creds = state.polymarket_credentials.lock().await.clone().ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "wallet not connected"})),
        )
    })?;

    let host = std::env::var("POLYMARKET_HOST").unwrap_or_else(|_| "https://clob.polymarket.com".to_string());
    let chain_id = std::env::var("POLYMARKET_CHAIN_ID")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(137);
    let proxy = std::env::var("POLYMARKET_PROXY").ok();

    let config = polymarket_client::ClientConfig {
        host,
        chain_id,
        proxy,
        credentials: polymarket_client::ApiCredentials {
            api_key: creds.api_key,
            api_secret: creds.api_secret,
            api_passphrase: creds.api_passphrase,
        },
        address: creds.address.clone(),
        private_key: creds.private_key,
        funder_address: creds.funder_address,
    };
    polymarket_client::PolymarketClient::new(config).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))
}

pub fn compute_verifier_hex(username: &str, password: &str, salt_hex: &str) -> String {
    let x = compute_x(username, password, salt_hex);
    let v = srp_g().modpow(&x, &srp_n());
    to_hex(&v)
}

fn compute_x(username: &str, password: &str, salt_hex: &str) -> BigUint {
    let inner = sha256_bytes(format!("{username}:{password}").as_bytes());
    let salt = hex::decode(salt_hex).expect("valid salt hex");
    let mut outer = Vec::with_capacity(salt.len() + inner.len());
    outer.extend_from_slice(&salt);
    outer.extend_from_slice(&inner);
    BigUint::from_bytes_be(&sha256_bytes(&outer))
}

fn compute_b_pub(v: &BigUint, b: &BigUint) -> BigUint {
    let gb = srp_g().modpow(b, &srp_n());
    ((srp_k() * v) + gb) % srp_n()
}

fn compute_m1(a_pub: &BigUint, b_pub: &BigUint, key: &[u8]) -> String {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&pad_to_n(a_pub));
    bytes.extend_from_slice(&pad_to_n(b_pub));
    bytes.extend_from_slice(key);
    hex::encode(sha256_bytes(&bytes))
}

fn compute_m2(a_pub: &BigUint, m1_hex: &str, key: &[u8]) -> String {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&pad_to_n(a_pub));
    bytes.extend_from_slice(&hex::decode(m1_hex).unwrap_or_default());
    bytes.extend_from_slice(key);
    hex::encode(sha256_bytes(&bytes))
}

fn decrypt_wrapped_file_key(session_key: &[u8], iv_b64: &str, ciphertext_b64: &str) -> Result<String> {
    use aes_gcm::{aead::{Aead, KeyInit}, Aes256Gcm, Nonce};
    let iv = B64.decode(iv_b64).context("invalid iv base64")?;
    let ciphertext = B64.decode(ciphertext_b64).context("invalid ciphertext base64")?;
    let cipher = Aes256Gcm::new_from_slice(session_key).context("invalid AES key")?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&iv), ciphertext.as_ref())
        .map_err(|_| anyhow!("AES-GCM decrypt failed"))?;
    String::from_utf8(plaintext).context("wrapped file key is not utf8")
}

fn parse_hex_biguint(hex_str: &str) -> BigUint {
    BigUint::from_str_radix(hex_str, 16).expect("valid biguint hex")
}

fn srp_n() -> BigUint {
    parse_hex_biguint(SRP_N_HEX_FULL)
}

fn srp_g() -> BigUint {
    BigUint::from(SRP_G)
}

fn srp_k() -> BigUint {
    hash_hex_biguints(&[&srp_n(), &srp_g()])
}

fn pad_to_n(num: &BigUint) -> Vec<u8> {
    let n_len = hex::decode(SRP_N_HEX_FULL).unwrap().len();
    let bytes = num.to_bytes_be();
    if bytes.len() >= n_len {
        bytes
    } else {
        let mut out = vec![0u8; n_len - bytes.len()];
        out.extend_from_slice(&bytes);
        out
    }
}

fn hash_hex_biguints(nums: &[&BigUint]) -> BigUint {
    let mut bytes = Vec::new();
    for num in nums {
        bytes.extend_from_slice(&pad_to_n(num));
    }
    BigUint::from_bytes_be(&sha256_bytes(&bytes))
}

fn hash_biguint(num: &BigUint) -> Vec<u8> {
    sha256_bytes(&pad_to_n(num))
}

fn sha256_bytes(data: &[u8]) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().to_vec()
}

fn to_hex(num: &BigUint) -> String {
    let s = num.to_str_radix(16);
    if s.len() % 2 == 1 { format!("0{s}") } else { s }
}

fn random_hex(len: usize) -> String {
    let mut buf = vec![0u8; len];
    OsRng.fill_bytes(&mut buf);
    hex::encode(buf)
}

fn constant_time_eq_hex(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut x = 0u8;
    for i in 0..a.len() {
        x |= a[i] ^ b[i];
    }
    x == 0
}

fn bad_request(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": msg})))
}

fn internal_error(err: anyhow::Error) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": err.to_string()})))
}

/// Test endpoint: manually place a small Polymarket order to verify the signing pipeline.
/// POST /api/polymarket/test-order
/// Body: { "token_id": "0x...", "price": 0.5, "size": 1.0 }
pub async fn test_polymarket_order(
    State(state): State<SharedState>,
    axum::extract::Json(body): axum::extract::Json<serde_json::Value>,
) -> (StatusCode, Json<serde_json::Value>) {
    let client = match state.polymarket_client.lock().await.clone() {
        Some(c) => c,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "wallet not connected"}))),
    };

    let token_id = body.get("token_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let price = body.get("price").and_then(|v| v.as_f64()).unwrap_or(0.5);
    let size = body.get("size").and_then(|v| v.as_f64()).unwrap_or(1.0);

    if token_id.is_empty() {
        return bad_request("token_id is required");
    }

    info!("🧪 TEST ORDER: token_id={} (len={}), price={}, size={}", &token_id[..token_id.len().min(20)], token_id.len(), price, size);
    info!("🧪 TEST ORDER: has_pk={}, addr={}", client.has_private_key(), client.address());

    // Try build_buy_order
    match client.build_buy_order(&token_id, price, size, None, false) {
        Some(signed_order) => {
            let body = serde_json::to_string(&signed_order).unwrap_or_default();
            info!("🧪 TEST ORDER payload: {}", &body[..body.len().min(500)]);
            info!("🧪 TEST ORDER: signed order built, maker={}, sig_len={}", signed_order.maker, signed_order.signature.len());
            match client.place_signed_order(&signed_order).await {
                Ok(resp) => {
                    info!("🧪 TEST ORDER: placed! order_id={}, status={}", resp.order_id, resp.status);
                    (StatusCode::OK, Json(serde_json::json!({
                        "success": true,
                        "order_id": resp.order_id,
                        "status": resp.status,
                        "fills": resp.fills.len(),
                    })))
                }
                Err(e) => {
                    error!("🧪 TEST ORDER: place failed: {}", e);
                    (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()})))
                }
            }
        }
        None => {
            error!("🧪 TEST ORDER: build_buy_order returned None");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "build_buy_order failed (check logs for details)"})))
        }
    }
}
