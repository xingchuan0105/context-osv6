use std::sync::Arc;

use async_trait::async_trait;
use app_core::{
    map_pg_error, domain_rows::DocumentDeletionOutcome,
    domain_rows::DocumentScopeState, domain_rows::DocumentTaskSeed,
    domain_rows::DocumentUploadMutationOutcome, domain_rows::DocumentUploadQueueOutcome,
    DocumentStorePort,
};
use avrag_auth::AuthContext;
use avrag_storage_pg::PgAppRepository;
use common::{
    AppError, Document, DocumentContentResponse, DocumentStatus, Notebook, ParsedPreviewResponse,
    SourceRow,
};
use ingestion_types::{AuditRecord, IngestionTask};
use uuid::Uuid;

pub struct PgDocumentStoreAdapter {
    repo: Arc<PgAppRepository>,
}

impl PgDocumentStoreAdapter {
    pub fn new(repo: Arc<PgAppRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl DocumentStorePort for PgDocumentStoreAdapter {
    async fn list_notebooks(&self, auth: &AuthContext) -> Result<Vec<Notebook>, AppError> {
        self.repo.list_notebooks(auth).await.map_err(map_pg_error)
    }

    async fn get_notebook(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
    ) -> Result<Option<Notebook>, AppError> {
        self.repo
            .get_notebook(auth, notebook_id)
            .await
            .map_err(map_pg_error)
    }

    async fn create_notebook(
        &self,
        auth: &AuthContext,
        name: &str,
        description: &str,
    ) -> Result<Notebook, AppError> {
        self.repo
            .create_notebook(auth, name, description)
            .await
            .map_err(map_pg_error)
    }

    async fn update_notebook(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        name: Option<&str>,
        description: Option<&str>,
    ) -> Result<Option<Notebook>, AppError> {
        let current = self
            .repo
            .get_notebook(auth, notebook_id)
            .await
            .map_err(map_pg_error)?;
        let Some(notebook) = current else {
            return Ok(None);
        };
        let name = name.unwrap_or(notebook.name.as_str());
        let description = description.unwrap_or(notebook.description.as_str());
        self.repo
            .update_notebook(auth, notebook_id, name, description)
            .await
            .map_err(map_pg_error)
    }

    async fn delete_notebook(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
    ) -> Result<bool, AppError> {
        self.repo
            .delete_notebook(auth, notebook_id)
            .await
            .map_err(map_pg_error)
    }

    async fn get_document_scope_states(
        &self,
        auth: &AuthContext,
        document_ids: &[Uuid],
    ) -> Result<Vec<DocumentScopeState>, AppError> {
        self.repo
            .get_document_scope_states(auth, document_ids)
            .await
            .map_err(map_pg_error)
    }

    async fn list_sources(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
    ) -> Result<Vec<SourceRow>, AppError> {
        self.repo
            .list_sources(auth, notebook_id)
            .await
            .map_err(map_pg_error)
    }

    async fn list_documents(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
        document_id: Option<Uuid>,
    ) -> Result<Vec<Document>, AppError> {
        self.repo
            .list_documents(auth, notebook_id, document_id)
            .await
            .map_err(map_pg_error)
    }

    async fn create_document(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        filename: &str,
        file_size: u64,
        mime_type: &str,
    ) -> Result<Document, AppError> {
        self.repo
            .create_document(auth, notebook_id, filename, file_size, mime_type)
            .await
            .map_err(map_pg_error)
    }

    async fn get_document_task_seed(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
    ) -> Result<Option<DocumentTaskSeed>, AppError> {
        self.repo
            .get_document_task_seed(auth, document_id)
            .await
            .map_err(map_pg_error)
    }

    async fn set_document_status(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
        status: DocumentStatus,
    ) -> Result<bool, AppError> {
        self.repo
            .set_document_status(auth, document_id, status)
            .await
            .map_err(map_pg_error)
    }

    async fn set_document_upload_invalid(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
        detail: &str,
    ) -> Result<DocumentUploadMutationOutcome, AppError> {
        self.repo
            .set_document_upload_invalid(auth, document_id, detail)
            .await
            .map_err(map_pg_error)
    }

    async fn queue_validated_document_upload(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
        size_bytes: u64,
        sha256_hex: Option<&str>,
        task: &IngestionTask,
    ) -> Result<DocumentUploadQueueOutcome, AppError> {
        self.repo
            .queue_validated_document_upload(auth, document_id, size_bytes, sha256_hex, task)
            .await
            .map_err(map_pg_error)
    }

    async fn update_document(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
        filename: Option<&str>,
        notebook_id: Option<Uuid>,
        status: Option<DocumentStatus>,
    ) -> Result<bool, AppError> {
        self.repo
            .update_document(auth, document_id, filename, notebook_id, status)
            .await
            .map_err(map_pg_error)
    }

    async fn delete_document(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
    ) -> Result<DocumentDeletionOutcome, AppError> {
        self.repo
            .delete_document(auth, document_id)
            .await
            .map_err(map_pg_error)
    }

    async fn get_document_content(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
    ) -> Result<Option<DocumentContentResponse>, AppError> {
        self.repo
            .get_document_content(auth, document_id)
            .await
            .map_err(map_pg_error)
    }

    async fn get_parsed_preview(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<ParsedPreviewResponse, AppError> {
        let cursor_offset = cursor
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        self.repo
            .get_parsed_preview(auth, document_id, cursor_offset, limit)
            .await
            .map_err(map_pg_error)?
            .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))
    }

    async fn enqueue_ingestion_task(&self, task: &IngestionTask) -> Result<bool, AppError> {
        self.repo
            .enqueue_ingestion_task(task)
            .await
            .map_err(map_pg_error)
    }

    async fn append_audit_record(&self, record: &AuditRecord) -> Result<(), AppError> {
        self.repo
            .append_audit_record(record)
            .await
            .map_err(map_pg_error)
    }
}
