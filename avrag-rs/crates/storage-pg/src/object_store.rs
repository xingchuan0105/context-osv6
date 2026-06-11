use bytes::Bytes;
use futures::Stream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use aws_credential_types::Credentials;
use aws_sdk_s3::config::Builder as S3ConfigBuilder;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::ChecksumMode;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::fs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectStoreMetadata {
    pub size_bytes: u64,
    pub sha256_hex: Option<String>,
    pub content_type: Option<String>,
    pub etag: Option<String>,
}

#[derive(Debug, Error)]
pub enum ObjectStoreHeadError {
    #[error("object not found: {path}")]
    NotFound { path: String },
    #[error("object path is not a file: {path}")]
    NotFile { path: String },
    #[error(transparent)]
    Backend(#[from] anyhow::Error),
}

pub enum ObjectStoreHandle {
    Local(LocalObjectStore),
    S3(S3ObjectStore),
}

impl ObjectStoreHandle {
    pub fn local(root: PathBuf) -> Self {
        Self::Local(LocalObjectStore::new(root))
    }

    pub async fn put(&self, path: &str, bytes: &[u8]) -> Result<()> {
        match self {
            Self::Local(store) => store.put(path, bytes).await,
            Self::S3(store) => store.put(path, bytes).await,
        }
    }

    pub async fn put_stream<S, E>(&self, path: &str, stream: S) -> Result<()>
    where
        S: Stream<Item = std::result::Result<Bytes, E>> + Send + Sync + Unpin + 'static,
        E: std::error::Error + Send + Sync + 'static,
    {
        match self {
            Self::Local(store) => store.put_stream(path, stream).await,
            Self::S3(store) => store.put_stream(path, stream).await,
        }
    }

    pub async fn get(&self, path: &str) -> Result<Vec<u8>> {
        match self {
            Self::Local(store) => store.get(path).await,
            Self::S3(store) => store.get(path).await,
        }
    }

    pub async fn head(
        &self,
        path: &str,
    ) -> std::result::Result<ObjectStoreMetadata, ObjectStoreHeadError> {
        match self {
            Self::Local(store) => store.head(path).await,
            Self::S3(store) => store.head(path).await,
        }
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        match self {
            Self::Local(store) => store.delete(path).await,
            Self::S3(store) => store.delete(path).await,
        }
    }

    pub async fn presigned_put_url(&self, path: &str, ttl_secs: u64) -> Result<String> {
        match self {
            Self::Local(store) => store.presigned_put_url(path, ttl_secs).await,
            Self::S3(store) => store.presigned_put_url(path, ttl_secs).await,
        }
    }

    pub async fn presigned_get_url(&self, path: &str, ttl_secs: u64) -> Result<String> {
        match self {
            Self::Local(store) => store.presigned_get_url(path, ttl_secs).await,
            Self::S3(store) => store.presigned_get_url(path, ttl_secs).await,
        }
    }

    pub fn is_remote(&self) -> bool {
        matches!(self, Self::S3(_))
    }

    pub async fn list(&self) -> Result<Vec<String>> {
        match self {
            Self::Local(store) => store.list().await,
            Self::S3(store) => store.list().await,
        }
    }
}

#[derive(Clone)]
pub struct LocalObjectStore {
    root: PathBuf,
}

impl LocalObjectStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn full_path(&self, path: &str) -> PathBuf {
        self.root.join(path)
    }

    pub async fn put(&self, path: &str, bytes: &[u8]) -> Result<()> {
        let full = self.full_path(path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(&full, bytes).await?;
        Ok(())
    }

    pub async fn put_stream<S, E>(&self, path: &str, mut stream: S) -> Result<()>
    where
        S: Stream<Item = std::result::Result<Bytes, E>> + Unpin + Send + Sync + 'static,
        E: std::error::Error + Send + Sync + 'static,
    {
        use futures::StreamExt;
        use tokio::io::AsyncWriteExt;

        let full = self.full_path(path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut file = fs::File::create(&full).await?;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| anyhow!(e))?;
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        Ok(())
    }

    pub async fn get(&self, path: &str) -> Result<Vec<u8>> {
        let full = self.full_path(path);
        let bytes = fs::read(&full).await?;
        Ok(bytes)
    }

    pub async fn head(
        &self,
        path: &str,
    ) -> std::result::Result<ObjectStoreMetadata, ObjectStoreHeadError> {
        let full = self.full_path(path);
        let metadata = fs::metadata(&full)
            .await
            .map_err(|error| local_head_io_error(&full, "stat", error))?;
        if !metadata.is_file() {
            return Err(ObjectStoreHeadError::NotFile {
                path: full.display().to_string(),
            });
        }

        use tokio::io::AsyncReadExt;
        let mut file = fs::File::open(&full)
            .await
            .map_err(|error| local_head_io_error(&full, "open", error))?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];
        loop {
            let n = file
                .read(&mut buffer)
                .await
                .map_err(|error| local_head_io_error(&full, "read", error))?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }

        Ok(ObjectStoreMetadata {
            size_bytes: metadata.len(),
            sha256_hex: Some(hex::encode(hasher.finalize())),
            content_type: None,
            etag: None,
        })
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        let full = self.full_path(path);
        if fs::try_exists(&full).await.unwrap_or(false) {
            fs::remove_file(&full).await?;
        }
        Ok(())
    }

