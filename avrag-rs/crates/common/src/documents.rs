use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocumentStatus {
    Pending,
    Enqueueing,
    Queued,
    Processing,
    Completed,
    Failed,
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
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub org_id: String,
    pub notebook_id: String,
    pub owner_id: String,
    pub file_name: String,
    pub mime_type: String,
    pub file_size: u64,
    pub status: DocumentStatus,
    pub chunk_count: usize,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentsResponse {
    pub documents: Vec<Document>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDocumentRequest {
    pub filename: String,
    pub file_size: u64,
    pub mime_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDocumentRequest {
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub notebook_id: Option<String>,
    #[serde(default)]
    pub status: Option<DocumentStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusOnlyResponse {
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentContentResponse {
    pub content: String,
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedPreviewResponse {
    pub items: Vec<ParsedPreviewItem>,
    pub has_more: bool,
    pub next_cursor: usize,
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedPreviewItem {
    pub kind: String,
    pub text: String,
    pub page: usize,
    pub cursor: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddUrlSourceRequest {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRow {
    pub id: String,
    pub notebook_id: String,
    pub notebook_name: String,
    pub title: String,
    pub file_name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcesResponse {
    pub sources: Vec<SourceRow>,
}
