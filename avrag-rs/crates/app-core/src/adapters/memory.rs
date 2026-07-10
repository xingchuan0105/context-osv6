use crate::ports::workspaces::workspace_store::WorkspaceStore;
use crate::{
    BillingQuotaPort, DocumentStorePort, MemoryState, current_owner_user_id, current_user_id,
    domain_rows::{
        DocumentDeletionOutcome, DocumentScopeState, DocumentTaskSeed,
        DocumentUploadMutationOutcome, DocumentUploadQueueOutcome,
    },
};
use async_trait::async_trait;
use contracts::auth_runtime::AuthContext;
use common::{
    AppError, CreateWorkspaceRequest, Document, DocumentContentResponse, ParsedPreviewResponse,
    SourceRow, default_owner_user_id, default_user_id, new_id, now_rfc3339,
};
use contracts::documents::DocumentStatus;
use contracts::workspaces::Workspace;
use ingestion_types::{AuditRecord, IngestionTask};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Default, Clone)]
pub struct MemoryWorkspaceStore;

#[async_trait]
impl WorkspaceStore for MemoryWorkspaceStore {
    async fn list_workspaces(&self) -> Result<Vec<Workspace>, AppError> {
        Ok(Vec::new())
    }

    async fn create_workspace(&self, req: CreateWorkspaceRequest) -> Result<Workspace, AppError> {
        let now = now_rfc3339();
        Ok(Workspace {
            id: new_id(),
            owner_user_id: default_owner_user_id(),
            owner_id: default_user_id(),
            name: req.name.clone(),
            title: req.name,
            description: req.description,
            created_at: now.clone(),
            updated_at: now,
            document_count: 0,
            status_summary: std::collections::HashMap::new(),
            shared: false,
        })
    }
}

#[derive(Clone)]
pub struct MemoryDocumentStore {
    state: Arc<RwLock<MemoryState>>,
}

impl MemoryDocumentStore {
    pub fn new(state: Arc<RwLock<MemoryState>>) -> Self {
        Self { state }
    }
}

fn org_matches(auth: &AuthContext, candidate: &str) -> bool {
    candidate == current_owner_user_id(auth)
}

fn is_deleting_or_deleted(status: &DocumentStatus) -> bool {
    matches!(status, DocumentStatus::Deleting | DocumentStatus::Deleted)
}

fn upload_status_mutable(status: &DocumentStatus) -> bool {
    matches!(
        status,
        DocumentStatus::Pending | DocumentStatus::UploadInvalid
    )
}

#[async_trait]
impl DocumentStorePort for MemoryDocumentStore {
    async fn list_workspaces(&self, auth: &AuthContext) -> Result<Vec<Workspace>, AppError> {
        let state = self.state.read().await;
        Ok(state
            .workspaces
            .values()
            .filter(|n| org_matches(auth, &n.owner_user_id))
            .cloned()
            .collect())
    }

