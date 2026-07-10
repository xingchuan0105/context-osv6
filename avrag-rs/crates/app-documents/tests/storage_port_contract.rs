use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use app_core::{
    AnalyticsServiceCtx, DocumentStorePort, MemoryDocumentStore, MemoryState, MemoryStateHandles,
    ObjectStoreConfig, ObjectStorePort, StorageContext, StorageContextParts, StorageInfra,
    StorageStores,
};
use app_documents::DocumentContext;
use async_trait::async_trait;
use contracts::auth_runtime::{ActorId, AuthContext, UserId, SubjectKind};
use common::{AppError, CreateWorkspaceRequest, Document, SourceRow, now_rfc3339};
use contracts::documents::DocumentStatus;
use contracts::workspaces::Workspace;
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
    list_workspaces_calls: Arc<std::sync::atomic::AtomicUsize>,
}

#[async_trait]
impl DocumentStorePort for RecordingDocumentStore {
    async fn list_workspaces(&self, _auth: &AuthContext) -> Result<Vec<Workspace>, AppError> {
        self.list_workspaces_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(vec![Workspace {
            id: Uuid::new_v4().to_string(),
            owner_user_id: "org-test".to_string(),
            owner_id: "user-test".to_string(),
            name: "Port Workspace".to_string(),
            title: "Port Workspace".to_string(),
            description: String::new(),
            document_count: 0,
            status_summary: HashMap::new(),
            shared: false,
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
        }])
    }

    async fn get_workspace(
        &self,
        _auth: &AuthContext,
        _workspace_id: Uuid,
    ) -> Result<Option<Workspace>, AppError> {
        Ok(None)
    }

    async fn create_workspace(
        &self,
        _auth: &AuthContext,
        _name: &str,
        _description: &str,
    ) -> Result<Workspace, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn update_workspace(
        &self,
        _auth: &AuthContext,
        _workspace_id: Uuid,
        _name: Option<&str>,
        _description: Option<&str>,
    ) -> Result<Option<Workspace>, AppError> {
        Ok(None)
    }

    async fn delete_workspace(
        &self,
        _auth: &AuthContext,
        _workspace_id: Uuid,
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
        _workspace_id: Option<Uuid>,
    ) -> Result<Vec<SourceRow>, AppError> {
        Ok(Vec::new())
    }

    async fn list_documents(
        &self,
        _auth: &AuthContext,
        _workspace_id: Option<Uuid>,
        _document_id: Option<Uuid>,
    ) -> Result<Vec<Document>, AppError> {
        Ok(Vec::new())
    }

    async fn create_document(
        &self,
        _auth: &AuthContext,
        _workspace_id: Uuid,
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
        _workspace_id: Option<Uuid>,
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
    AuthContext::new(UserId::from(Uuid::nil()), SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::nil()))
        .with_request_id("documents-port-contract")
}

fn memory_storage() -> StorageContext {
    let memory_state = Arc::new(RwLock::new(MemoryState::default()));
    StorageContext::from_parts(StorageContextParts {
        infra: StorageInfra {
            postgres_health: None,
            postgres_configured: false,
            uses_memory_adapters: StorageInfra::memory_adapters_flag(true),
            max_upload_file_size_bytes: 10 * 1024 * 1024,
        },
        stores: StorageStores {
            document_store: Some(Arc::new(MemoryDocumentStore::new(memory_state.clone()))),
            auth_store: None,
            admin_store: None,
            billing_quota: None,
            billing_store: None,
            share_store: None,
            chat_persistence: None,
        },
        memory: MemoryStateHandles {
            inner: memory_state,
            api_keys: Arc::new(RwLock::new(BTreeMap::new())),
            api_key_hashes: Arc::new(RwLock::new(BTreeMap::new())),
        },
        objects: ObjectStoreConfig {
            object_store: Arc::new(TestObjectStore),
            public_base_url: "http://localhost".to_string(),
            object_root: "/tmp/avrag-documents-test".to_string(),
            upload_expire_sec: 3600,
            download_expire_sec: 3600,
        },
    })
}

fn storage_with_document_store(store: Arc<dyn DocumentStorePort>) -> StorageContext {
    StorageContext::from_parts(StorageContextParts {
        infra: StorageInfra {
            postgres_health: None,
            postgres_configured: false,
            uses_memory_adapters: StorageInfra::memory_adapters_flag(false),
            max_upload_file_size_bytes: 10 * 1024 * 1024,
        },
        stores: StorageStores {
            document_store: Some(store),
            auth_store: None,
            admin_store: None,
            billing_quota: None,
            billing_store: None,
            share_store: None,
            chat_persistence: None,
        },
        memory: MemoryStateHandles {
            inner: Arc::new(RwLock::new(MemoryState::default())),
            api_keys: Arc::new(RwLock::new(BTreeMap::new())),
            api_key_hashes: Arc::new(RwLock::new(BTreeMap::new())),
        },
        objects: ObjectStoreConfig {
            object_store: Arc::new(TestObjectStore),
            public_base_url: "http://localhost".to_string(),
            object_root: "/tmp/avrag-documents-test".to_string(),
            upload_expire_sec: 3600,
            download_expire_sec: 3600,
        },
    })
}

#[tokio::test]
async fn memory_mode_create_workspace_round_trips_via_memory_document_store() {
    let ctx = DocumentContext::new();
    let storage = memory_storage();
    let auth = test_auth();
    let analytics = AnalyticsServiceCtx::new(None);

    let notebook = ctx
        .create_workspace(
            &auth,
            &storage,
            &analytics,
            CreateWorkspaceRequest {
                name: "Memory Workspace".to_string(),
                description: String::new(),
            },
        )
        .await
        .unwrap();

    let listed = ctx.list_workspaces(&auth, &storage).await;
    assert!(listed.iter().any(|item| item.id == notebook.id));
}

#[tokio::test]
async fn document_store_port_is_used_when_wired() {
    let recorder = Arc::new(RecordingDocumentStore::default());
    let calls = recorder.list_workspaces_calls.clone();
    let storage = storage_with_document_store(recorder);
    let ctx = DocumentContext::new();
    let auth = test_auth();

    let workspaces = ctx.list_workspaces(&auth, &storage).await;
    assert_eq!(notebooks.len(), 1);
    assert_eq!(notebooks[0].name, "Port Workspace");
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "list_workspaces should delegate to DocumentStorePort"
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
