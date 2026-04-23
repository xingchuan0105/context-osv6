use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDocumentUploadResponse {
    pub document_id: String,
    pub upload_url: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentStatusResponse {
    pub status: String,
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
    pub status: String,
    pub chunk_count: usize,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentsResponse {
    pub documents: Vec<Document>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentContentResponse {
    pub content: String,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDocumentRequest {
    pub filename: String,
    pub file_size: u64,
    pub mime_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedPreviewItem {
    pub kind: String,
    pub text: String,
    pub page: usize,
    pub cursor: usize,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationLookupRequest {
    pub session_id: String,
    pub message_id: i64,
    pub citation_id: i64,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnswerContextChunk {
    pub chunk_id: String,
    #[serde(default)]
    pub doc_id: Option<String>,
    pub chunk_type: String,
    #[serde(default)]
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
