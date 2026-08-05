//! Cloud BYOK provider-secret HTTP service (ADR-0010 PR7 §3.2).
//!
//! API never returns plaintext keys — only fingerprints. Encrypt/decrypt lives
//! in the store adapter (`ByokMasterKey` + AES-256-GCM).
//!
//! # Wallet debit (TODO PR7 follow-up)
//!
//! `PgUsageObserver::skip_wallet_debit` is process-wide today. When chat is
//! rewired to resolve a user BYOK secret, set skip per request (or extend
//! `TenantContext`) so platform wallet is not debited for that path. Until
//! then, platform debit remains the default even if a secret is stored.

use std::sync::Arc;

use app_core::{
    ProviderSecretPurpose, ProviderSecretStorePort, ProviderSecretView, ResolvedProviderSecret,
    UpsertProviderSecretInput,
};
use common::{ApiResponse, AppError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Request body for create/update.
#[derive(Clone, Deserialize)]
pub struct UpsertProviderSecretRequest {
    /// llm | embedding | rerank
    pub purpose: String,
    /// e.g. deepseek | openai | siliconflow
    pub provider: String,
    pub api_key: String,
    pub workspace_id: Option<Uuid>,
    pub base_url: Option<String>,
    pub model_hint: Option<String>,
}

impl std::fmt::Debug for UpsertProviderSecretRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpsertProviderSecretRequest")
            .field("purpose", &self.purpose)
            .field("provider", &self.provider)
            .field("api_key", &"[redacted]")
            .field("workspace_id", &self.workspace_id)
            .field("base_url", &self.base_url)
            .field("model_hint", &self.model_hint)
            .finish()
    }
}

