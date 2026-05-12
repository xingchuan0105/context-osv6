use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    pub document_count: i64,
    #[serde(default)]
    pub status_summary: HashMap<String, i64>,
    #[serde(default)]
    pub shared: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotebookListResponse {
    pub notebooks: Vec<Notebook>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotebookResponse {
    pub notebook: Notebook,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CreateNotebookRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct UpdateNotebookRequest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ChatSession {
    pub id: String,
    pub notebook_id: String,
    #[serde(default)]
    pub title: Option<String>,
    pub agent_type: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub pinned: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ChatSessionListResponse {
    pub sessions: Vec<ChatSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CreateChatSessionRequest {
    pub notebook_id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default = "default_rag_agent")]
    pub agent_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, utoipa::ToSchema)]
pub struct UpdateChatSessionRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub pinned: Option<bool>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ApiKeyListResponse {
    pub api_keys: Vec<ApiKeyRow>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CreateApiKeyResponse {
    pub api_key: ApiKeyRow,
    pub plaintext_key: String,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotebookNoteListResponse {
    pub notes: Vec<NotebookNote>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotebookNoteResponse {
    pub note: NotebookNote,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, utoipa::ToSchema)]
pub struct CreateNotebookNoteRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, utoipa::ToSchema)]
pub struct UpdateNotebookNoteRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PromoteNotebookNoteResponse {
    pub note: NotebookNote,
    pub source_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotebookAnalysisOverview {
    pub title: String,
    pub description: String,
    pub updated_at: String,
    pub document_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotebookAnalysisSources {
    pub total: i64,
    pub ready: i64,
    pub processing: i64,
    pub failed: i64,
    pub selected: i64,
    pub pinned: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotebookAnalysisThreads {
    pub total: i64,
    pub pinned: i64,
    #[serde(default)]
    pub latest_activity_at: Option<String>,
    #[serde(default)]
    pub latest_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotebookAnalysisNotes {
    pub total: i64,
    #[serde(default)]
    pub latest_edited_at: Option<String>,
    pub promoted_to_source: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotebookAnalysisAccess {
    pub share_enabled: bool,
    pub member_count: i64,
    pub active_api_key_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotebookAnalysisAlert {
    pub level: String,
    pub code: String,
    pub message: String,
}

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
