use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use app_core::{AnalyticsServiceCtx, DocumentStorePort, MemoryState, ObjectStorePort, StorageContext};
use app_documents::DocumentContext;
use async_trait::async_trait;
use avrag_auth::{ActorId, AuthContext, OrgId, SubjectKind};
use common::{AppError, CreateNotebookRequest, Document, SourceRow, now_rfc3339};
use contracts::documents::{DocumentStatus};
use contracts::notebooks::{Notebook};
use tokio::sync::RwLock;
use uuid::Uuid;

struct TestObjectStore;

#[async_trait]
impl ObjectStorePort for TestObjectStore {
    async fn put(&self, _path: &str, _bytes: &[u8]) -> Result<(), AppError> {
        Ok(())
    }

    async fn put_stream(
        &self,
        _path: &str,
        _stream: app_core::ObjectStoreUploadStream,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn get(&self, _path: &str) -> Result<Vec<u8>, AppError> {
        Ok(Vec::new())
    }

    async fn head(
        &self,
        _path: &str,
    ) -> Result<app_core::ObjectStoreMetadata, app_core::ObjectStoreHeadError> {
        Err(app_core::ObjectStoreHeadError::NotFound {
            path: String::new(),
        })
    }

    async fn presigned_get_url(&self, _path: &str, _ttl_secs: u64) -> Result<String, AppError> {
        Ok(String::new())
    }
}

#[test]
fn document_modules_do_not_call_storage_pg_escape_hatch() {
    let forbidden = concat!("storage.", "pg(");
    let sources = [
        include_str!("../src/documents.rs"),
        include_str!("../src/document_context.rs"),
        include_str!("../src/url_imports.rs"),
        include_str!("../src/ingest.rs"),
        include_str!("../src/notebooks.rs"),
    ];
    for source in sources {
        assert!(
            !source.contains(forbidden),
            "app-documents must use typed storage ports, not the pg escape hatch"
        );
    }
}

#[derive(Clone, Default)]
struct RecordingDocumentStore {
    list_notebooks_calls: Arc<std::sync::atomic::AtomicUsize>,
}

#[async_trait]
impl DocumentStorePort for RecordingDocumentStore {
    async fn list_notebooks(&self, _auth: &AuthContext) -> Result<Vec<Notebook>, AppError> {
        self.list_notebooks_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(vec![Notebook {
            id: Uuid::new_v4().to_string(),
            org_id: "org-test".to_string(),
            owner_id: "user-test".to_string(),
            name: "Port Notebook".to_string(),
            title: "Port Notebook".to_string(),
            description: String::new(),
            document_count: 0,
            status_summary: HashMap::new(),
            shared: false,
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
        }])
    }

    async fn get_notebook(
        &self,
        _auth: &AuthContext,
        _notebook_id: Uuid,
    ) -> Result<Option<Notebook>, AppError> {
        Ok(None)
    }

    async fn create_notebook(
        &self,
        _auth: &AuthContext,
        _name: &str,
        _description: &str,
    ) -> Result<Notebook, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn update_notebook(
        &self,
        _auth: &AuthContext,
        _notebook_id: Uuid,
        _name: Option<&str>,
        _description: Option<&str>,
    ) -> Result<Option<Notebook>, AppError> {
        Ok(None)
    }

    async fn delete_notebook(
        &self,
        _auth: &AuthContext,
        _notebook_id: Uuid,
    ) -> Result<bool, AppError> {
        Ok(false)
    }

    async fn get_document_scope_states(
        &self,
        _auth: &AuthContext,
        _document_ids: &[Uuid],
    ) -> Result<Vec<app_core::DocumentScopeState>, AppError> {
        Ok(Vec::new())
    }

    async fn list_sources(
        &self,
        _auth: &AuthContext,
        _notebook_id: Option<Uuid>,
    ) -> Result<Vec<SourceRow>, AppError> {
        Ok(Vec::new())
    }

    async fn list_documents(
        &self,
        _auth: &AuthContext,
        _notebook_id: Option<Uuid>,
        _document_id: Option<Uuid>,
    ) -> Result<Vec<Document>, AppError> {
        Ok(Vec::new())
    }

    async fn create_document(
        &self,
        _auth: &AuthContext,
        _notebook_id: Uuid,
        _filename: &str,
        _file_size: u64,
        _mime_type: &str,
    ) -> Result<Document, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn get_document_task_seed(
        &self,
        _auth: &AuthContext,
        _document_id: Uuid,
    ) -> Result<Option<app_core::DocumentTaskSeed>, AppError> {
        Ok(None)
    }

    async fn set_document_status(
        &self,
        _auth: &AuthContext,
        _document_id: Uuid,
        _status: DocumentStatus,
    ) -> Result<bool, AppError> {
        Ok(false)
    }

