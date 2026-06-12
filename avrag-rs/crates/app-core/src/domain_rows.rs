//! Domain row types shared across ports (mapped from storage adapters).

use avrag_auth::OrgId;
use chrono::{DateTime, Utc};
use contracts::documents::DocumentStatus;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentAssetRow {
    pub asset_id: Uuid,
    pub org_id: Uuid,
    pub notebook_id: Uuid,
    pub document_id: Uuid,
    pub parse_run_id: Option<Uuid>,
    pub page: Option<i32>,
    pub asset_kind: String,
    pub storage_path: Option<String>,
    pub mime_type: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub caption: Option<String>,
    pub parser_backend: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultimodalChunkRow {
    pub chunk_id: Uuid,
    pub org_id: Uuid,
    pub notebook_id: Uuid,
    pub document_id: Uuid,
    pub parse_run_id: Option<Uuid>,
    pub asset_id: Option<Uuid>,
    pub page: Option<i32>,
    pub context_text: Option<String>,
    pub caption: Option<String>,
    pub normalized_text: String,
    pub parser_backend: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct DocumentTaskSeed {
    pub document_id: String,
    pub org_id: String,
    pub notebook_id: String,
    pub filename: String,
    pub mime_type: String,
    pub file_size: u64,
    pub object_path: String,
    pub status: DocumentStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentUploadMutationOutcome {
    Updated,
    NotFound,
    StatusConflict(DocumentStatus),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentUploadQueueOutcome {
    Queued { task_inserted: bool },
    NotFound,
    StatusConflict(DocumentStatus),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentDeletionOutcome {
    Queued { task_inserted: bool },
    AlreadyDeleting { task_inserted: bool },
    AlreadyDeleted,
    NotFound,
}

#[derive(Debug, Clone)]
pub struct DocumentScopeState {
    pub document_id: Uuid,
    pub status: DocumentStatus,
}

#[derive(Debug, Clone)]
pub struct IndexedChunk {
    pub chunk_id: String,
    pub doc_id: String,
    pub page: Option<i64>,
    pub content: String,
    pub score: Option<f32>,
    pub metadata: serde_json::Value,
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
pub struct TaggedMessage {
    pub message_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub tags: Vec<String>,
}
