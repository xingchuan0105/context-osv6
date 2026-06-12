use std::sync::Arc;

use async_trait::async_trait;
use avrag_auth::AuthContext;
use common::{ContentStore, ContentStoreError, IndexedChunk};
use avrag_storage_pg::{IndexedChunk as PgIndexedChunk, PgAppRepository, PgStorageError};
use common::{Document, DocumentMetadata, SummaryMetadata, TocEntry};
use uuid::Uuid;

#[derive(Clone)]
pub struct PgContentStore {
    repo: Arc<PgAppRepository>,
}

impl PgContentStore {
    pub fn new(repo: Arc<PgAppRepository>) -> Self {
        Self { repo }
    }
}

fn map_pg_error(error: PgStorageError) -> ContentStoreError {
    match error {
        PgStorageError::NotFound(message) => ContentStoreError::NotFound(message),
        other => ContentStoreError::Internal(other.to_string()),
    }
}

fn map_indexed_chunk(chunk: PgIndexedChunk) -> IndexedChunk {
    IndexedChunk {
        chunk_id: chunk.chunk_id,
        doc_id: chunk.doc_id,
        page: chunk.page,
        content: chunk.content,
        score: chunk.score,
        metadata: chunk.metadata,
    }
}

#[async_trait]
impl ContentStore for PgContentStore {
    async fn get_chunks_by_ids(
        &self,
        auth: &AuthContext,
        chunk_ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, IndexedChunk>, ContentStoreError> {
        self.repo
            .get_chunks_by_ids(auth, chunk_ids)
            .await
            .map(|chunks| {
                chunks
                    .into_iter()
                    .map(|(id, chunk)| (id, map_indexed_chunk(chunk)))
                    .collect()
            })
            .map_err(map_pg_error)
    }

    async fn get_document_metadata_by_ids(
        &self,
        auth: &AuthContext,
        doc_ids: &[Uuid],
    ) -> Result<Vec<DocumentMetadata>, ContentStoreError> {
        self.repo
            .get_document_metadata_by_ids(auth, doc_ids)
            .await
            .map_err(map_pg_error)
    }

    async fn get_summary_metadata(
        &self,
        auth: &AuthContext,
        doc_ids: &[Uuid],
    ) -> Result<Vec<SummaryMetadata>, ContentStoreError> {
        self.repo
            .get_summary_metadata(auth, doc_ids)
            .await
            .map_err(map_pg_error)
    }

    async fn get_document_toc_entries(
        &self,
        auth: &AuthContext,
        doc_ids: &[Uuid],
    ) -> Result<Vec<(Uuid, TocEntry)>, ContentStoreError> {
        self.repo
            .get_document_toc_entries(auth, doc_ids)
            .await
            .map_err(map_pg_error)
    }

    async fn get_summary_chunks(
        &self,
        auth: &AuthContext,
        doc_ids: &[Uuid],
    ) -> Result<Vec<(Uuid, String)>, ContentStoreError> {
        self.repo
            .get_summary_chunks(auth, doc_ids)
            .await
            .map_err(map_pg_error)
    }

    async fn list_documents(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
        document_id: Option<Uuid>,
    ) -> Result<Vec<Document>, ContentStoreError> {
        self.repo
            .list_documents(auth, notebook_id, document_id)
            .await
            .map_err(map_pg_error)
    }

    async fn get_document_names(
        &self,
        auth: &AuthContext,
        doc_ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, String>, ContentStoreError> {
        self.repo
            .get_document_names(auth, doc_ids)
            .await
            .map_err(map_pg_error)
    }
}
