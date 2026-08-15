//! Postgres adapter for desktop relay tokens (2026-08-15 W2).
//!
//! Only sha256 hashes are persisted; `resolve_by_hash` is the relay auth path
//! and must fail closed on errors (callers map store errors to 5xx, never allow).

use std::sync::Arc;

use crate::adapters::pg_session::begin_super_admin_tx_sqlx;
use app_core::{DesktopTokenIdentity, DesktopTokenStorePort, DesktopTokenView};
use async_trait::async_trait;
use avrag_storage_pg::PgAppRepository;
use chrono::{DateTime, Utc};
use common::AppError;
use sqlx::Row;
use uuid::Uuid;

pub struct PgDesktopTokenStoreAdapter {
    repo: Arc<PgAppRepository>,
}

impl PgDesktopTokenStoreAdapter {
    pub fn new(repo: Arc<PgAppRepository>) -> Self {
        Self { repo }
    }
}

fn map_sqlx(error: sqlx::Error) -> AppError {
    AppError::internal(error.to_string())
}

const VIEW_COLS: &str =
    "id, owner_user_id, name, prefix, created_at, last_used_at, revoked_at";

fn view_from_row(row: &sqlx::postgres::PgRow) -> Result<DesktopTokenView, AppError> {
    Ok(DesktopTokenView {
        id: row.try_get("id").map_err(map_sqlx)?,
        owner_user_id: row.try_get("owner_user_id").map_err(map_sqlx)?,
        name: row.try_get("name").map_err(map_sqlx)?,
        prefix: row.try_get("prefix").map_err(map_sqlx)?,
        created_at: row
            .try_get::<DateTime<Utc>, _>("created_at")
            .map_err(map_sqlx)?,
        last_used_at: row
            .try_get::<Option<DateTime<Utc>>, _>("last_used_at")
            .map_err(map_sqlx)?,
        revoked_at: row
            .try_get::<Option<DateTime<Utc>>, _>("revoked_at")
            .map_err(map_sqlx)?,
    })
}

#[async_trait]
impl DesktopTokenStorePort for PgDesktopTokenStoreAdapter {
    async fn insert(
        &self,
        owner_user_id: Uuid,
        name: &str,
        token_hash: &str,
        prefix: &str,
    ) -> Result<DesktopTokenView, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;
        let row = sqlx::query(&format!(
            "INSERT INTO desktop_tokens (owner_user_id, name, token_hash, prefix) \
             VALUES ($1, $2, $3, $4) \
             RETURNING {VIEW_COLS}"
        ))
        .bind(owner_user_id)
        .bind(name)
        .bind(token_hash)
        .bind(prefix)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;

        tracing::info!(
            target: "desktop_tokens",
            token_id = %row.try_get::<Uuid, _>("id").unwrap_or_default(),
            owner_user_id = %owner_user_id,
            name = %name,
            prefix = %prefix,
            "desktop token minted"
        );

        view_from_row(&row)
    }

    async fn list(&self, owner_user_id: Uuid) -> Result<Vec<DesktopTokenView>, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;
        let rows = sqlx::query(&format!(
            "SELECT {VIEW_COLS} FROM desktop_tokens \
             WHERE owner_user_id = $1 \
             ORDER BY created_at DESC"
        ))
        .bind(owner_user_id)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        rows.iter().map(view_from_row).collect()
    }

    async fn revoke(&self, owner_user_id: Uuid, id: Uuid) -> Result<DesktopTokenView, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;
        let row = sqlx::query(&format!(
            "UPDATE desktop_tokens \
             SET revoked_at = COALESCE(revoked_at, NOW()) \
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
                "desktop_token_not_found",
                "desktop token not found",
            ));
        };

        tracing::info!(
            target: "desktop_tokens",
            token_id = %id,
            owner_user_id = %owner_user_id,
            "desktop token revoked"
        );

        view_from_row(&row)
    }

    async fn resolve_by_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<DesktopTokenIdentity>, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;
        let row = sqlx::query(
            "SELECT id, owner_user_id FROM desktop_tokens \
             WHERE token_hash = $1 AND revoked_at IS NULL \
             LIMIT 1",
        )
        .bind(token_hash)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;

        row.map(|row| {
            Ok(DesktopTokenIdentity {
                id: row.try_get("id").map_err(map_sqlx)?,
                owner_user_id: row.try_get("owner_user_id").map_err(map_sqlx)?,
            })
        })
        .transpose()
    }

    async fn touch_last_used(&self, id: Uuid) -> Result<(), AppError> {
        sqlx::query("UPDATE desktop_tokens SET last_used_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(self.repo.raw())
            .await
            .map_err(map_sqlx)?;
        Ok(())
    }
}
