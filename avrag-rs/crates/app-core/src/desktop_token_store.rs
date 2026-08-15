//! Desktop relay token persistence boundary (2026-08-15 desktop cloud login wave, W2).
//!
//! Desktop tokens are long-lived, revocable, user-scoped credentials that
//! authorize ONLY the cloud `/v1/relay/*` routes (platform official-key proxy).
//! Plaintext format `cos_dt_<32 hex>`; only the sha256 hash is stored.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use common::AppError;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Plaintext token prefix (machine-stable; clients may sniff it).
pub const DESKTOP_TOKEN_PREFIX: &str = "cos_dt_";

/// Public view of a desktop token row (redacted — never contains plaintext).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopTokenView {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub name: String,
    pub prefix: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Identity resolved from a presented plaintext token (relay auth guard).
#[derive(Debug, Clone)]
pub struct DesktopTokenIdentity {
    pub id: Uuid,
    pub owner_user_id: Uuid,
}

/// Mint request body (`POST /api/v1/desktop/tokens`).
#[derive(Debug, Clone, Deserialize)]
pub struct MintDesktopTokenRequest {
    pub name: String,
}

/// Mint response: the only time plaintext is returned.
#[derive(Debug, Clone, Serialize)]
pub struct MintedDesktopTokenResponse {
    #[serde(flatten)]
    pub view: DesktopTokenView,
    pub token: String,
}

/// sha256 hex of a plaintext desktop token.
pub fn hash_desktop_token(plaintext: &str) -> String {
    crate::api_key::hash_api_key(plaintext)
}

/// Generate a new desktop token: `(plaintext, hash, display_prefix)`.
pub fn generate_desktop_token() -> (String, String, String) {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    let plaintext = format!("{DESKTOP_TOKEN_PREFIX}{}", hex::encode(bytes));
    let hash = hash_desktop_token(&plaintext);
    let prefix = plaintext.chars().take(13).collect();
    (plaintext, hash, prefix)
}

fn validate_name(raw: &str) -> Result<String, AppError> {
    let name = raw.trim();
    if name.is_empty() || name.len() > 128 {
        return Err(AppError::validation(
            "desktop_token_name_invalid",
            "name is required and must be ≤ 128 chars",
        ));
    }
    Ok(name.to_string())
}

#[async_trait]
pub trait DesktopTokenStorePort: Send + Sync {
    /// Insert a new active token row; returns the redacted view.
    async fn insert(
        &self,
        owner_user_id: Uuid,
        name: &str,
        token_hash: &str,
        prefix: &str,
    ) -> Result<DesktopTokenView, AppError>;

    /// List the owner's tokens, newest first (includes revoked rows).
    async fn list(&self, owner_user_id: Uuid) -> Result<Vec<DesktopTokenView>, AppError>;

    /// Soft-revoke by id (owner-scoped). Idempotent on already-revoked rows.
    async fn revoke(&self, owner_user_id: Uuid, id: Uuid) -> Result<DesktopTokenView, AppError>;

    /// Resolve an **active** identity by token hash (revoked → `Ok(None)`).
    async fn resolve_by_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<DesktopTokenIdentity>, AppError>;

    /// Best-effort `last_used_at` bump; failures are tolerated by callers.
    async fn touch_last_used(&self, id: Uuid) -> Result<(), AppError>;
}

/// Mint + persist a desktop token for the owner. Plaintext returned once.
pub async fn mint_desktop_token(
    store: &Arc<dyn DesktopTokenStorePort>,
    owner_user_id: Uuid,
    name: &str,
) -> Result<MintedDesktopTokenResponse, AppError> {
    let name = validate_name(name)?;
    let (plaintext, hash, prefix) = generate_desktop_token();
    let view = store.insert(owner_user_id, &name, &hash, &prefix).await?;
    Ok(MintedDesktopTokenResponse {
        view,
        token: plaintext,
    })
}

/// In-memory store for memory-mode bootstrap and tests.
#[derive(Default)]
pub struct MemoryDesktopTokenStore {
    rows: RwLock<BTreeMap<Uuid, DesktopTokenView>>,
    by_hash: RwLock<BTreeMap<String, Uuid>>,
}

