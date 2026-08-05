//! Envelope encryption for cloud BYOK secrets (ADR-0010 PR7).
//!
//! Master key from env `BYOK_MASTER_KEY` (32-byte raw key as **base64** or **hex**).
//! Per-secret AES-256-GCM with a random 12-byte nonce.
//!
//! Fail closed: missing / wrong-length / undecodable master key → error (no silent
//! zero-key, no plaintext fallback).

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use common::AppError;
use rand::RngCore;

/// AES-256-GCM nonce size (96-bit, standard for GCM).
pub const BYOK_NONCE_LEN: usize = 12;
/// AES-256 key size.
pub const BYOK_KEY_LEN: usize = 32;

/// Parsed master key for BYOK envelope encryption.
#[derive(Clone)]
pub struct ByokMasterKey {
    key: [u8; BYOK_KEY_LEN],
}

impl std::fmt::Debug for ByokMasterKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ByokMasterKey")
            .field("key", &"[redacted]")
            .finish()
    }
}

impl ByokMasterKey {
    /// Parse a 32-byte key from base64 (standard) or hex (64 hex chars).
    pub fn parse(raw: &str) -> Result<Self, AppError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(AppError::validation(
                "byok_master_key_missing",
                "BYOK_MASTER_KEY is empty",
            ));
        }

        // Prefer hex when it looks like hex of exact key length.
        if trimmed.len() == BYOK_KEY_LEN * 2 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
            let bytes = hex::decode(trimmed).map_err(|_| {
                AppError::validation(
                    "byok_master_key_invalid",
                    "BYOK_MASTER_KEY hex decode failed",
                )
            })?;
            return Self::from_bytes(&bytes);
        }

        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(trimmed)
            .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(trimmed))
            .map_err(|_| {
                AppError::validation(
                    "byok_master_key_invalid",
                    "BYOK_MASTER_KEY must be 32-byte base64 or 64-char hex",
                )
            })?;
        Self::from_bytes(&bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, AppError> {
        if bytes.len() != BYOK_KEY_LEN {
            return Err(AppError::validation(
                "byok_master_key_invalid",
                format!(
                    "BYOK_MASTER_KEY must decode to {BYOK_KEY_LEN} bytes, got {}",
                    bytes.len()
                ),
            ));
        }
        let mut key = [0u8; BYOK_KEY_LEN];
        key.copy_from_slice(bytes);
        Ok(Self { key })
    }

    /// Load from `BYOK_MASTER_KEY` env. Fails closed when unset or malformed.
    pub fn from_env() -> Result<Self, AppError> {
        let raw = std::env::var("BYOK_MASTER_KEY").map_err(|_| {
            AppError::validation(
                "byok_master_key_missing",
                "BYOK_MASTER_KEY is not set",
            )
        })?;
        Self::parse(&raw)
    }

    /// Encrypt plaintext → (ciphertext, nonce). Never log plaintext.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>), AppError> {
        let cipher = Aes256Gcm::new_from_slice(&self.key).map_err(|_| {
            AppError::internal("byok cipher init failed")
        })?;
        let mut nonce_bytes = [0u8; BYOK_NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, plaintext).map_err(|_| {
            AppError::internal("byok encrypt failed")
        })?;
        Ok((ciphertext, nonce_bytes.to_vec()))
    }

    /// Decrypt ciphertext with the given nonce. Fail closed on tamper / wrong key.
    pub fn decrypt(&self, ciphertext: &[u8], nonce: &[u8]) -> Result<Vec<u8>, AppError> {
        if nonce.len() != BYOK_NONCE_LEN {
            return Err(AppError::validation(
                "byok_nonce_invalid",
                format!("nonce must be {BYOK_NONCE_LEN} bytes"),
            ));
        }
        let cipher = Aes256Gcm::new_from_slice(&self.key).map_err(|_| {
            AppError::internal("byok cipher init failed")
        })?;
        let nonce = Nonce::from_slice(nonce);
        cipher.decrypt(nonce, ciphertext).map_err(|_| {
            AppError::internal("byok decrypt failed")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_key_hex() -> String {
        // 32 zero bytes as hex — tests only.
        "00".repeat(BYOK_KEY_LEN)
    }

    fn sample_key_b64() -> String {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode([0xab; BYOK_KEY_LEN])
    }

    #[test]
    fn parse_hex_and_base64() {
        let hex_key = ByokMasterKey::parse(&sample_key_hex()).unwrap();
        let b64_key = ByokMasterKey::parse(&sample_key_b64()).unwrap();
        // Different material; both valid.
        assert_ne!(format!("{hex_key:?}"), "");
        assert_ne!(format!("{b64_key:?}"), "");
        // Debug never prints key bytes.
        assert!(format!("{hex_key:?}").contains("redacted"));
    }

    #[test]
    fn empty_and_short_fail_closed() {
        assert_eq!(
            ByokMasterKey::parse("").unwrap_err().code(),
            "byok_master_key_missing"
        );
        assert_eq!(
            ByokMasterKey::parse("too-short").unwrap_err().code(),
            "byok_master_key_invalid"
        );
        // 16 bytes base64 → wrong length
        use base64::Engine;
        let short = base64::engine::general_purpose::STANDARD.encode([0u8; 16]);
        assert_eq!(
            ByokMasterKey::parse(&short).unwrap_err().code(),
            "byok_master_key_invalid"
        );
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = ByokMasterKey::parse(&sample_key_b64()).unwrap();
        let plain = b"sk-test-secret-value-do-not-log";
        let (ct, nonce) = key.encrypt(plain).unwrap();
        assert_ne!(ct.as_slice(), plain.as_slice());
        assert_eq!(nonce.len(), BYOK_NONCE_LEN);
        let out = key.decrypt(&ct, &nonce).unwrap();
        assert_eq!(out, plain);
    }

    #[test]
    fn wrong_key_fails_decrypt() {
        let key_a = ByokMasterKey::parse(&sample_key_hex()).unwrap();
        let key_b = ByokMasterKey::parse(&sample_key_b64()).unwrap();
        let (ct, nonce) = key_a.encrypt(b"secret").unwrap();
        assert!(key_b.decrypt(&ct, &nonce).is_err());
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let key = ByokMasterKey::parse(&sample_key_b64()).unwrap();
        let (mut ct, nonce) = key.encrypt(b"secret").unwrap();
        if let Some(b) = ct.last_mut() {
            *b ^= 0xff;
        }
        assert!(key.decrypt(&ct, &nonce).is_err());
    }
}
