use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Notebook {
    pub id: String,
    pub org_id: String,
    pub owner_id: String,
    pub name: String,
    pub title: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    #[typeshare(serialized_as = "number")]
    pub document_count: i64,
    #[serde(default)]
    #[typeshare(serialized_as = "Record<string, number>")]
    pub status_summary: HashMap<String, i64>,
    #[serde(default)]
    pub shared: bool,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotebookListResponse {
    pub notebooks: Vec<Notebook>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotebookResponse {
    pub notebook: Notebook,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CreateNotebookRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct UpdateNotebookRequest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ChatSession {
    pub id: String,
    pub notebook_id: String,
    #[serde(default)]
    pub title: Option<String>,
    pub agent_type: String,
    #[serde(default)]
    pub pinned: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ChatSessionListResponse {
    pub sessions: Vec<ChatSession>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CreateChatSessionRequest {
    pub notebook_id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default = "default_rag_agent")]
    pub agent_type: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, Default, utoipa::ToSchema)]
pub struct UpdateChatSessionRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub pinned: Option<bool>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ApiKeyRow {
    pub id: String,
    pub org_id: String,
    pub notebook_id: String,
    pub key_prefix: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub rate_limit_rpm: u32,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub last_used_at: Option<String>,
    pub is_active: bool,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ApiKeyListResponse {
    pub api_keys: Vec<ApiKeyRow>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CreateApiKeyRequest {
    pub name: String,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub rate_limit_rpm: Option<u32>,
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CreateApiKeyResponse {
    pub api_key: ApiKeyRow,
    pub plaintext_key: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotebookNote {
    pub id: String,
    pub notebook_id: String,
    pub title: String,
    pub content: String,
    pub preview: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub promoted_document_id: Option<String>,
    #[serde(default)]
    pub promoted_at: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotebookNoteListResponse {
    pub notes: Vec<NotebookNote>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotebookNoteResponse {
    pub note: NotebookNote,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, Default, utoipa::ToSchema)]
pub struct CreateNotebookNoteRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, Default, utoipa::ToSchema)]
pub struct UpdateNotebookNoteRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PromoteNotebookNoteResponse {
    pub note: NotebookNote,
    pub source_id: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotebookAnalysisOverview {
    pub title: String,
    pub description: String,
    pub updated_at: String,
    #[typeshare(serialized_as = "number")]
    pub document_count: i64,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotebookAnalysisSources {
    #[typeshare(serialized_as = "number")]
    pub total: i64,
    #[typeshare(serialized_as = "number")]
    pub ready: i64,
    #[typeshare(serialized_as = "number")]
    pub processing: i64,
    #[typeshare(serialized_as = "number")]
    pub failed: i64,
    #[typeshare(serialized_as = "number")]
    pub selected: i64,
    #[typeshare(serialized_as = "number")]
    pub pinned: i64,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotebookAnalysisThreads {
    #[typeshare(serialized_as = "number")]
    pub total: i64,
    #[typeshare(serialized_as = "number")]
    pub pinned: i64,
    #[serde(default)]
    pub latest_activity_at: Option<String>,
    #[serde(default)]
    pub latest_mode: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotebookAnalysisNotes {
    #[typeshare(serialized_as = "number")]
    pub total: i64,
    #[serde(default)]
    pub latest_edited_at: Option<String>,
    #[typeshare(serialized_as = "number")]
    pub promoted_to_source: i64,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotebookAnalysisAccess {
    pub share_enabled: bool,
    #[typeshare(serialized_as = "number")]
    pub member_count: i64,
    #[typeshare(serialized_as = "number")]
    pub active_api_key_count: i64,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotebookAnalysisAlert {
    pub level: String,
    pub code: String,
    pub message: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotebookAnalysisResponse {
    pub overview: NotebookAnalysisOverview,
    pub sources: NotebookAnalysisSources,
    pub threads: NotebookAnalysisThreads,
    pub notes: NotebookAnalysisNotes,
    pub access: NotebookAnalysisAccess,
    pub alerts: Vec<NotebookAnalysisAlert>,
}

fn default_rag_agent() -> String {
    "rag".to_string()
}
