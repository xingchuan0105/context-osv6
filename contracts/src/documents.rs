use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocumentStatus {
    Pending,
    Enqueueing,
    Queued,
    Processing,
    Completed,
    Failed,
    Deleting,
    Deleted,
    #[serde(rename = "upload_invalid")]
    UploadInvalid,
}

impl DocumentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Enqueueing => "enqueueing",
            Self::Queued => "queued",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Deleting => "deleting",
            Self::Deleted => "deleted",
            Self::UploadInvalid => "upload_invalid",
        }
    }
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDocumentUploadResponse {
    pub document_id: String,
    pub upload_url: String,
    pub status: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentStatusResponse {
    pub status: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub owner_user_id: String,
    /// Workspace binding of the artifact when one exists; session-bound
    /// artifacts (chat-first W2) have none. Binding tables are the scope truth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    pub owner_id: String,
    pub file_name: String,
    pub mime_type: String,
    #[typeshare(serialized_as = "number")]
    pub file_size: u64,
    pub status: String,
    #[typeshare(serialized_as = "number")]
    pub chunk_count: usize,
    pub created_at: String,
    pub updated_at: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentsResponse {
    pub documents: Vec<Document>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentContentResponse {
    pub content: String,
    pub summary: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDocumentRequest {
    pub filename: String,
    #[typeshare(serialized_as = "number")]
    pub file_size: u64,
    pub mime_type: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedPreviewItem {
    pub kind: String,
    pub text: String,
    #[typeshare(serialized_as = "number")]
    pub page: usize,
    #[typeshare(serialized_as = "number")]
    pub cursor: usize,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedPreviewResponse {
    pub items: Vec<ParsedPreviewItem>,
    pub has_more: bool,
    #[typeshare(serialized_as = "number")]
    pub next_cursor: usize,
    #[serde(default)]
    pub summary: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRow {
    pub id: String,
    pub workspace_id: String,
    #[serde(rename = "workspace_name", alias = "workspace_name")]
    pub workspace_name: String,
    pub title: String,
    pub file_name: String,
    pub status: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcesResponse {
    pub sources: Vec<SourceRow>,
}

/// One session-bound artifact as seen from its Conversation (chat-first W2).
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFileRow {
    pub binding_id: String,
    pub document_id: String,
    pub file_name: String,
    pub mime_type: String,
    #[typeshare(serialized_as = "number")]
    pub file_size: u64,
    pub status: String,
    pub created_at: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFilesResponse {
    pub files: Vec<SessionFileRow>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationLookupRequest {
    pub session_id: String,
    #[typeshare(serialized_as = "number")]
    pub message_id: i64,
    #[typeshare(serialized_as = "number")]
    pub citation_id: i64,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationLookupResponse {
    #[serde(default)]
    pub doc_name: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub doc_id: Option<String>,
    #[serde(default)]
    pub chunk_id: Option<String>,
    #[serde(default)]
    #[typeshare(serialized_as = "number")]
    pub page: Option<usize>,
    #[serde(default)]
    pub chunk_type: Option<String>,
    #[serde(default)]
    pub asset_id: Option<String>,
    #[serde(default)]
    pub caption: Option<String>,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub parser_backend: Option<String>,
    #[serde(default)]
    pub source_locator: Option<serde_json::Value>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnswerContextChunk {
    pub chunk_id: String,
    #[serde(default)]
    pub doc_id: Option<String>,
    pub chunk_type: String,
    #[serde(default)]
    #[typeshare(serialized_as = "number")]
    pub page: Option<i64>,
    pub text: String,
    #[serde(default)]
    pub asset_id: Option<String>,
    #[serde(default)]
    pub caption: Option<String>,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub parser_backend: Option<String>,
    #[serde(default)]
    pub source_locator: Option<serde_json::Value>,
}
