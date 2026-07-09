use std::sync::Arc;

use app_core::{
    ObjectStoreHeadError, ObjectStoreMetadata, ObjectStorePort,
    object_store_port::ObjectStoreUploadStream,
};
use async_trait::async_trait;
use avrag_storage_pg::ObjectStoreHandle;
use common::AppError;
pub struct ObjectStorePortAdapter {
    inner: Arc<ObjectStoreHandle>,
}

impl ObjectStorePortAdapter {
    pub fn new(inner: Arc<ObjectStoreHandle>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl ObjectStorePort for ObjectStorePortAdapter {
    async fn put(&self, path: &str, bytes: &[u8]) -> Result<(), AppError> {
        self.inner
            .put(path, bytes)
            .await
            .map_err(|error| AppError::internal(error.to_string()))
    }

    async fn put_stream(
        &self,
        path: &str,
        stream: ObjectStoreUploadStream,
    ) -> Result<(), AppError> {
        use futures::TryStreamExt;
        let bytes = stream
            .try_fold(Vec::new(), |mut acc, chunk| async move {
                acc.extend_from_slice(&chunk);
                Ok(acc)
            })
            .await?;
        self.put(path, &bytes).await
    }

    async fn get(&self, path: &str) -> Result<Vec<u8>, AppError> {
        self.inner
            .get(path)
            .await
            .map_err(|error| AppError::internal(error.to_string()))
    }

    async fn head(&self, path: &str) -> Result<ObjectStoreMetadata, ObjectStoreHeadError> {
        self.inner
            .head(path)
            .await
            .map(|metadata| ObjectStoreMetadata {
                size_bytes: metadata.size_bytes,
                sha256_hex: metadata.sha256_hex,
                content_type: metadata.content_type,
                etag: metadata.etag,
            })
            .map_err(|error| match error {
                avrag_storage_pg::ObjectStoreHeadError::NotFound { path } => {
                    ObjectStoreHeadError::NotFound { path }
                }
                avrag_storage_pg::ObjectStoreHeadError::NotFile { path } => {
                    ObjectStoreHeadError::NotFile { path }
                }
                avrag_storage_pg::ObjectStoreHeadError::Backend(error) => {
                    ObjectStoreHeadError::Backend(error.to_string())
                }
            })
    }

    async fn presigned_get_url(&self, path: &str, ttl_secs: u64) -> Result<String, AppError> {
        self.inner
            .presigned_get_url(path, ttl_secs)
            .await
            .map_err(|error| AppError::internal(error.to_string()))
    }
}
