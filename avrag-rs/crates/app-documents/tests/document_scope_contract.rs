use std::collections::BTreeMap;
use std::sync::Arc;

use app_core::{
    DocumentScopeValidator, MemoryDocumentStore, MemoryState, MemoryStateHandles, ObjectStoreConfig,
    ObjectStorePort, StorageContext, StorageContextParts, StorageInfra, StorageStores,
};
use app_documents::DocumentContext;
use async_trait::async_trait;
use contracts::auth_runtime::{ActorId, AuthContext, UserId, SubjectKind};
use common::AppError;
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

fn memory_storage(owner_user_id: &str, user_id: &str) -> (StorageContext, String, String) {
    let memory_state = Arc::new(RwLock::new(MemoryState::default()));
    let storage = StorageContext::from_parts(StorageContextParts {
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
            object_root: "/tmp/avrag-documents-scope-test".to_string(),
            upload_expire_sec: 3600,
            download_expire_sec: 3600,
        },
    });
    (storage, owner_user_id.to_string(), user_id.to_string())
}

#[tokio::test]
async fn validate_document_scope_rejects_foreign_workspace_in_memory() {
    let (storage, owner_user_id, user_id) = memory_storage(
        "00000000-0000-0000-0000-000000000001",
        "00000000-0000-0000-0000-000000000002",
    );
    let auth = AuthContext::new(
        UserId::from(Uuid::parse_str(&owner_user_id).unwrap()),
        SubjectKind::User,
    )
    .with_actor_id(ActorId::new(Uuid::parse_str(&user_id).unwrap()));

    let notebook_a = Uuid::new_v4().to_string();
    let notebook_b = Uuid::new_v4().to_string();
    let document_id = Uuid::new_v4().to_string();

    {
        let mut state = storage.inner().write().await;
        state.documents.insert(
            document_id.clone(),
            app_core::StoredDocument {
                document: common::Document {
                    id: document_id.clone(),
                    owner_user_id: owner_user_id.clone(),
                    workspace_id: notebook_a.clone(),
                    owner_id: user_id.clone(),
                    file_name: "scope.txt".to_string(),
                    mime_type: "text/plain".to_string(),
                    file_size: 4,
                    status: contracts::documents::DocumentStatus::Completed,
                    chunk_count: 0,
                    created_at: String::new(),
                    updated_at: String::new(),
                },
                content: String::new(),
                summary: None,
                parsed_items: Vec::new(),
            },
        );
    }

    let service = DocumentContext::new();
    let error = service
        .validate_document_scope(&auth, &storage, &notebook_b, &[document_id])
        .await
        .expect_err("foreign notebook should be rejected");

    assert!(matches!(error, AppError::Validation { .. }));
}