    pub async fn presigned_put_url(&self, path: &str, _ttl_secs: u64) -> Result<String> {
        Ok(format!("file://{}", path))
    }

    pub async fn presigned_get_url(&self, path: &str, _ttl_secs: u64) -> Result<String> {
        Ok(format!("file://{}", path))
    }

    pub async fn list(&self) -> Result<Vec<String>> {
        let mut paths = Vec::new();
        let mut dirs = vec![self.root.clone()];
        while let Some(dir) = dirs.pop() {
            let mut entries = fs::read_dir(&dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if entry.file_type().await?.is_dir() {
                    dirs.push(path);
                } else {
                    let relative = path.strip_prefix(&self.root)?.to_string_lossy().to_string();
                    paths.push(relative);
                }
            }
        }
        Ok(paths)
    }
}

#[derive(Clone)]
pub struct S3ObjectStore {
    client: aws_sdk_s3::Client,
    bucket: String,
}

impl Clone for ObjectStoreHandle {
    fn clone(&self) -> Self {
        match self {
            Self::Local(store) => Self::Local(store.clone()),
            Self::S3(store) => Self::S3(store.clone()),
        }
    }
}

impl S3ObjectStore {
    pub async fn new(
        endpoint: String,
        bucket: String,
        region: String,
        access_key: String,
        secret_key: String,
        use_path_style: bool,
    ) -> Result<Self> {
        let shared_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_config::Region::new(region))
            .credentials_provider(Credentials::new(
                access_key,
                secret_key,
                None,
                None,
                "context-osv6",
            ))
            .load()
            .await;

        let mut builder = S3ConfigBuilder::from(&shared_config).force_path_style(use_path_style);
        if !endpoint.trim().is_empty() {
            builder = builder.endpoint_url(endpoint);
        }
        let client = aws_sdk_s3::Client::from_conf(builder.build());
        Ok(Self { client, bucket })
    }

    pub async fn put(&self, path: &str, bytes: &[u8]) -> Result<()> {
        let checksum_sha256 = BASE64_STANDARD.encode(Sha256::digest(bytes));
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(path)
            .checksum_sha256(checksum_sha256)
            .body(ByteStream::from(bytes.to_vec()))
            .send()
            .await
            .context("s3 put object failed")?;
        Ok(())
    }

    pub async fn put_stream<S, E>(&self, path: &str, stream: S) -> Result<()>
    where
        S: Stream<Item = std::result::Result<Bytes, E>> + Send + Sync + 'static,
        E: std::error::Error + Send + Sync + 'static,
    {
        use futures::StreamExt;
        let mut bytes = bytes::BytesMut::new();
        let mut pinned_stream = Box::pin(stream);
        while let Some(chunk) = pinned_stream.next().await {
            bytes.extend_from_slice(&chunk.map_err(|e| anyhow!(e))?);
        }
        let body = ByteStream::from(bytes.freeze());
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(path)
            .body(body)
            .send()
            .await
            .context("s3 put stream failed")?;
        Ok(())
    }

    pub async fn get(&self, path: &str) -> Result<Vec<u8>> {
        let response = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await
            .context("s3 get object failed")?;
        let data = response
            .body
            .collect()
            .await
            .context("s3 collect body failed")?;
        Ok(data.into_bytes().to_vec())
    }

    pub async fn head(
        &self,
        path: &str,
    ) -> std::result::Result<ObjectStoreMetadata, ObjectStoreHeadError> {
        let response = match self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(path)
            .checksum_mode(ChecksumMode::Enabled)
            .send()
            .await
        {
            Ok(response) => response,
            Err(error) => {
                if error
                    .as_service_error()
                    .map(|service_error| service_error.is_not_found())
                    .unwrap_or(false)
                {
                    return Err(ObjectStoreHeadError::NotFound {
                        path: format!("s3://{}/{}", self.bucket, path),
                    });
                }
                return Err(ObjectStoreHeadError::Backend(anyhow!(error).context(
                    format!("s3 head object failed for s3://{}/{}", self.bucket, path),
                )));
            }
        };

        let content_length = response.content_length().ok_or_else(|| {
            ObjectStoreHeadError::Backend(anyhow!("s3 head object missing content length"))
        })?;
        if content_length < 0 {
            return Err(ObjectStoreHeadError::Backend(anyhow!(
                "s3 head object returned negative content length: {content_length}"
            )));
        }

        let sha256_hex = response
            .checksum_sha256()
            .and_then(normalize_sha256_value)
            .or_else(|| response.metadata().and_then(sha256_from_user_metadata));

        Ok(ObjectStoreMetadata {
            size_bytes: u64::try_from(content_length).map_err(|_| {
                ObjectStoreHeadError::Backend(anyhow!(
                    "s3 head object content length does not fit u64"
                ))
            })?,
            sha256_hex,
            content_type: response.content_type().map(ToOwned::to_owned),
            etag: response.e_tag().map(ToOwned::to_owned),
        })
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(path)
            .send()
            .await
            .context("s3 delete object failed")?;
        Ok(())
    }

    pub async fn presigned_put_url(&self, path: &str, ttl_secs: u64) -> Result<String> {
        let request = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(path)
            .presigned(
                PresigningConfig::expires_in(Duration::from_secs(ttl_secs.max(1)))
                    .context("s3 presign config failed")?,
            )
            .await
            .context("s3 presign put failed")?;
        Ok(request.uri().to_string())
    }

    pub async fn presigned_get_url(&self, path: &str, ttl_secs: u64) -> Result<String> {
        let request = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(path)
            .presigned(
                PresigningConfig::expires_in(Duration::from_secs(ttl_secs.max(1)))
                    .context("s3 presign config failed")?,
            )
            .await
            .context("s3 presign get failed")?;
        Ok(request.uri().to_string())
    }

    pub async fn list(&self) -> Result<Vec<String>> {
        let mut paths = Vec::new();
        let mut continuation_token: Option<String> = None;
        loop {
            let mut req = self.client.list_objects_v2().bucket(&self.bucket);
            if let Some(ref token) = continuation_token {
                req = req.continuation_token(token);
            }
            let resp = req.send().await.context("s3 list objects failed")?;
            if let Some(contents) = resp.contents {
                for obj in contents {
                    if let Some(key) = obj.key {
                        paths.push(key);
                    }
                }
            }
            continuation_token = resp.next_continuation_token;
            if continuation_token.is_none() {
                break;
            }
        }
        Ok(paths)
    }
}