/// Public list/get response (fingerprint only).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderSecretResponse {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub workspace_id: Option<Uuid>,
    pub purpose: String,
    pub provider: String,
    pub base_url: Option<String>,
    pub model_hint: Option<String>,
    /// `{last4}:{length}` — never the full key.
    pub key_fingerprint: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<ProviderSecretView> for ProviderSecretResponse {
    fn from(v: ProviderSecretView) -> Self {
        Self {
            id: v.id,
            owner_user_id: v.owner_user_id,
            workspace_id: v.workspace_id,
            purpose: v.purpose,
            provider: v.provider,
            base_url: v.base_url,
            model_hint: v.model_hint,
            key_fingerprint: v.key_fingerprint,
            created_at: v.created_at,
            updated_at: v.updated_at,
            revoked_at: v.revoked_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderSecretListResponse {
    pub secrets: Vec<ProviderSecretResponse>,
}

fn parse_purpose(raw: &str) -> Result<ProviderSecretPurpose, AppError> {
    ProviderSecretPurpose::parse(raw).map_err(|msg| AppError::validation("purpose_invalid", msg))
}

/// Upsert an encrypted provider secret for the authenticated owner.
pub async fn upsert_provider_secret(
    store: Arc<dyn ProviderSecretStorePort>,
    owner_user_id: Uuid,
    body: UpsertProviderSecretRequest,
) -> Result<ProviderSecretView, AppError> {
    let purpose = parse_purpose(&body.purpose)?;
    let input = UpsertProviderSecretInput {
        owner_user_id,
        workspace_id: body.workspace_id,
        purpose,
        provider: body.provider,
        base_url: body.base_url,
        model_hint: body.model_hint,
        api_key: body.api_key,
    };
    store.upsert(&input).await
}

pub async fn list_provider_secrets(
    store: Arc<dyn ProviderSecretStorePort>,
    owner_user_id: Uuid,
    include_revoked: bool,
) -> Result<Vec<ProviderSecretView>, AppError> {
    store.list(owner_user_id, include_revoked).await
}

pub async fn revoke_provider_secret(
    store: Arc<dyn ProviderSecretStorePort>,
    owner_user_id: Uuid,
    id: Uuid,
) -> Result<ProviderSecretView, AppError> {
    store.revoke(owner_user_id, id).await
}

/// Runtime resolve (stub-friendly): decrypt active secret for outbound calls.
pub async fn resolve_provider_secret(
    store: Arc<dyn ProviderSecretStorePort>,
    owner_user_id: Uuid,
    workspace_id: Option<Uuid>,
    purpose: ProviderSecretPurpose,
) -> Result<Option<ResolvedProviderSecret>, AppError> {
    store.resolve(owner_user_id, workspace_id, purpose).await
}

pub async fn handle_upsert_provider_secret(
    store: Arc<dyn ProviderSecretStorePort>,
    owner_user_id: Uuid,
    body: UpsertProviderSecretRequest,
) -> ApiResponse<ProviderSecretResponse> {
    match upsert_provider_secret(store, owner_user_id, body).await {
        Ok(view) => ApiResponse::ok(ProviderSecretResponse::from(view)),
        Err(error) => ApiResponse::err(error.code(), error.message()),
    }
}

pub async fn handle_list_provider_secrets(
    store: Arc<dyn ProviderSecretStorePort>,
    owner_user_id: Uuid,
    include_revoked: bool,
) -> ApiResponse<ProviderSecretListResponse> {
    match list_provider_secrets(store, owner_user_id, include_revoked).await {
        Ok(views) => ApiResponse::ok(ProviderSecretListResponse {
            secrets: views.into_iter().map(ProviderSecretResponse::from).collect(),
        }),
        Err(error) => ApiResponse::err(error.code(), error.message()),
    }
}

pub async fn handle_revoke_provider_secret(
    store: Arc<dyn ProviderSecretStorePort>,
    owner_user_id: Uuid,
    id: Uuid,
) -> ApiResponse<ProviderSecretResponse> {
    match revoke_provider_secret(store, owner_user_id, id).await {
        Ok(view) => ApiResponse::ok(ProviderSecretResponse::from(view)),
        Err(error) => ApiResponse::err(error.code(), error.message()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_core::{ByokMasterKey, key_fingerprint};
    use async_trait::async_trait;
    use chrono::Utc;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// In-memory encrypting store for unit tests (same AES-GCM path as PG adapter).
    struct MemoryProviderSecretStore {
        master: ByokMasterKey,
        rows: Mutex<HashMap<Uuid, Stored>>,
    }

    struct Stored {
        view: ProviderSecretView,
        ciphertext: Vec<u8>,
        nonce: Vec<u8>,
    }

    impl MemoryProviderSecretStore {
        fn new(master: ByokMasterKey) -> Self {
            Self {
                master,
                rows: Mutex::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl ProviderSecretStorePort for MemoryProviderSecretStore {
        async fn upsert(
            &self,
            input: &UpsertProviderSecretInput,
        ) -> Result<ProviderSecretView, AppError> {
            let key = input.api_key.trim();
            if key.is_empty() {
                return Err(AppError::validation("api_key_required", "api_key is required"));
            }
            let (ciphertext, nonce) = self.master.encrypt(key.as_bytes())?;
            let fingerprint = key_fingerprint(key);
            let now = Utc::now();
            let mut rows = self.rows.lock().unwrap();

            // Revoke active same-scope rows.
            for s in rows.values_mut() {
                if s.view.owner_user_id == input.owner_user_id
                    && s.view.workspace_id == input.workspace_id
                    && s.view.purpose == input.purpose.as_str()
                    && s.view.revoked_at.is_none()
                {
                    s.view.revoked_at = Some(now);
                    s.view.updated_at = now;
                }
            }

            let id = Uuid::new_v4();
            let view = ProviderSecretView {
                id,
                owner_user_id: input.owner_user_id,
                workspace_id: input.workspace_id,
                purpose: input.purpose.as_str().to_string(),
                provider: input.provider.trim().to_ascii_lowercase(),
                base_url: input.base_url.clone(),
                model_hint: input.model_hint.clone(),
                key_fingerprint: fingerprint,
                created_at: now,
                updated_at: now,
                revoked_at: None,
            };
            rows.insert(
                id,
                Stored {
                    view: view.clone(),
                    ciphertext,
                    nonce,
                },
            );
            Ok(view)
        }

        async fn list(
            &self,
            owner_user_id: Uuid,
            include_revoked: bool,
        ) -> Result<Vec<ProviderSecretView>, AppError> {
            let rows = self.rows.lock().unwrap();
            let mut out: Vec<_> = rows
                .values()
                .filter(|s| s.view.owner_user_id == owner_user_id)
                .filter(|s| include_revoked || s.view.revoked_at.is_none())
                .map(|s| s.view.clone())
                .collect();
            out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            Ok(out)
        }

        async fn revoke(
            &self,
            owner_user_id: Uuid,
            id: Uuid,
        ) -> Result<ProviderSecretView, AppError> {
            let mut rows = self.rows.lock().unwrap();
            let Some(s) = rows.get_mut(&id) else {
                return Err(AppError::not_found(
                    "provider_secret_not_found",
                    "provider secret not found",
                ));
            };
            if s.view.owner_user_id != owner_user_id {
                return Err(AppError::not_found(
                    "provider_secret_not_found",
                    "provider secret not found",
                ));
            }
            let now = Utc::now();
            s.view.revoked_at = Some(s.view.revoked_at.unwrap_or(now));
            s.view.updated_at = now;
            Ok(s.view.clone())
        }

        async fn resolve(
            &self,
            owner_user_id: Uuid,
            workspace_id: Option<Uuid>,
            purpose: ProviderSecretPurpose,
        ) -> Result<Option<ResolvedProviderSecret>, AppError> {
            let rows = self.rows.lock().unwrap();
            let purpose_s = purpose.as_str();
            let pick = |ws: Option<Uuid>| {
                rows.values().find(|s| {
                    s.view.owner_user_id == owner_user_id
                        && s.view.workspace_id == ws
                        && s.view.purpose == purpose_s
                        && s.view.revoked_at.is_none()
                })
            };
            let stored = if workspace_id.is_some() {
                pick(workspace_id).or_else(|| pick(None))
            } else {
                pick(None)
            };
            let Some(s) = stored else {
                return Ok(None);
            };
            let plain = self.master.decrypt(&s.ciphertext, &s.nonce)?;
            let api_key = String::from_utf8(plain).map_err(|_| {
                AppError::internal("byok decrypted payload is not utf-8")
            })?;
            Ok(Some(ResolvedProviderSecret {
                id: s.view.id,
                owner_user_id: s.view.owner_user_id,
                workspace_id: s.view.workspace_id,
                purpose,
                provider: s.view.provider.clone(),
                base_url: s.view.base_url.clone(),
                model_hint: s.view.model_hint.clone(),
                api_key,
            }))
        }

        async fn has_active(
            &self,
            owner_user_id: Uuid,
            purpose: ProviderSecretPurpose,
        ) -> Result<bool, AppError> {
            let rows = self.rows.lock().unwrap();
            Ok(rows.values().any(|s| {
                s.view.owner_user_id == owner_user_id
                    && s.view.purpose == purpose.as_str()
                    && s.view.revoked_at.is_none()
            }))
        }
    }

    fn test_master() -> ByokMasterKey {
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode([0x42u8; 32]);
        ByokMasterKey::parse(&b64).unwrap()
    }

    #[tokio::test]
    async fn encrypt_store_decrypt_roundtrip_list_fingerprint_only() {
        let store: Arc<dyn ProviderSecretStorePort> =
            Arc::new(MemoryProviderSecretStore::new(test_master()));
        let owner = Uuid::new_v4();
        let plain = "sk-test-roundtrip-secret-ABCDEF";

        let view = upsert_provider_secret(
            store.clone(),
            owner,
            UpsertProviderSecretRequest {
                purpose: "llm".into(),
                provider: "deepseek".into(),
                api_key: plain.into(),
                workspace_id: None,
                base_url: Some("https://api.deepseek.com".into()),
                model_hint: Some("deepseek-v4-flash".into()),
            },
        )
        .await
        .unwrap();

        assert_eq!(view.key_fingerprint, key_fingerprint(plain));
        // List must never echo the raw key.
        let listed = list_provider_secrets(store.clone(), owner, false)
            .await
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].key_fingerprint, key_fingerprint(plain));
        let json = serde_json::to_string(&ProviderSecretResponse::from(listed[0].clone())).unwrap();
        assert!(!json.contains(plain));
        assert!(!json.contains("sk-test-roundtrip"));

        let resolved = resolve_provider_secret(
            store.clone(),
            owner,
            None,
            ProviderSecretPurpose::Llm,
        )
        .await
        .unwrap()
        .expect("resolved");
        assert_eq!(resolved.api_key, plain);
        assert_eq!(resolved.provider, "deepseek");
    }

    #[tokio::test]
    async fn revoke_prevents_resolve() {
        let store: Arc<dyn ProviderSecretStorePort> =
            Arc::new(MemoryProviderSecretStore::new(test_master()));
        let owner = Uuid::new_v4();
        let view = upsert_provider_secret(
            store.clone(),
            owner,
            UpsertProviderSecretRequest {
                purpose: "llm".into(),
                provider: "openai".into(),
                api_key: "sk-revokeme-1234".into(),
                workspace_id: None,
                base_url: None,
                model_hint: None,
            },
        )
        .await
        .unwrap();

        assert!(
            resolve_provider_secret(store.clone(), owner, None, ProviderSecretPurpose::Llm)
                .await
                .unwrap()
                .is_some()
        );

        let revoked = revoke_provider_secret(store.clone(), owner, view.id)
            .await
            .unwrap();
        assert!(revoked.revoked_at.is_some());

        assert!(
            resolve_provider_secret(store.clone(), owner, None, ProviderSecretPurpose::Llm)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            !store
                .has_active(owner, ProviderSecretPurpose::Llm)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn malformed_master_key_fails_closed() {
        assert_eq!(
            ByokMasterKey::parse("").unwrap_err().code(),
            "byok_master_key_missing"
        );
        assert_eq!(
            ByokMasterKey::parse("not-a-key").unwrap_err().code(),
            "byok_master_key_invalid"
        );
    }

    #[tokio::test]
    async fn upsert_replaces_active_same_scope() {
        let store: Arc<dyn ProviderSecretStorePort> =
            Arc::new(MemoryProviderSecretStore::new(test_master()));
        let owner = Uuid::new_v4();
        let first = upsert_provider_secret(
            store.clone(),
            owner,
            UpsertProviderSecretRequest {
                purpose: "llm".into(),
                provider: "deepseek".into(),
                api_key: "sk-old-key-xxxx".into(),
                workspace_id: None,
                base_url: None,
                model_hint: None,
            },
        )
        .await
        .unwrap();
        let second = upsert_provider_secret(
            store.clone(),
            owner,
            UpsertProviderSecretRequest {
                purpose: "llm".into(),
                provider: "deepseek".into(),
                api_key: "sk-new-key-yyyy".into(),
                workspace_id: None,
                base_url: None,
                model_hint: None,
            },
        )
        .await
        .unwrap();
        assert_ne!(first.id, second.id);

        let active = list_provider_secrets(store.clone(), owner, false)
            .await
            .unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, second.id);

        let resolved =
            resolve_provider_secret(store, owner, None, ProviderSecretPurpose::Llm)
                .await
                .unwrap()
                .unwrap();
        assert_eq!(resolved.api_key, "sk-new-key-yyyy");
    }

    #[tokio::test]
    async fn handle_list_never_includes_plaintext() {
        let store: Arc<dyn ProviderSecretStorePort> =
            Arc::new(MemoryProviderSecretStore::new(test_master()));
        let owner = Uuid::new_v4();
        let secret = "sk-handle-list-PLAINTEXT-ZZZZ";
        let _ = handle_upsert_provider_secret(
            store.clone(),
            owner,
            UpsertProviderSecretRequest {
                purpose: "embedding".into(),
                provider: "siliconflow".into(),
                api_key: secret.into(),
                workspace_id: None,
                base_url: None,
                model_hint: None,
            },
        )
        .await;
        let resp = handle_list_provider_secrets(store, owner, false).await;
        assert!(resp.ok);
        let body = serde_json::to_string(&resp.data).unwrap();
        assert!(!body.contains(secret));
        assert!(body.contains(&key_fingerprint(secret)));
    }
}
