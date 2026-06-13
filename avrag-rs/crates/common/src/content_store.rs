use async_trait::async_trait;
use contracts::AuthContext;
use uuid::Uuid;

use crate::{Document, DocumentMetadata, SummaryMetadata, TocEntry};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IndexedChunk {
    pub chunk_id: String,
    pub doc_id: String,
    pub page: Option<i64>,
    pub content: String,
    pub score: Option<f32>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, thiserror::Error)]
pub enum ContentStoreError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("internal: {0}")]
    Internal(String),
}

#[async_trait]
pub trait ContentStore: Send + Sync {
    async fn get_chunks_by_ids(
        &self,
        auth: &AuthContext,
        chunk_ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, IndexedChunk>, ContentStoreError>;

    async fn get_document_metadata_by_ids(
        &self,
        auth: &AuthContext,
        doc_ids: &[Uuid],
    ) -> Result<Vec<DocumentMetadata>, ContentStoreError>;

    async fn get_summary_metadata(
        &self,
        auth: &AuthContext,
        doc_ids: &[Uuid],
    ) -> Result<Vec<SummaryMetadata>, ContentStoreError>;

    async fn get_document_toc_entries(
        &self,
        auth: &AuthContext,
        doc_ids: &[Uuid],
    ) -> Result<Vec<(Uuid, TocEntry)>, ContentStoreError>;

    async fn get_summary_chunks(
        &self,
        auth: &AuthContext,
        doc_ids: &[Uuid],
    ) -> Result<Vec<(Uuid, String)>, ContentStoreError>;

    async fn list_documents(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
        document_id: Option<Uuid>,
    ) -> Result<Vec<Document>, ContentStoreError>;

    async fn get_document_names(
        &self,
        auth: &AuthContext,
        doc_ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, String>, ContentStoreError>;
}
