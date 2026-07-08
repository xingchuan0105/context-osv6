use super::*;
pub use avrag_auth::{ActorId, AuthContext, AuthError, OrgId};
pub use chrono::{DateTime, Utc};
pub use common::{
    merge_search_tokens, rrf_merge, segment_for_fts, ApiKeyRow, Document, DocumentContentResponse,
    NotificationRow, ParsedPreviewItem, ParsedPreviewResponse, SourceRow,
};
pub use contracts::chat::{ChatMessage, Citation};
pub use contracts::documents::{DocumentStatus};
pub use contracts::notebooks::{ChatSession, Notebook};
pub use ingestion_types::{
    AuditRecord, IngestionTask, IngestionTaskKind, IngestionTaskPayload, TaskCompletionOutcome,
    TaskFailureOutcome,
};
pub use serde::{Deserialize, Serialize};
pub use serde_json::json;
pub use sha2::{Digest, Sha256};
pub use sqlx::{
    PgPool, Postgres, Row, Transaction,
    postgres::{PgConnection, PgPoolOptions, PgRow},
};
pub use std::collections::HashMap;
pub use thiserror::Error;
pub use uuid::Uuid;

pub use crate::object_store::{
    LocalObjectStore, ObjectStoreHandle, ObjectStoreHeadError, ObjectStoreMetadata, S3ObjectStore,
};

#[derive(Debug, Clone)]
pub struct TenantPgPool {
    pub pool: PgPool,
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
    pub pool: TenantPgPool,
}

macro_rules! define_repository {
    ($name:ident) => {
        #[derive(Debug, Clone)]
        pub struct $name {
            pub(crate) pool: TenantPgPool,
        }
    };
}
define_repository!(AuthRepository);
define_repository!(AssetRepository);
define_repository!(BootstrapRepository);
define_repository!(IngestionQueueRepository);
define_repository!(ConversationMemoryRepository);
define_repository!(DocumentRepository);
define_repository!(ChunkRepository);
define_repository!(SessionRepository);
define_repository!(AuditRepository);

impl PgAppRepository {
    pub fn raw(&self) -> &PgPool { self.pool.raw() }
    pub fn from_pool(pool: PgPool) -> Self { Self { pool: TenantPgPool::new(pool) } }
    pub fn auth(&self) -> AuthRepository { AuthRepository { pool: self.pool.clone() } }
    pub fn assets(&self) -> AssetRepository { AssetRepository { pool: self.pool.clone() } }
    pub fn bootstrap(&self) -> BootstrapRepository { BootstrapRepository { pool: self.pool.clone() } }
    pub fn ingestion_queue(&self) -> IngestionQueueRepository { IngestionQueueRepository { pool: self.pool.clone() } }
    pub fn conversation_memory(&self) -> ConversationMemoryRepository { ConversationMemoryRepository { pool: self.pool.clone() } }
    pub fn documents(&self) -> DocumentRepository { DocumentRepository { pool: self.pool.clone() } }
    pub fn chunks(&self) -> ChunkRepository { ChunkRepository { pool: self.pool.clone() } }
    pub fn sessions(&self) -> SessionRepository { SessionRepository { pool: self.pool.clone() } }
    pub fn audit(&self) -> AuditRepository { AuditRepository { pool: self.pool.clone() } }
}