    async fn set_document_upload_invalid(
        &self,
        _auth: &AuthContext,
        _document_id: Uuid,
        _detail: &str,
    ) -> Result<app_core::DocumentUploadMutationOutcome, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn queue_validated_document_upload(
        &self,
        _auth: &AuthContext,
        _document_id: Uuid,
        _size_bytes: u64,
        _sha256_hex: Option<&str>,
        _task: &ingestion_types::IngestionTask,
    ) -> Result<app_core::DocumentUploadQueueOutcome, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn update_document(
        &self,
        _auth: &AuthContext,
        _document_id: Uuid,
        _filename: Option<&str>,
        _notebook_id: Option<Uuid>,
        _status: Option<DocumentStatus>,
    ) -> Result<bool, AppError> {
        Ok(false)
    }

    async fn delete_document(
        &self,
        _auth: &AuthContext,
        _document_id: Uuid,
    ) -> Result<app_core::DocumentDeletionOutcome, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn get_document_content(
        &self,
        _auth: &AuthContext,
        _document_id: Uuid,
    ) -> Result<Option<common::DocumentContentResponse>, AppError> {
        Ok(None)
    }

    async fn get_parsed_preview(
        &self,
        _auth: &AuthContext,
        _document_id: Uuid,
        _cursor: Option<&str>,
        _limit: usize,
    ) -> Result<common::ParsedPreviewResponse, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn enqueue_ingestion_task(
        &self,
        _task: &ingestion_types::IngestionTask,
    ) -> Result<bool, AppError> {
        Ok(false)
    }

    async fn append_audit_record(
        &self,
        _record: &ingestion_types::AuditRecord,
    ) -> Result<(), AppError> {
        Ok(())
    }
}

fn test_auth() -> AuthContext {
    AuthContext::new(OrgId::from(Uuid::nil()), SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::nil()))
        .with_request_id("documents-port-contract")
}

fn memory_storage() -> StorageContext {
    StorageContext::new(
        None,
        false,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Arc::new(RwLock::new(MemoryState::default())),
        Arc::new(RwLock::new(BTreeMap::new())),
        Arc::new(RwLock::new(BTreeMap::new())),
        10 * 1024 * 1024,
        true,
        Arc::new(TestObjectStore),
        "http://localhost".to_string(),
        "/tmp/avrag-documents-test".to_string(),
        3600,
        3600,
    )
}

fn storage_with_document_store(store: Arc<dyn DocumentStorePort>) -> StorageContext {
    StorageContext::new(
        None,
        false,
        Some(store),
        None,
        None,
        None,
        None,
        None,
        None,
        Arc::new(RwLock::new(MemoryState::default())),
        Arc::new(RwLock::new(BTreeMap::new())),
        Arc::new(RwLock::new(BTreeMap::new())),
        10 * 1024 * 1024,
        false,
        Arc::new(TestObjectStore),
        "http://localhost".to_string(),
        "/tmp/avrag-documents-test".to_string(),
        3600,
        3600,
    )
}

#[tokio::test]
async fn memory_mode_create_notebook_round_trips_without_ports() {
    let ctx = DocumentContext::new();
    let storage = memory_storage();
    let auth = test_auth();
    let analytics = AnalyticsServiceCtx::new(None);

    let notebook = ctx
        .create_notebook(
            &auth,
            &storage,
            &analytics,
            CreateNotebookRequest {
                name: "Memory Notebook".to_string(),
                description: String::new(),
            },
        )
        .await
        .unwrap();

    let listed = ctx.list_notebooks(&auth, &storage).await;
    assert!(listed.iter().any(|item| item.id == notebook.id));
}

#[tokio::test]
async fn document_store_port_is_used_when_wired() {
    let recorder = Arc::new(RecordingDocumentStore::default());
    let calls = recorder.list_notebooks_calls.clone();
    let storage = storage_with_document_store(recorder);
    let ctx = DocumentContext::new();
    let auth = test_auth();

    let notebooks = ctx.list_notebooks(&auth, &storage).await;
    assert_eq!(notebooks.len(), 1);
    assert_eq!(notebooks[0].name, "Port Notebook");
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "list_notebooks should delegate to DocumentStorePort"
    );
}
#[test]
fn billing_quota_port_is_required_for_url_import_pg_path() {
    let source = include_str!("../src/url_imports.rs");
    assert!(source.contains("storage.billing_quota()"));
    assert!(source.contains("ensure_storage_bytes_quota"));
    assert!(!source.contains("billing.ensure_metric_quota"));
}

#[test]
fn billing_quota_port_is_required_for_create_document_pg_path() {
    let source = include_str!("../src/documents.rs");
    assert!(source.contains("storage.billing_quota().ok_or_else"));
    assert!(source.contains("ensure_storage_bytes_quota"));
}
