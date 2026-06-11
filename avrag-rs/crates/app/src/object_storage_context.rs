use std::path::Path;
use std::sync::Arc;

use avrag_storage_pg::{DocumentAssetRow, ObjectStoreHandle};
use common::AppError;

use crate::lib_impl::asset_helpers::is_remote_asset_reference;
use crate::lib_impl::config_helpers::{sign_upload_payload, upload_signing_secret};

#[derive(Clone)]
pub struct ObjectStorageContext {
    object_store: Arc<ObjectStoreHandle>,
    public_base_url: String,
    object_root: String,
    upload_expire_sec: u64,
    download_expire_sec: u64,
}

impl ObjectStorageContext {
    pub fn new(
        object_store: Arc<ObjectStoreHandle>,
        public_base_url: String,
        object_root: String,
        upload_expire_sec: u64,
        download_expire_sec: u64,
    ) -> Self {
        Self {
            object_store,
            public_base_url,
            object_root,
            upload_expire_sec,
            download_expire_sec,
        }
    }

    pub fn object_store(&self) -> &Arc<ObjectStoreHandle> {
        &self.object_store
    }

    pub fn object_root_path(&self) -> &Path {
        Path::new(&self.object_root)
    }

    pub fn public_base_url(&self) -> &str {
        &self.public_base_url
    }

    pub fn download_expire_sec(&self) -> u64 {
        self.download_expire_sec
    }

    pub fn upload_expire_sec(&self) -> u64 {
        self.upload_expire_sec
    }

    pub fn signed_upload_url(
        &self,
        document_id: &str,
        object_path: &str,
        expires_at_unix: Option<u64>,
    ) -> Result<String, AppError> {
        let expires = expires_at_unix.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|value| value.as_secs())
                .unwrap_or_default()
                + self.upload_expire_sec
        });
        let signature =
            sign_upload_payload(&upload_signing_secret(), document_id, object_path, expires)?;
        Ok(format!(
            "{}/uploads/{}?expires={}&signature={}",
            self.public_base_url, document_id, expires, signature
        ))
    }

    pub fn verify_upload_signature(
        &self,
        document_id: &str,
        object_path: &str,
        expires: u64,
        signature: &str,
    ) -> Result<(), AppError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_secs())
            .unwrap_or_default();
        if expires < now {
            return Err(AppError::validation(
                "upload_url_expired",
                "upload url expired",
            ));
        }
        let expected =
            sign_upload_payload(&upload_signing_secret(), document_id, object_path, expires)?;
        if expected != signature {
            return Err(AppError::validation(
                "invalid_upload_signature",
                "invalid upload signature",
            ));
        }
        Ok(())
    }

    pub async fn resolve_citation_asset_url(
        &self,
        asset: &DocumentAssetRow,
    ) -> Option<String> {
        let storage_path = asset.storage_path.as_deref()?;
        if is_remote_asset_reference(storage_path) {
            return Some(storage_path.to_string());
        }

        match self
            .object_store
            .presigned_get_url(storage_path, self.download_expire_sec)
            .await
        {
            Ok(url) if !url.starts_with("file://") => Some(url),
            _ => Some(format!("/api/v1/chat/citations/assets/{}", asset.asset_id)),
        }
    }
}
