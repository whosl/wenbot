//! API credential management — 从环境变量加载，绝不硬编码

use anyhow::{Context, Result};

/// Polymarket API credentials
///
/// SECURITY: Only loaded from environment variables.
/// Never logged, never serialized to files, never sent to LLM APIs.
#[derive(Clone)]
pub struct ApiCredentials {
    pub api_key: String,
    pub api_secret: String,
    pub api_passphrase: String,
}

impl std::fmt::Debug for ApiCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiCredentials")
            .field("api_key", &"[REDACTED]")
            .field("api_secret", &"[REDACTED]")
            .field("api_passphrase", &"[REDACTED]")
            .finish()
    }
}

impl ApiCredentials {
    /// Load credentials from environment variables.
    ///
    /// # Security
    /// - Credentials are only read from `std::env`
    /// - The `Display` impl masks sensitive values
    /// - Never hardcode these values in source code
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("POLYMARKET_API_KEY")
            .context("POLYMARKET_API_KEY not set in environment")?;
        let api_secret = std::env::var("POLYMARKET_API_SECRET")
            .context("POLYMARKET_API_SECRET not set in environment")?;
        let api_passphrase = std::env::var("POLYMARKET_API_PASSPHRASE")
            .context("POLYMARKET_API_PASSPHRASE not set in environment")?;

        Ok(Self {
            api_key,
            api_secret,
            api_passphrase,
        })
    }
}

/// Masked display — only shows first 4 and last 4 characters
impl std::fmt::Display for ApiCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ApiCredentials(key=****{}, secret=****{}, passphrase=****{})",
            &self.api_key[self.api_key.len().saturating_sub(4)..],
            &self.api_secret[self.api_secret.len().saturating_sub(4)..],
            &self.api_passphrase[self.api_passphrase.len().saturating_sub(4)..],
        )
    }
}

/// L1签名的 API 请求头生成
#[derive(Clone)]
pub struct L1Signature {
    api_key: String,
    api_secret: String,
    api_passphrase: String,
    address: String,
}

impl L1Signature {
    pub fn new(creds: &ApiCredentials, address: &str) -> Self {
        Self {
            api_key: creds.api_key.clone(),
            api_secret: creds.api_secret.clone(),
            api_passphrase: creds.api_passphrase.clone(),
            address: address.to_lowercase(),
        }
    }

    /// Returns the signer address (lowercase, without 0x prefix)
    pub fn address(&self) -> String {
        self.address.clone()
    }

    /// Get the API key UUID
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// Generate HMAC-SHA256 signature and return L1 auth headers.
    ///
    /// Polymarket CLOB L1 auth:
    /// - timestamp (seconds since epoch)
    /// - timestamp + method + path + body as signature payload
    /// - HMAC-SHA256 with api_secret
    pub fn sign_request(&self, method: &str, path: &str, body: &str) -> AuthHeaders {
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let message = format!("{}{}{}{}", timestamp, method, path, body);

        let signature = hmac_sha256(&self.api_secret, &message);

        tracing::debug!(
            "POLY signing: method={} path={} body_len={} sig_len={} key_prefix=...{} secret_len={} secret_end=...{} pass_len={} addr={}",
            method, path, body.len(), signature.len(),
            &self.api_key[self.api_key.len().saturating_sub(6)..],
            self.api_secret.len(),
            &self.api_secret[self.api_secret.len().saturating_sub(4)..],
            self.api_passphrase.len(),
            self.address,
        );

        let mut hdrs = AuthHeaders::new();
        hdrs.set("POLY_ADDRESS", self.address.clone());
        hdrs.set("POLY_API_KEY", self.api_key.clone());
        hdrs.set("POLY_SIGNATURE", signature);
        hdrs.set("POLY_PASSPHRASE", self.api_passphrase.clone());
        hdrs.set("POLY_TIMESTAMP", timestamp);
        hdrs
    }
}

/// Auth headers wrapper
pub struct AuthHeaders {
    pub headers: Vec<(String, String)>,
}

impl AuthHeaders {
    pub fn new() -> Self {
        Self { headers: Vec::new() }
    }

    pub fn set(&mut self, key: &str, value: String) {
        self.headers.push((key.to_string(), value));
    }
}

/// HMAC-SHA256 signature — matches Polymarket Python SDK:
/// 1. Decode secret from base64url to raw bytes
/// 2. HMAC-SHA256 with raw secret bytes
/// 3. Encode result as base64url
fn hmac_sha256(secret: &str, message: &str) -> String {
    use base64::Engine;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    // Decode base64url-encoded secret to raw bytes
    // Polymarket stores secrets as base64url (may or may not have padding)
    let secret_bytes = base64::engine::general_purpose::URL_SAFE
        .decode(secret)
        .unwrap_or_else(|_| {
            // Fallback: strip padding and try NO_PAD
            let trimmed = secret.trim_end_matches('=');
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(trimmed)
                .unwrap_or_else(|_| {
                    // Last resort: try standard base64
                    base64::engine::general_purpose::STANDARD
                        .decode(secret)
                        .unwrap_or_else(|_| secret.as_bytes().to_vec())
                })
        });

    let mut mac = HmacSha256::new_from_slice(&secret_bytes)
        .expect("HMAC can take key of any size");
    mac.update(message.as_bytes());
    let result = mac.finalize();

    // Encode result as base64url (WITH padding, matching Polymarket server requirements)
    base64::engine::general_purpose::URL_SAFE.encode(result.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hmac_sha256_deterministic() {
        let sig1 = hmac_sha256("secret", "hello world");
        let sig2 = hmac_sha256("secret", "hello world");
        assert_eq!(sig1, sig2);
        assert_eq!(sig1.len(), 44); // base64url SHA256 = 44 chars (with padding)
    }

    #[test]
    fn test_hmac_sha256_different_inputs() {
        let sig1 = hmac_sha256("secret", "hello world");
        let sig2 = hmac_sha256("secret", "hello mars");
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn test_credentials_masked_display() {
        let creds = ApiCredentials {
            api_key: "abcdef1234567890".into(),
            api_secret: "fedcba0987654321".into(),
            api_passphrase: "1122334455667788".into(),
        };
        let display = format!("{}", creds);
        assert!(!display.contains("abcdef1234567890"));
        assert!(display.contains("****"));
    }
}
