use serde::{Deserialize, Serialize};
use uuid::Uuid;

use contracts::documents::DocumentStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub owner_user_id: String,
    #[serde(rename = "workspace_id", alias = "workspace_id")]
    pub workspace_id: String,
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
    #[serde(
        default,
        rename = "workspace_id",
        alias = "workspace_id",
        skip_serializing_if = "Option::is_none"
    )]
    pub workspace_id: Option<String>,
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
pub struct DocumentMetadata {
    pub doc_id: String,
    pub name: String,
    pub mime_type: String,
    pub file_size: u64,
    pub status: DocumentStatus,
    pub chunk_count: usize,
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
    #[serde(rename = "workspace_id", alias = "workspace_id")]
    pub workspace_id: String,
    #[serde(rename = "workspace_name", alias = "workspace_name")]
    pub workspace_name: String,
    pub title: String,
    pub file_name: String,
    pub status: String,
    /// Latest ingestion task error (if any). Surfaces failed parse/index reasons in UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcesResponse {
    pub sources: Vec<SourceRow>,
}

#[derive(Debug, Clone)]
pub struct TocEntry {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub title: String,
    pub heading_level: i32,
    pub page: Option<i32>,
    pub chunk_id: Option<Uuid>,
    pub rank: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_invalid_status_serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&DocumentStatus::UploadInvalid).unwrap(),
            "\"upload_invalid\""
        );
        assert_eq!(DocumentStatus::UploadInvalid.as_str(), "upload_invalid");
    }

    #[test]
    fn deletion_statuses_serialize_as_stable_lowercase_names() {
        assert_eq!(
            serde_json::to_string(&DocumentStatus::Deleting).unwrap(),
            "\"deleting\""
        );
        assert_eq!(
            serde_json::to_string(&DocumentStatus::Deleted).unwrap(),
            "\"deleted\""
        );
        assert_eq!(DocumentStatus::Deleting.as_str(), "deleting");
        assert_eq!(DocumentStatus::Deleted.as_str(), "deleted");
    }
}
