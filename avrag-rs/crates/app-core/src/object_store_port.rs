use std::pin::Pin;

use async_trait::async_trait;
use bytes::Bytes;
use common::AppError;
use futures::Stream;

pub type ObjectStoreUploadStream =
    Pin<Box<dyn Stream<Item = Result<Bytes, AppError>> + Send>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectStoreMetadata {
    pub size_bytes: u64,
    pub sha256_hex: Option<String>,
    pub content_type: Option<String>,
    pub etag: Option<String>,
}

#[derive(Debug)]
pub enum ObjectStoreHeadError {
    NotFound { path: String },
    NotFile { path: String },
    Backend(String),
}

/// Object storage boundary — implementations live in storage adapters (local/S3).
#[async_trait]
pub trait ObjectStorePort: Send + Sync {
    async fn put(&self, path: &str, bytes: &[u8]) -> Result<(), AppError>;

    async fn put_stream(
        &self,
        path: &str,
        stream: ObjectStoreUploadStream,
    ) -> Result<(), AppError>;

    async fn get(&self, path: &str) -> Result<Vec<u8>, AppError>;

    async fn head(&self, path: &str) -> Result<ObjectStoreMetadata, ObjectStoreHeadError>;

    async fn presigned_get_url(&self, path: &str, ttl_secs: u64) -> Result<String, AppError>;
}