fn sha256_from_user_metadata(
    metadata: &std::collections::HashMap<String, String>,
) -> Option<String> {
    metadata.iter().find_map(|(key, value)| {
        let key = key.to_ascii_lowercase();
        if matches!(
            key.as_str(),
            "sha256" | "sha256_hex" | "sha256-hex" | "x-amz-meta-sha256" | "x-amz-meta-sha256-hex"
        ) {
            normalize_sha256_value(value)
        } else {
            None
        }
    })
}

fn normalize_sha256_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.len() == 64 && lower.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Some(lower);
    }

    BASE64_STANDARD
        .decode(trimmed)
        .ok()
        .filter(|bytes| bytes.len() == 32)
        .map(hex::encode)
}

fn local_head_io_error(path: &Path, action: &str, error: std::io::Error) -> ObjectStoreHeadError {
    if error.kind() == std::io::ErrorKind::NotFound {
        return ObjectStoreHeadError::NotFound {
            path: path.display().to_string(),
        };
    }

    ObjectStoreHeadError::Backend(
        anyhow!(error).context(format!("failed to {action} object at {}", path.display())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[tokio::test]
    async fn local_head_returns_size_and_sha256() {
        let root =
            std::env::temp_dir().join(format!("avrag-local-object-head-test-{}", Uuid::new_v4()));
        let store = ObjectStoreHandle::local(root.clone());

        store.put("nested/hello.txt", b"hello world").await.unwrap();
        let metadata = store.head("nested/hello.txt").await.unwrap();

        assert_eq!(metadata.size_bytes, 11);
        assert_eq!(
            metadata.sha256_hex.as_deref(),
            Some("b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9")
        );
        assert_eq!(metadata.content_type, None);
        assert_eq!(metadata.etag, None);

        let _ = fs::remove_dir_all(root).await;
    }

    #[tokio::test]
    async fn local_head_classifies_missing_object() {
        let root = std::env::temp_dir().join(format!(
            "avrag-local-object-head-missing-test-{}",
            Uuid::new_v4()
        ));
        let store = ObjectStoreHandle::local(root.clone());

        let error = store.head("missing.txt").await.unwrap_err();

        assert!(matches!(error, ObjectStoreHeadError::NotFound { .. }));
        let _ = fs::remove_dir_all(root).await;
    }

    #[tokio::test]
    async fn local_head_classifies_directory_as_not_file() {
        let root = std::env::temp_dir().join(format!(
            "avrag-local-object-head-directory-test-{}",
            Uuid::new_v4()
        ));
        let store = ObjectStoreHandle::local(root.clone());
        fs::create_dir_all(root.join("nested/dir")).await.unwrap();

        let error = store.head("nested/dir").await.unwrap_err();

        assert!(matches!(error, ObjectStoreHeadError::NotFile { .. }));
        let _ = fs::remove_dir_all(root).await;
    }
}
