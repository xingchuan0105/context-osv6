use async_trait::async_trait;
use common::{AppError, Document, DocumentContentResponse, ParsedPreviewResponse, SourceRow};
use contracts::auth_runtime::AuthContext;
use contracts::documents::DocumentStatus;
use contracts::workspaces::Workspace;
use ingestion_types::{AuditRecord, IngestionTask};
use uuid::Uuid;

use crate::domain_rows::{
    DocumentDeletionOutcome, DocumentScopeState, DocumentTaskSeed, DocumentUploadMutationOutcome,
    DocumentUploadQueueOutcome, WorkspaceBindingVersion,
};

#[async_trait]
pub trait DocumentStorePort: Send + Sync {
    async fn list_workspaces(&self, auth: &AuthContext) -> Result<Vec<Workspace>, AppError>;

    async fn get_workspace(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<Option<Workspace>, AppError>;

    async fn create_workspace(
        &self,
        auth: &AuthContext,
        name: &str,
        description: &str,
    ) -> Result<Workspace, AppError>;

    async fn update_workspace(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        name: Option<&str>,
        description: Option<&str>,
    ) -> Result<Option<Workspace>, AppError>;

    async fn delete_workspace(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<bool, AppError>;

    async fn get_document_scope_states(
        &self,
        auth: &AuthContext,
        document_ids: &[Uuid],
    ) -> Result<Vec<DocumentScopeState>, AppError>;

    async fn list_sources(
        &self,
        auth: &AuthContext,
        workspace_id: Option<Uuid>,
    ) -> Result<Vec<SourceRow>, AppError>;

    async fn list_documents(
        &self,
        auth: &AuthContext,
        workspace_id: Option<Uuid>,
        document_id: Option<Uuid>,
    ) -> Result<Vec<Document>, AppError>;

    async fn create_document(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        filename: &str,
        file_size: u64,
        mime_type: &str,
    ) -> Result<Document, AppError>;

    /// Upsert a document row with a caller-supplied id (workspace publish import).
    async fn upsert_published_document(
        &self,
        auth: &AuthContext,
        input: crate::PublishedDocumentUpsert,
    ) -> Result<Document, AppError>;

    /// Create an artifact bound to a Conversation (chat-first W2b). No workspace
    /// binding is written; the conversation binding is the sole scope.
    async fn create_session_document(
        &self,
        auth: &AuthContext,
        conversation_id: Uuid,
        filename: &str,
        file_size: u64,
        mime_type: &str,
    ) -> Result<Document, AppError>;

    /// Files visible to a Conversation via its typed bindings (upload order).
    /// Workspace-bound completed artifacts with binding + parse version facts
    /// (chat-first W2d snapshot; scope truth is the binding tables).
    async fn completed_workspace_binding_versions(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<Vec<WorkspaceBindingVersion>, AppError>;

    async fn list_session_files(
        &self,
        auth: &AuthContext,
        conversation_id: Uuid,
    ) -> Result<Vec<contracts::documents::SessionFileRow>, AppError>;

    /// Remove one Conversation binding. Returns the binding's artifact id so the
    /// caller can run zero-binding GC (W2e); `None` when binding/conversation
    /// was not found for this owner.
    async fn delete_session_file_binding(
        &self,
        auth: &AuthContext,
        conversation_id: Uuid,
        binding_id: Uuid,
    ) -> Result<Option<String>, AppError>;

    async fn get_document_task_seed(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
    ) -> Result<Option<DocumentTaskSeed>, AppError>;

    async fn set_document_status(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
        status: DocumentStatus,
    ) -> Result<bool, AppError>;

    async fn set_document_upload_invalid(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
        detail: &str,
    ) -> Result<DocumentUploadMutationOutcome, AppError>;

    async fn queue_validated_document_upload(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
        size_bytes: u64,
        sha256_hex: Option<&str>,
        task: &IngestionTask,
    ) -> Result<DocumentUploadQueueOutcome, AppError>;

    async fn update_document(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
        filename: Option<&str>,
        status: Option<DocumentStatus>,
    ) -> Result<bool, AppError>;

    async fn delete_document(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
    ) -> Result<DocumentDeletionOutcome, AppError>;

    /// Full deletion for artifacts with zero bindings of either kind (chat-first
    /// W2e). Owner-scoped; no-op when any session/workspace binding remains.
    /// Returns `true` when the artifact entered the async cleanup flow.
    async fn delete_document_if_unbound(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
    ) -> Result<bool, AppError>;

    async fn get_document_content(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
    ) -> Result<Option<DocumentContentResponse>, AppError>;

    async fn get_parsed_preview(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<ParsedPreviewResponse, AppError>;

    async fn enqueue_ingestion_task(&self, task: &IngestionTask) -> Result<bool, AppError>;

    async fn append_audit_record(&self, record: &AuditRecord) -> Result<(), AppError>;
}
