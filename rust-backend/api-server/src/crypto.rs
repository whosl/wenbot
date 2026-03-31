use age::{secrecy::SecretString, Decryptor, Encryptor};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredentials {
    pub api_key: String,
    pub api_secret: String,
    pub api_passphrase: String,
}

pub fn derive_file_key_hex(password: &str, srp_salt_hex: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"wenbot:polymarket:file-key:v1:");
    hasher.update(srp_salt_hex.as_bytes());
    hasher.update(b":");
    hasher.update(password.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn encrypt_credentials(
    api_key: &str,
    api_secret: &str,
    api_passphrase: &str,
    age_passphrase: &str,
) -> Result<Vec<u8>> {
    let payload = serde_json::to_vec(&StoredCredentials {
        api_key: api_key.to_string(),
        api_secret: api_secret.to_string(),
        api_passphrase: api_passphrase.to_string(),
    })?;

    let encryptor = Encryptor::with_user_passphrase(SecretString::new(age_passphrase.to_string()));
    let mut encrypted = Vec::new();
    let mut writer = encryptor.wrap_output(&mut encrypted)?;
    writer.write_all(&payload)?;
    writer.finish()?;
    Ok(encrypted)
}

pub fn decrypt_credentials(age_data: &[u8], age_passphrase: &str) -> Result<StoredCredentials> {
    let decryptor = Decryptor::new(age_data)?;
    let mut reader = match decryptor {
        Decryptor::Passphrase(d) => d.decrypt(&SecretString::new(age_passphrase.to_string()), None)?,
        _ => anyhow::bail!("unexpected non-passphrase age payload"),
    };

    let mut plaintext = Vec::new();
    reader.read_to_end(&mut plaintext)?;
    let creds: StoredCredentials = serde_json::from_slice(&plaintext)
        .context("failed to decode decrypted Polymarket credentials")?;
    Ok(creds)
}
