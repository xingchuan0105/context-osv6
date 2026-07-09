use std::collections::BTreeMap;
use std::sync::Arc;

use app_core::{
    MemoryState, MemoryStateHandles, ObjectStoreConfig, ObjectStorePort, StorageContext,
    StorageContextParts, StorageInfra, StorageStores,
};
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
    let storage = StorageContext::from_parts(StorageContextParts {
        infra: StorageInfra {
            postgres_health: None,
            postgres_configured: false,
            uses_memory_adapters: true,
            max_upload_file_size_bytes: 10 * 1024 * 1024,
        },
        stores: StorageStores {
            document_store: None,
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
            object_store: Arc::new(NoopObjectStore),
            public_base_url: "http://localhost".to_string(),
            object_root: "/tmp/avrag-core-test".to_string(),
            upload_expire_sec: 3600,
            download_expire_sec: 3600,
        },
    });

    assert!(storage.document_store().is_none());
    assert!(storage.admin_store().is_none());
    assert!(storage.billing_quota().is_none());
    assert!(storage.uses_memory_adapters());
    assert!(!storage.postgres_configured());
    assert!(storage.infra().uses_memory_adapters);
    assert!(!storage.infra().postgres_configured);
    assert!(storage.stores().document_store.is_none());
    assert_eq!(storage.objects().public_base_url, "http://localhost");
    assert_eq!(storage.objects().upload_expire_sec, 3600);
    assert!(Arc::ptr_eq(&storage.memory().inner, storage.inner()));
}
