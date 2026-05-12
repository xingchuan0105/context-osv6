use avrag_auth::{ActorId, AuthContext, AuthError, OrgId};
use chrono::{DateTime, Utc};
use common::{
    ApiKeyRow, ChatMessage, ChatSession, Citation, Document, DocumentContentResponse,
    DocumentStatus, Notebook, NotificationRow, ParsedPreviewItem, ParsedPreviewResponse, SourceRow,
};
use ingestion::{
    AuditRecord, IngestionTask, IngestionTaskKind, IngestionTaskPayload, TaskCompletionOutcome,
    TaskFailureOutcome,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{
    PgPool, Postgres, Row, Transaction,
    postgres::{PgConnection, PgPoolOptions, PgRow},
};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

pub use object_store::{
    LocalObjectStore, ObjectStoreHandle, ObjectStoreHeadError, ObjectStoreMetadata, S3ObjectStore,
};

#[derive(Debug, Clone)]
pub struct TenantPgPool {
    pool: PgPool,
}

impl TenantPgPool {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn raw(&self) -> &PgPool {
        &self.pool
    }

    pub async fn begin<'a>(
        &'a self,
        context: &AuthContext,
    ) -> Result<TenantTransaction<'a>, PgStorageError> {
        let mut tx = self.pool.begin().await?;
        let org_id = context.org_id();
        sqlx::query("select set_config('app.current_org', $1, true)")
            .bind(org_id.to_string())
            .execute(&mut *tx)
            .await?;

        Ok(TenantTransaction { tx, org_id })
    }
}

pub struct TenantTransaction<'a> {
    tx: Transaction<'a, Postgres>,
    org_id: OrgId,
}

impl<'a> TenantTransaction<'a> {
    pub fn org_id(&self) -> OrgId {
        self.org_id
    }

    pub fn inner(&mut self) -> &mut PgConnection {
        self.tx.as_mut()
    }

    pub async fn commit(self) -> Result<(), PgStorageError> {
        self.tx.commit().await?;
        Ok(())
    }

    pub async fn rollback(self) -> Result<(), PgStorageError> {
        self.tx.rollback().await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct NotificationCreateParams {
    pub user_id: Uuid,
    pub event_type: String,
    pub title: String,
    pub body: String,
    pub data: serde_json::Value,
    pub channels: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ValidatedApiKey {
    pub id: Uuid,
    pub org_id: OrgId,
    pub notebook_id: Option<Uuid>,
    pub permissions: Vec<String>,
    pub created_by: Option<Uuid>,
    pub rate_limit_rpm: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfileRow {
    pub user_id: Uuid,
    pub org_id: OrgId,
    pub expertise_domains: Vec<String>,
    pub preferred_answer_style: Option<String>,
    pub frequently_asked_topics: Vec<String>,
    pub custom_preferences: serde_json::Value,
    pub structured_profile: serde_json::Value,
    pub inferred_at: DateTime<Utc>,
    pub inference_version: String,
}

#[derive(Debug, Clone)]
pub struct PgAppRepository {
    pool: TenantPgPool,
}