impl MemoryDesktopTokenStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl DesktopTokenStorePort for MemoryDesktopTokenStore {
    async fn insert(
        &self,
        owner_user_id: Uuid,
        name: &str,
        token_hash: &str,
        prefix: &str,
    ) -> Result<DesktopTokenView, AppError> {
        if self.by_hash.read().await.contains_key(token_hash) {
            return Err(AppError::conflict(
                "desktop_token_hash_conflict",
                "token hash already exists",
            ));
        }
        let view = DesktopTokenView {
            id: Uuid::new_v4(),
            owner_user_id,
            name: name.to_string(),
            prefix: prefix.to_string(),
            created_at: Utc::now(),
            last_used_at: None,
            revoked_at: None,
        };
        self.rows.write().await.insert(view.id, view.clone());
        self.by_hash
            .write()
            .await
            .insert(token_hash.to_string(), view.id);
        Ok(view)
    }

    async fn list(&self, owner_user_id: Uuid) -> Result<Vec<DesktopTokenView>, AppError> {
        let mut rows: Vec<_> = self
            .rows
            .read()
            .await
            .values()
            .filter(|row| row.owner_user_id == owner_user_id)
            .cloned()
            .collect();
        rows.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(rows)
    }

    async fn revoke(&self, owner_user_id: Uuid, id: Uuid) -> Result<DesktopTokenView, AppError> {
        let mut rows = self.rows.write().await;
        let row = rows.get_mut(&id).filter(|row| row.owner_user_id == owner_user_id);
        let Some(row) = row else {
            return Err(AppError::not_found(
                "desktop_token_not_found",
                "desktop token not found",
            ));
        };
        if row.revoked_at.is_none() {
            row.revoked_at = Some(Utc::now());
        }
        Ok(row.clone())
    }

    async fn resolve_by_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<DesktopTokenIdentity>, AppError> {
        let Some(id) = self.by_hash.read().await.get(token_hash).copied() else {
            return Ok(None);
        };
        let rows = self.rows.read().await;
        let Some(row) = rows.get(&id) else {
            return Ok(None);
        };
        if row.revoked_at.is_some() {
            return Ok(None);
        }
        Ok(Some(DesktopTokenIdentity {
            id: row.id,
            owner_user_id: row.owner_user_id,
        }))
    }

    async fn touch_last_used(&self, id: Uuid) -> Result<(), AppError> {
        if let Some(row) = self.rows.write().await.get_mut(&id) {
            row.last_used_at = Some(Utc::now());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_token_shape() {
        let (plaintext, hash, prefix) = generate_desktop_token();
        assert!(plaintext.starts_with(DESKTOP_TOKEN_PREFIX));
        assert_eq!(plaintext.len(), DESKTOP_TOKEN_PREFIX.len() + 32);
        assert!(plaintext[DESKTOP_TOKEN_PREFIX.len()..].chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(hash, hash_desktop_token(&plaintext));
        assert_eq!(hash.len(), 64);
        assert!(plaintext.starts_with(&prefix));
        assert_ne!(prefix, plaintext, "prefix is a display redaction");
    }

    #[test]
    fn name_validation() {
        assert!(validate_name("MacBook Pro").is_ok());
        assert!(validate_name("   ").is_err());
        assert!(validate_name(&"x".repeat(129)).is_err());
    }

    #[tokio::test]
    async fn memory_store_mint_resolve_revoke() {
        let store: Arc<dyn DesktopTokenStorePort> = Arc::new(MemoryDesktopTokenStore::new());
        let owner = Uuid::new_v4();
        let minted = mint_desktop_token(&store, owner, "laptop").await.unwrap();

        let identity = store
            .resolve_by_hash(&hash_desktop_token(&minted.token))
            .await
            .unwrap()
            .expect("active token resolves");
        assert_eq!(identity.owner_user_id, owner);
        assert_eq!(identity.id, minted.view.id);

        // Wrong token → no identity.
        assert!(
            store
                .resolve_by_hash(&hash_desktop_token("cos_dt_deadbeef"))
                .await
                .unwrap()
                .is_none()
        );

        // List redacts plaintext.
        let rows = store.list(owner).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "laptop");
        assert!(rows[0].last_used_at.is_none());

        store.touch_last_used(identity.id).await.unwrap();
        assert!(store.list(owner).await.unwrap()[0].last_used_at.is_some());

        // Revoke → resolve fails, list still shows the row.
        store.revoke(owner, identity.id).await.unwrap();
        assert!(
            store
                .resolve_by_hash(&hash_desktop_token(&minted.token))
                .await
                .unwrap()
                .is_none()
        );
        assert!(store.list(owner).await.unwrap()[0].revoked_at.is_some());

        // Cross-owner revoke misses.
        let err = store.revoke(Uuid::new_v4(), identity.id).await.unwrap_err();
        assert_eq!(err.code(), "desktop_token_not_found");

        // Empty name refused before insert.
        assert!(mint_desktop_token(&store, owner, "").await.is_err());
    }
}
