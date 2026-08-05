//! Postgres encrypted provider-secret adapter (ADR-0010 PR7).
//!
//! Encrypts with [`ByokMasterKey`] (AES-256-GCM) before insert; decrypt only on
//! [`ProviderSecretStorePort::resolve`]. Logs never include secret material.

use std::sync::Arc;

use crate::adapters::pg_session::begin_super_admin_tx_sqlx;
use app_core::{
    ByokMasterKey, ProviderSecretPurpose, ProviderSecretStorePort, ProviderSecretView,
    ResolvedProviderSecret, UpsertProviderSecretInput, key_fingerprint,
};
use async_trait::async_trait;
use avrag_storage_pg::PgAppRepository;
use chrono::{DateTime, Utc};
use common::AppError;
use sqlx::Row;
use uuid::Uuid;

pub struct PgProviderSecretStoreAdapter {
    repo: Arc<PgAppRepository>,
    master: ByokMasterKey,
}

impl PgProviderSecretStoreAdapter {
    pub fn new(repo: Arc<PgAppRepository>, master: ByokMasterKey) -> Self {
        Self { repo, master }
    }

    /// Construct from env `BYOK_MASTER_KEY` (fail closed when missing/malformed).
    pub fn from_env(repo: Arc<PgAppRepository>) -> Result<Self, AppError> {
        Ok(Self::new(repo, ByokMasterKey::from_env()?))
    }
}

fn map_sqlx(error: sqlx::Error) -> AppError {
    AppError::internal(error.to_string())
}

fn view_from_row(row: &sqlx::postgres::PgRow) -> Result<ProviderSecretView, AppError> {
    Ok(ProviderSecretView {
        id: row.try_get("id").map_err(map_sqlx)?,
        owner_user_id: row.try_get("owner_user_id").map_err(map_sqlx)?,
        workspace_id: row.try_get("workspace_id").map_err(map_sqlx)?,
        purpose: row.try_get("purpose").map_err(map_sqlx)?,
        provider: row.try_get("provider").map_err(map_sqlx)?,
        base_url: row.try_get("base_url").map_err(map_sqlx)?,
        model_hint: row.try_get("model_hint").map_err(map_sqlx)?,
        key_fingerprint: row.try_get("key_fingerprint").map_err(map_sqlx)?,
        created_at: row
            .try_get::<DateTime<Utc>, _>("created_at")
            .map_err(map_sqlx)?,
        updated_at: row
            .try_get::<DateTime<Utc>, _>("updated_at")
            .map_err(map_sqlx)?,
        revoked_at: row
            .try_get::<Option<DateTime<Utc>>, _>("revoked_at")
            .map_err(map_sqlx)?,
    })
}

const VIEW_COLS: &str = "id, owner_user_id, workspace_id, purpose, provider, base_url, \
     model_hint, key_fingerprint, created_at, updated_at, revoked_at";

fn normalize_provider(raw: &str) -> Result<String, AppError> {
    let p = raw.trim().to_ascii_lowercase();
    if p.is_empty() || p.len() > 64 {
        return Err(AppError::validation(
            "provider_invalid",
            "provider is required and must be ≤ 64 chars",
        ));
    }
    Ok(p)
}

fn validate_api_key(api_key: &str) -> Result<(), AppError> {
    let t = api_key.trim();
    if t.is_empty() {
        return Err(AppError::validation(
            "api_key_required",
            "api_key is required",
        ));
    }
    if t.len() > 4096 {
        return Err(AppError::validation(
            "api_key_too_long",
            "api_key exceeds maximum length",
        ));
    }
    Ok(())
}

#[async_trait]
impl ProviderSecretStorePort for PgProviderSecretStoreAdapter {
    async fn upsert(
        &self,
        input: &UpsertProviderSecretInput,
    ) -> Result<ProviderSecretView, AppError> {
        validate_api_key(&input.api_key)?;
        let provider = normalize_provider(&input.provider)?;
        let purpose = input.purpose.as_str();
        let fingerprint = key_fingerprint(input.api_key.trim());
        let (ciphertext, nonce) = self.master.encrypt(input.api_key.trim().as_bytes())?;

        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;

        // Soft-revoke any active secret for the same scope so the unique index allows insert.
        if let Some(ws) = input.workspace_id {
            sqlx::query(
                "UPDATE user_provider_secrets \
                 SET revoked_at = NOW(), updated_at = NOW() \
                 WHERE owner_user_id = $1 AND workspace_id = $2 AND purpose = $3 \
                   AND revoked_at IS NULL",
            )
            .bind(input.owner_user_id)
            .bind(ws)
            .bind(purpose)
            .execute(tx.as_mut())
            .await
            .map_err(map_sqlx)?;
        } else {
            sqlx::query(
                "UPDATE user_provider_secrets \
                 SET revoked_at = NOW(), updated_at = NOW() \
                 WHERE owner_user_id = $1 AND workspace_id IS NULL AND purpose = $2 \
                   AND revoked_at IS NULL",
            )
            .bind(input.owner_user_id)
            .bind(purpose)
            .execute(tx.as_mut())
            .await
            .map_err(map_sqlx)?;
        }

        let row = sqlx::query(&format!(
            "INSERT INTO user_provider_secrets \
             (owner_user_id, workspace_id, purpose, provider, base_url, model_hint, \
              ciphertext, nonce, key_fingerprint) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
             RETURNING {VIEW_COLS}"
        ))
        .bind(input.owner_user_id)
        .bind(input.workspace_id)
        .bind(purpose)
        .bind(&provider)
        .bind(input.base_url.as_deref())
        .bind(input.model_hint.as_deref())
        .bind(&ciphertext)
        .bind(&nonce)
        .bind(&fingerprint)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_sqlx)?;

