use std::collections::BTreeMap;
use std::sync::Arc;

use app_core::{DocumentScopeValidator, MemoryState, ObjectStorePort, StorageContext};
use app_documents::DocumentContext;
use async_trait::async_trait;
use avrag_auth::{ActorId, AuthContext, OrgId, SubjectKind};
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

fn memory_storage(org_id: &str, user_id: &str) -> (StorageContext, String, String) {
    let storage = StorageContext::new(
        None,
        false,
        None,
        None,
        None,
        None,
        None,
        Arc::new(RwLock::new(MemoryState::default())),
        Arc::new(RwLock::new(BTreeMap::new())),
        10 * 1024 * 1024,
        true,
        Arc::new(TestObjectStore),
        "http://localhost".to_string(),
        "/tmp/avrag-documents-scope-test".to_string(),
        3600,
        3600,
    );
    (storage, org_id.to_string(), user_id.to_string())
}

#[tokio::test]
async fn validate_document_scope_rejects_foreign_notebook_in_memory() {
    let (storage, org_id, user_id) = memory_storage(
        "00000000-0000-0000-0000-000000000001",
        "00000000-0000-0000-0000-000000000002",
    );
    let auth = AuthContext::new(
        OrgId::from(Uuid::parse_str(&org_id).unwrap()),
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
                    org_id: org_id.clone(),
                    notebook_id: notebook_a.clone(),
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
