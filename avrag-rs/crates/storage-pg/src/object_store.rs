use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use aws_credential_types::Credentials;
use aws_sdk_s3::config::Builder as S3ConfigBuilder;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use tokio::fs;

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

    pub async fn get(&self, path: &str) -> Result<Vec<u8>> {
        match self {
            Self::Local(store) => store.get(path).await,
            Self::S3(store) => store.get(path).await,
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
}

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

    pub async fn get(&self, path: &str) -> Result<Vec<u8>> {
        let full = self.full_path(path);
        let bytes = fs::read(&full).await?;
        Ok(bytes)
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
}

pub struct S3ObjectStore {
    client: aws_sdk_s3::Client,
    bucket: String,
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
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(path)
            .body(ByteStream::from(bytes.to_vec()))
            .send()
            .await
            .context("s3 put object failed")?;
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
}