    async fn get_workspace(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<Option<Workspace>, AppError> {
        let state = self.state.read().await;
        Ok(state
            .workspaces
            .get(&workspace_id.to_string())
            .filter(|n| org_matches(auth, &n.owner_user_id))
            .cloned())
    }

    async fn create_workspace(
        &self,
        auth: &AuthContext,
        name: &str,
        description: &str,
    ) -> Result<Workspace, AppError> {
        let now = now_rfc3339();
        let notebook = Workspace {
            id: new_id(),
            owner_user_id: current_owner_user_id(auth),
            owner_id: current_user_id(auth),
            name: name.to_string(),
            title: name.to_string(),
            description: description.to_string(),
            document_count: 0,
            status_summary: std::collections::HashMap::new(),
            shared: false,
            created_at: now.clone(),
            updated_at: now,
        };
        let mut state = self.state.write().await;
        state
            .workspaces
            .insert(notebook.id.clone(), notebook.clone());
        Ok(notebook)
    }

    async fn update_workspace(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        name: Option<&str>,
        description: Option<&str>,
    ) -> Result<Option<Workspace>, AppError> {
        let mut state = self.state.write().await;
        let key = workspace_id.to_string();
        let notebook = state.workspaces.get_mut(&key);
        let Some(notebook) = notebook else {
            return Ok(None);
        };
        if !org_matches(auth, &notebook.owner_user_id) {
            return Ok(None);
        }
        if let Some(name) = name {
            notebook.name = name.to_string();
            notebook.title = name.to_string();
        }
        if let Some(description) = description {
            notebook.description = description.to_string();
        }
        notebook.updated_at = now_rfc3339();
        Ok(Some(notebook.clone()))
    }

    async fn delete_workspace(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<bool, AppError> {
        let key = workspace_id.to_string();
        let mut state = self.state.write().await;
        let can_delete = state
            .workspaces
            .get(&key)
            .map(|n| org_matches(auth, &n.owner_user_id))
            .unwrap_or(false);
        if !can_delete {
            return Ok(false);
        }
        state.workspaces.remove(&key);
        state
            .documents
            .retain(|_, stored| stored.document.workspace_id != key);
        let removed_sessions: Vec<String> = state
            .sessions
            .iter()
            .filter_map(|(id, session)| (session.workspace_id == key).then_some(id.clone()))
            .collect();
        for session_id in &removed_sessions {
            state.sessions.remove(session_id);
            state.messages.remove(session_id);
        }
        Ok(true)
    }

    async fn get_document_scope_states(
        &self,
        auth: &AuthContext,
        document_ids: &[Uuid],
    ) -> Result<Vec<DocumentScopeState>, AppError> {
        let state = self.state.read().await;
        let mut result = Vec::new();
        for id in document_ids {
            if let Some(stored) = state.documents.get(&id.to_string())
                && org_matches(auth, &stored.document.owner_user_id)
            {
                result.push(DocumentScopeState {
                    document_id: *id,
                    status: stored.document.status.clone(),
                });
            }
        }
        Ok(result)
    }

    async fn list_sources(
        &self,
        _auth: &AuthContext,
        _workspace_id: Option<Uuid>,
    ) -> Result<Vec<SourceRow>, AppError> {
        Ok(Vec::new())
    }

    async fn list_documents(
        &self,
        auth: &AuthContext,
        workspace_id: Option<Uuid>,
        document_id: Option<Uuid>,
    ) -> Result<Vec<Document>, AppError> {
        let state = self.state.read().await;
        let notebook_filter = workspace_id.map(|id| id.to_string());
        let document_filter = document_id.map(|id| id.to_string());
        Ok(state
            .documents
            .values()
            .filter(|stored| org_matches(auth, &stored.document.owner_user_id))
            .filter(|stored| {
                notebook_filter
                    .as_ref()
                    .map(|id| stored.document.workspace_id == *id)
                    .unwrap_or(true)
            })
            .filter(|stored| {
                document_filter
                    .as_ref()
                    .map(|id| stored.document.id == *id)
                    .unwrap_or(true)
            })
            .filter(|stored| !is_deleting_or_deleted(&stored.document.status))
            .map(|stored| stored.document.clone())
            .collect())
    }

    async fn create_document(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        filename: &str,
        file_size: u64,
        mime_type: &str,
    ) -> Result<Document, AppError> {
        let now = now_rfc3339();
        let document = Document {
            id: new_id(),
            owner_user_id: current_owner_user_id(auth),
            workspace_id: workspace_id.to_string(),
            owner_id: current_user_id(auth),
            file_name: filename.to_string(),
            mime_type: mime_type.to_string(),
            file_size,
            status: DocumentStatus::Pending,
            chunk_count: 0,
            created_at: now.clone(),
            updated_at: now,
        };
        let stored = crate::StoredDocument {
            document: document.clone(),
            content: String::new(),
            summary: None,
            parsed_items: Vec::new(),
        };
        let mut state = self.state.write().await;
        state.documents.insert(document.id.clone(), stored);
        Ok(document)
    }

    async fn get_document_task_seed(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
    ) -> Result<Option<DocumentTaskSeed>, AppError> {
        let state = self.state.read().await;
        let Some(stored) = state.documents.get(&document_id.to_string()) else {
            return Ok(None);
        };
        if !org_matches(auth, &stored.document.owner_user_id) {
            return Ok(None);
        }
        let doc = &stored.document;
        Ok(Some(DocumentTaskSeed {
            document_id: doc.id.clone(),
            owner_user_id: doc.owner_user_id.clone(),
            workspace_id: doc.workspace_id.clone(),
            filename: doc.file_name.clone(),
            mime_type: doc.mime_type.clone(),
            file_size: doc.file_size,
            object_path: format!("{}/{}/{}", doc.owner_user_id, doc.workspace_id, doc.id),
            status: doc.status.clone(),
        }))
    }

    async fn set_document_status(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
        status: DocumentStatus,
    ) -> Result<bool, AppError> {
        let mut state = self.state.write().await;
        let Some(stored) = state.documents.get_mut(&document_id.to_string()) else {
            return Ok(false);
        };
        if !org_matches(auth, &stored.document.owner_user_id) {
            return Ok(false);
        }
        if is_deleting_or_deleted(&stored.document.status) {
            return Ok(false);
        }
        stored.document.status = status;
        stored.document.updated_at = now_rfc3339();
        Ok(true)
    }

    async fn set_document_upload_invalid(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
        _detail: &str,
    ) -> Result<DocumentUploadMutationOutcome, AppError> {
        let mut state = self.state.write().await;
        let Some(stored) = state.documents.get_mut(&document_id.to_string()) else {
            return Ok(DocumentUploadMutationOutcome::NotFound);
        };
        if !org_matches(auth, &stored.document.owner_user_id) {
            return Ok(DocumentUploadMutationOutcome::NotFound);
        }
        if !upload_status_mutable(&stored.document.status) {
            return Ok(DocumentUploadMutationOutcome::StatusConflict(
                stored.document.status.clone(),
            ));
        }
        stored.document.status = DocumentStatus::UploadInvalid;
        stored.document.updated_at = now_rfc3339();
        Ok(DocumentUploadMutationOutcome::Updated)
    }

    async fn queue_validated_document_upload(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
        size_bytes: u64,
        _sha256_hex: Option<&str>,
        _task: &IngestionTask,
    ) -> Result<DocumentUploadQueueOutcome, AppError> {
        let mut state = self.state.write().await;
        let Some(stored) = state.documents.get_mut(&document_id.to_string()) else {
            return Ok(DocumentUploadQueueOutcome::NotFound);
        };
        if !org_matches(auth, &stored.document.owner_user_id) {
            return Ok(DocumentUploadQueueOutcome::NotFound);
        }
        if !upload_status_mutable(&stored.document.status) {
            return Ok(DocumentUploadQueueOutcome::StatusConflict(
                stored.document.status.clone(),
            ));
        }
        stored.document.status = DocumentStatus::Queued;
        stored.document.file_size = size_bytes;
        stored.document.updated_at = now_rfc3339();
        Ok(DocumentUploadQueueOutcome::Queued { task_inserted: false })
    }

    async fn update_document(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
        filename: Option<&str>,
        workspace_id: Option<Uuid>,
        status: Option<DocumentStatus>,
    ) -> Result<bool, AppError> {
        let mut state = self.state.write().await;
        let Some(stored) = state.documents.get_mut(&document_id.to_string()) else {
            return Ok(false);
        };
        if !org_matches(auth, &stored.document.owner_user_id) {
            return Ok(false);
        }
        if is_deleting_or_deleted(&stored.document.status) {
            return Ok(false);
        }
        if let Some(filename) = filename {
            stored.document.file_name = filename.to_string();
        }
        if let Some(workspace_id) = workspace_id {
            stored.document.workspace_id = workspace_id.to_string();
        }
        if let Some(status) = status {
            stored.document.status = status;
        }
        stored.document.updated_at = now_rfc3339();
        Ok(true)
    }

    async fn delete_document(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
    ) -> Result<DocumentDeletionOutcome, AppError> {
        let mut state = self.state.write().await;
        let key = document_id.to_string();
        let Some(stored) = state.documents.get_mut(&key) else {
            return Ok(DocumentDeletionOutcome::NotFound);
        };
        if !org_matches(auth, &stored.document.owner_user_id) {
            return Ok(DocumentDeletionOutcome::NotFound);
        }
        match stored.document.status {
            DocumentStatus::Deleted => return Ok(DocumentDeletionOutcome::AlreadyDeleted),
            DocumentStatus::Deleting => {
                return Ok(DocumentDeletionOutcome::AlreadyDeleting {
                    task_inserted: false,
                })
            }
            _ => {}
        }
        stored.document.status = DocumentStatus::Deleting;
        stored.document.updated_at = now_rfc3339();
        Ok(DocumentDeletionOutcome::Queued { task_inserted: false })
    }

    async fn get_document_content(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
    ) -> Result<Option<DocumentContentResponse>, AppError> {
        let state = self.state.read().await;
        let Some(stored) = state.documents.get(&document_id.to_string()) else {
            return Ok(None);
        };
        if !org_matches(auth, &stored.document.owner_user_id) {
            return Ok(None);
        }
        if is_deleting_or_deleted(&stored.document.status) {
            return Ok(None);
        }
        Ok(Some(DocumentContentResponse {
            content: stored.content.clone(),
            summary: stored.summary.clone(),
        }))
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
        let state = self.state.read().await;
        let Some(stored) = state.documents.get(&document_id.to_string()) else {
            return Err(AppError::not_found(
                "document_not_found",
                "document not found",
            ));
        };
        if !org_matches(auth, &stored.document.owner_user_id) {
            return Err(AppError::not_found(
                "document_not_found",
                "document not found",
            ));
        }
        if is_deleting_or_deleted(&stored.document.status) {
            return Err(AppError::not_found(
                "document_not_found",
                "document not found",
            ));
        }
        let items = stored
            .parsed_items
            .iter()
            .skip(cursor_offset)
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        let next_cursor = cursor_offset + items.len();
        Ok(ParsedPreviewResponse {
            items,
            has_more: next_cursor < stored.parsed_items.len(),
            next_cursor,
            summary: stored.summary.clone(),
        })
    }

    async fn enqueue_ingestion_task(&self, _task: &IngestionTask) -> Result<bool, AppError> {
        Ok(false)
    }

    async fn append_audit_record(&self, _record: &AuditRecord) -> Result<(), AppError> {
        Ok(())
    }
}

#[derive(Default, Clone)]
pub struct MemoryBillingQuotaPort;

#[async_trait]
impl BillingQuotaPort for MemoryBillingQuotaPort {
    async fn ensure_storage_bytes_quota(
        &self,
        _auth: &AuthContext,
        _bytes: i64,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn notebook_exists(
        &self,
        _auth: &AuthContext,
        _workspace_id: Uuid,
    ) -> Result<bool, AppError> {
        Ok(true)
    }
}
