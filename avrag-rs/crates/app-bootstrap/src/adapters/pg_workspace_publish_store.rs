//! Postgres adapter for workspace publish mapping (ADR-0010 B3b).

use std::sync::Arc;

use crate::adapters::pg_session::begin_super_admin_tx_sqlx;
use app_core::{PublishStatus, WorkspacePublishRow, WorkspacePublishStorePort};
use async_trait::async_trait;
use avrag_storage_pg::PgAppRepository;
use chrono::{DateTime, Utc};
use common::AppError;
use sqlx::Row;
use uuid::Uuid;

pub struct PgWorkspacePublishStoreAdapter {
    repo: Arc<PgAppRepository>,
}

impl PgWorkspacePublishStoreAdapter {
    pub fn new(repo: Arc<PgAppRepository>) -> Self {
        Self { repo }
    }
}

fn map_sqlx(error: sqlx::Error) -> AppError {
    AppError::internal(error.to_string())
}

const ROW_COLS: &str = "id, owner_user_id, cloud_workspace_id, local_workspace_id, upload_id, \
     status, embedding_model_id, vector_dim, expected_parts, last_published_at, error";

fn row_from(row: &sqlx::postgres::PgRow) -> Result<WorkspacePublishRow, AppError> {
    let status: String = row.try_get("status").map_err(map_sqlx)?;
    Ok(WorkspacePublishRow {
        id: row.try_get("id").map_err(map_sqlx)?,
        owner_user_id: row.try_get("owner_user_id").map_err(map_sqlx)?,
        cloud_workspace_id: row.try_get("cloud_workspace_id").map_err(map_sqlx)?,
        local_workspace_id: row.try_get("local_workspace_id").map_err(map_sqlx)?,
        upload_id: row.try_get("upload_id").map_err(map_sqlx)?,
        status: PublishStatus::parse(&status),
        embedding_model_id: row.try_get("embedding_model_id").map_err(map_sqlx)?,
        vector_dim: row.try_get("vector_dim").map_err(map_sqlx)?,
        expected_parts: row.try_get("expected_parts").map_err(map_sqlx)?,
        last_published_at: row
            .try_get::<Option<DateTime<Utc>>, _>("last_published_at")
            .map_err(map_sqlx)?,
        error: row.try_get("error").map_err(map_sqlx)?,
    })
}

#[async_trait]
impl WorkspacePublishStorePort for PgWorkspacePublishStoreAdapter {
    async fn get_by_local(
        &self,
        owner_user_id: Uuid,
        local_workspace_id: Uuid,
    ) -> Result<Option<WorkspacePublishRow>, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;
        let row = sqlx::query(&format!(
            "SELECT {ROW_COLS} FROM workspace_publish \
             WHERE owner_user_id = $1 AND local_workspace_id = $2"
        ))
        .bind(owner_user_id)
        .bind(local_workspace_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        row.as_ref().map(row_from).transpose()
    }

    async fn get_by_upload(
        &self,
        owner_user_id: Uuid,
        upload_id: Uuid,
    ) -> Result<Option<WorkspacePublishRow>, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;
        let row = sqlx::query(&format!(
            "SELECT {ROW_COLS} FROM workspace_publish \
             WHERE owner_user_id = $1 AND upload_id = $2"
        ))
        .bind(owner_user_id)
        .bind(upload_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        row.as_ref().map(row_from).transpose()
    }

    async fn upsert_session(
        &self,
        row: &WorkspacePublishRow,
    ) -> Result<WorkspacePublishRow, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;
        let fetched = sqlx::query(&format!(
            "INSERT INTO workspace_publish (
                id, owner_user_id, cloud_workspace_id, local_workspace_id, upload_id,
                status, embedding_model_id, vector_dim, expected_parts, last_published_at, error
             ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
             ON CONFLICT (owner_user_id, local_workspace_id) DO UPDATE SET
                cloud_workspace_id = EXCLUDED.cloud_workspace_id,
                upload_id = EXCLUDED.upload_id,
                status = EXCLUDED.status,
                embedding_model_id = EXCLUDED.embedding_model_id,
                vector_dim = EXCLUDED.vector_dim,
                expected_parts = EXCLUDED.expected_parts,
                error = EXCLUDED.error,
                updated_at = NOW()
             RETURNING {ROW_COLS}"
        ))
        .bind(row.id)
        .bind(row.owner_user_id)
        .bind(row.cloud_workspace_id)
        .bind(row.local_workspace_id)
        .bind(row.upload_id)
        .bind(row.status.as_str())
        .bind(&row.embedding_model_id)
        .bind(row.vector_dim)
        .bind(row.expected_parts)
        .bind(row.last_published_at)
        .bind(&row.error)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        row_from(&fetched)
    }

    async fn mark_status(
        &self,
        owner_user_id: Uuid,
        local_workspace_id: Uuid,
        status: PublishStatus,
        error: Option<&str>,
        last_published_at: Option<DateTime<Utc>>,
    ) -> Result<WorkspacePublishRow, AppError> {
        let pool = self.repo.raw();
        let mut tx = begin_super_admin_tx_sqlx(pool).await.map_err(map_sqlx)?;
        let fetched = sqlx::query(&format!(
            "UPDATE workspace_publish SET
                status = $3,
                error = $4,
                last_published_at = COALESCE($5, last_published_at),
                updated_at = NOW()
             WHERE owner_user_id = $1 AND local_workspace_id = $2
             RETURNING {ROW_COLS}"
        ))
        .bind(owner_user_id)
        .bind(local_workspace_id)
        .bind(status.as_str())
        .bind(error)
        .bind(last_published_at)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        let Some(fetched) = fetched else {
            return Err(AppError::not_found(
                "publish_mapping_not_found",
                "publish mapping not found",
            ));
        };
        row_from(&fetched)
    }
}
