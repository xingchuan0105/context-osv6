use std::sync::Arc;

use crate::domain_row_convert::{
    document_deletion_outcome, document_scope_state, document_task_seed,
    document_upload_mutation_outcome, document_upload_queue_outcome,
};
use crate::pg_error::map_pg_error;
use app_core::{
    DocumentStorePort, domain_rows::DocumentDeletionOutcome, domain_rows::DocumentScopeState,
    domain_rows::DocumentTaskSeed, domain_rows::DocumentUploadMutationOutcome,
    domain_rows::DocumentUploadQueueOutcome,
};
use async_trait::async_trait;
use contracts::auth_runtime::AuthContext;
use avrag_storage_pg::PgAppRepository;
use common::{AppError, Document, DocumentContentResponse, ParsedPreviewResponse, SourceRow};
use contracts::documents::DocumentStatus;
use contracts::notebooks::Notebook;
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
        workspace_id: Uuid,
    ) -> Result<Option<Notebook>, AppError> {
        self.repo
            .bootstrap()
            .get_notebook(auth, workspace_id)
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
            .bootstrap()
            .create_notebook(auth, name, description)
            .await
            .map_err(map_pg_error)
    }

    async fn update_notebook(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        name: Option<&str>,
        description: Option<&str>,
    ) -> Result<Option<Notebook>, AppError> {
        let current = self
            .repo
            .bootstrap()
            .get_notebook(auth, workspace_id)
            .await
            .map_err(map_pg_error)?;
        let Some(notebook) = current else {
            return Ok(None);
        };
        let name = name.unwrap_or(notebook.name.as_str());
        let description = description.unwrap_or(notebook.description.as_str());
        self.repo
            .bootstrap()
            .update_notebook(auth, workspace_id, name, description)
            .await
            .map_err(map_pg_error)
    }

    async fn delete_notebook(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<bool, AppError> {
        self.repo
            .bootstrap()
            .delete_notebook(auth, workspace_id)
            .await
            .map_err(map_pg_error)
    }

    async fn get_document_scope_states(
        &self,
        auth: &AuthContext,
        document_ids: &[Uuid],
    ) -> Result<Vec<DocumentScopeState>, AppError> {
        self.repo
            .chunks()
            .get_document_scope_states(auth, document_ids)
            .await
            .map_err(map_pg_error)
            .map(|rows| rows.into_iter().map(document_scope_state).collect())
    }

    async fn list_sources(
        &self,
        auth: &AuthContext,
        workspace_id: Option<Uuid>,
    ) -> Result<Vec<SourceRow>, AppError> {
        self.repo
            .chunks()
            .list_sources(auth, workspace_id)
            .await
            .map_err(map_pg_error)
    }

    async fn list_documents(
        &self,
        auth: &AuthContext,
        workspace_id: Option<Uuid>,
        document_id: Option<Uuid>,
    ) -> Result<Vec<Document>, AppError> {
        self.repo
            .list_documents(auth, workspace_id, document_id)
            .await
            .map_err(map_pg_error)
    }

    async fn create_document(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        filename: &str,
        file_size: u64,
        mime_type: &str,
    ) -> Result<Document, AppError> {
        self.repo
            .bootstrap()
            .create_document(auth, workspace_id, filename, file_size, mime_type)
            .await
            .map_err(map_pg_error)
    }

    async fn get_document_task_seed(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
    ) -> Result<Option<DocumentTaskSeed>, AppError> {
        self.repo
            .bootstrap()
            .get_document_task_seed(auth, document_id)
            .await
            .map_err(map_pg_error)
            .map(|seed| seed.map(document_task_seed))
    }

    async fn set_document_status(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
        status: DocumentStatus,
    ) -> Result<bool, AppError> {
        self.repo
            .documents()
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
            .documents()
            .set_document_upload_invalid(auth, document_id, detail)
            .await
            .map_err(map_pg_error)
            .map(document_upload_mutation_outcome)
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
            .ingestion_queue()
            .queue_validated_document_upload(auth, document_id, size_bytes, sha256_hex, task)
            .await
            .map_err(map_pg_error)
            .map(document_upload_queue_outcome)
    }

    async fn update_document(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
        filename: Option<&str>,
        workspace_id: Option<Uuid>,
        status: Option<DocumentStatus>,
    ) -> Result<bool, AppError> {
        self.repo
            .documents()
            .update_document(auth, document_id, filename, workspace_id, status)
            .await
            .map_err(map_pg_error)
    }

    async fn delete_document(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
    ) -> Result<DocumentDeletionOutcome, AppError> {
        self.repo
            .documents()
            .delete_document(auth, document_id)
            .await
            .map_err(map_pg_error)
            .map(document_deletion_outcome)
    }

    async fn get_document_content(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
    ) -> Result<Option<DocumentContentResponse>, AppError> {
        self.repo
            .chunks()
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
            .chunks()
            .get_parsed_preview(auth, document_id, cursor_offset, limit)
            .await
            .map_err(map_pg_error)?
            .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))
    }

    async fn enqueue_ingestion_task(&self, task: &IngestionTask) -> Result<bool, AppError> {
        self.repo
            .ingestion_queue()
            .enqueue_ingestion_task(task)
            .await
            .map_err(map_pg_error)
    }

    async fn append_audit_record(&self, record: &AuditRecord) -> Result<(), AppError> {
        self.repo
            .audit()
            .append_audit_record(record)
            .await
            .map_err(map_pg_error)
    }
}