        tx.commit().await.map_err(map_sqlx)?;

        tracing::info!(
            target: "byok",
            secret_id = %row.try_get::<Uuid, _>("id").unwrap_or_default(),
            owner_user_id = %input.owner_user_id,
            workspace_id = ?input.workspace_id,
            purpose = %purpose,
            provider = %provider,
            fingerprint = %fingerprint,
            "provider secret upserted"
        );

        view_from_row(&row)
    }

    async fn list(
        &self,
        owner_user_id: Uuid,
        include_revoked: bool,
    ) -> Result<Vec<ProviderSecretView>, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;
        let rows = if include_revoked {
            sqlx::query(&format!(
                "SELECT {VIEW_COLS} FROM user_provider_secrets \
                 WHERE owner_user_id = $1 \
                 ORDER BY updated_at DESC"
            ))
            .bind(owner_user_id)
            .fetch_all(tx.as_mut())
            .await
            .map_err(map_sqlx)?
        } else {
            sqlx::query(&format!(
                "SELECT {VIEW_COLS} FROM user_provider_secrets \
                 WHERE owner_user_id = $1 AND revoked_at IS NULL \
                 ORDER BY updated_at DESC"
            ))
            .bind(owner_user_id)
            .fetch_all(tx.as_mut())
            .await
            .map_err(map_sqlx)?
        };
        tx.commit().await.map_err(map_sqlx)?;
        rows.iter().map(view_from_row).collect()
    }

    async fn revoke(
        &self,
        owner_user_id: Uuid,
        id: Uuid,
    ) -> Result<ProviderSecretView, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;
        let row = sqlx::query(&format!(
            "UPDATE user_provider_secrets \
             SET revoked_at = COALESCE(revoked_at, NOW()), updated_at = NOW() \
             WHERE id = $1 AND owner_user_id = $2 \
             RETURNING {VIEW_COLS}"
        ))
        .bind(id)
        .bind(owner_user_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;

        let Some(row) = row else {
            return Err(AppError::not_found(
                "provider_secret_not_found",
                "provider secret not found",
            ));
        };

        tracing::info!(
            target: "byok",
            secret_id = %id,
            owner_user_id = %owner_user_id,
            "provider secret revoked"
        );

        view_from_row(&row)
    }

    async fn resolve(
        &self,
        owner_user_id: Uuid,
        workspace_id: Option<Uuid>,
        purpose: ProviderSecretPurpose,
    ) -> Result<Option<ResolvedProviderSecret>, AppError> {
        let purpose_s = purpose.as_str();
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;

        // Prefer workspace-scoped secret when workspace_id is provided.
        let row = if let Some(ws) = workspace_id {
            let scoped = sqlx::query(
                "SELECT id, owner_user_id, workspace_id, purpose, provider, base_url, model_hint, \
                        ciphertext, nonce \
                 FROM user_provider_secrets \
                 WHERE owner_user_id = $1 AND workspace_id = $2 AND purpose = $3 \
                   AND revoked_at IS NULL \
                 LIMIT 1",
            )
            .bind(owner_user_id)
            .bind(ws)
            .bind(purpose_s)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_sqlx)?;

            if scoped.is_some() {
                scoped
            } else {
                sqlx::query(
                    "SELECT id, owner_user_id, workspace_id, purpose, provider, base_url, model_hint, \
                            ciphertext, nonce \
                     FROM user_provider_secrets \
                     WHERE owner_user_id = $1 AND workspace_id IS NULL AND purpose = $2 \
                       AND revoked_at IS NULL \
                     LIMIT 1",
                )
                .bind(owner_user_id)
                .bind(purpose_s)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_sqlx)?
            }
        } else {
            sqlx::query(
                "SELECT id, owner_user_id, workspace_id, purpose, provider, base_url, model_hint, \
                        ciphertext, nonce \
                 FROM user_provider_secrets \
                 WHERE owner_user_id = $1 AND workspace_id IS NULL AND purpose = $2 \
                   AND revoked_at IS NULL \
                 LIMIT 1",
            )
            .bind(owner_user_id)
            .bind(purpose_s)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_sqlx)?
        };
        tx.commit().await.map_err(map_sqlx)?;

        let Some(row) = row else {
            return Ok(None);
        };

        let ciphertext: Vec<u8> = row.try_get("ciphertext").map_err(map_sqlx)?;
        let nonce: Vec<u8> = row.try_get("nonce").map_err(map_sqlx)?;
        let plain = self.master.decrypt(&ciphertext, &nonce)?;
        let api_key = String::from_utf8(plain).map_err(|_| {
            AppError::internal("byok decrypted payload is not utf-8")
        })?;

        Ok(Some(ResolvedProviderSecret {
            id: row.try_get("id").map_err(map_sqlx)?,
            owner_user_id: row.try_get("owner_user_id").map_err(map_sqlx)?,
            workspace_id: row.try_get("workspace_id").map_err(map_sqlx)?,
            purpose,
            provider: row.try_get("provider").map_err(map_sqlx)?,
            base_url: row.try_get("base_url").map_err(map_sqlx)?,
            model_hint: row.try_get("model_hint").map_err(map_sqlx)?,
            api_key,
        }))
    }

    async fn has_active(
        &self,
        owner_user_id: Uuid,
        purpose: ProviderSecretPurpose,
    ) -> Result<bool, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS( \
                 SELECT 1 FROM user_provider_secrets \
                 WHERE owner_user_id = $1 AND purpose = $2 AND revoked_at IS NULL \
             )",
        )
        .bind(owner_user_id)
        .bind(purpose.as_str())
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(exists)
    }
}
