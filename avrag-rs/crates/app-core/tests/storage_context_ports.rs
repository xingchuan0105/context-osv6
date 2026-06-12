use std::collections::BTreeMap;
use std::sync::Arc;

use app_core::{MemoryState, ObjectStorePort, StorageContext};
use async_trait::async_trait;
use common::AppError;
use tokio::sync::RwLock;

struct NoopObjectStore;

#[async_trait]
impl ObjectStorePort for NoopObjectStore {
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
        path: &str,
    ) -> Result<app_core::ObjectStoreMetadata, app_core::ObjectStoreHeadError> {
        Err(app_core::ObjectStoreHeadError::NotFound {
            path: path.to_string(),
        })
    }

    async fn presigned_get_url(&self, _path: &str, _ttl_secs: u64) -> Result<String, AppError> {
        Ok(String::new())
    }
}

#[test]
fn storage_context_exposes_typed_ports() {
    let storage = StorageContext::new(
        None,
        false,
        None,
        None,
        None,
        None,
        Arc::new(RwLock::new(MemoryState::default())),
        Arc::new(RwLock::new(BTreeMap::new())),
        10 * 1024 * 1024,
        true,
        Arc::new(NoopObjectStore),
        "http://localhost".to_string(),
        "/tmp/avrag-core-test".to_string(),
        3600,
        3600,
    );

    assert!(storage.document_store().is_none());
    assert!(storage.admin_store().is_none());
    assert!(storage.billing_quota().is_none());
    assert!(storage.uses_memory_adapters());
    assert!(!storage.postgres_configured());
}
