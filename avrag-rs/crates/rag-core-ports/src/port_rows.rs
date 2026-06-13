//! Row types referenced by chat persistence port methods.

use chrono::{DateTime, Utc};
use contracts::OrgId;
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
