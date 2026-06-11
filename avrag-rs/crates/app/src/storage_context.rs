use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use avrag_auth::AuthContext;
use avrag_storage_pg::{DocumentAssetRow, ObjectStoreHandle, PgAppRepository};
use common::{ApiKeyRow, AppError};
use tokio::sync::RwLock;

use crate::lib_impl::asset_helpers::is_remote_asset_reference;
use crate::lib_impl::config_helpers::{sign_upload_payload, upload_signing_secret};
use crate::lib_impl::state_types::MemoryState;

#[derive(Clone)]
pub struct StorageContext {
    pg: Option<Arc<PgAppRepository>>,
    inner: Arc<RwLock<MemoryState>>,
    api_keys: Arc<RwLock<BTreeMap<String, Vec<ApiKeyRow>>>>,
    max_upload_file_size_bytes: u64,
    uses_memory_adapters: bool,
    object_store: Arc<ObjectStoreHandle>,
    public_base_url: String,
    object_root: String,
    upload_expire_sec: u64,
    download_expire_sec: u64,
}

impl StorageContext {
    pub(crate) fn new(
        pg: Option<Arc<PgAppRepository>>,
        inner: Arc<RwLock<MemoryState>>,
        api_keys: Arc<RwLock<BTreeMap<String, Vec<ApiKeyRow>>>>,
        max_upload_file_size_bytes: u64,
        uses_memory_adapters: bool,
        object_store: Arc<ObjectStoreHandle>,
        public_base_url: String,
        object_root: String,
        upload_expire_sec: u64,
        download_expire_sec: u64,
    ) -> Self {
        Self {
            pg,
            inner,
            api_keys,
            max_upload_file_size_bytes,
            uses_memory_adapters,
            object_store,
            public_base_url,
            object_root,
            upload_expire_sec,
            download_expire_sec,
        }
    }

    pub fn pg(&self) -> Option<Arc<PgAppRepository>> {
        self.pg.clone()
    }

    pub async fn pg_ready(&self) -> bool {
        if let Some(pg) = &self.pg {
            return pg.ping().await.is_ok();
        }
        false
    }

    pub fn runtime_mode(&self) -> &'static str {
        if self.pg.is_some() {
            "postgres"
        } else {
            "memory"
        }
    }

    pub fn uses_memory_adapters(&self) -> bool {
        self.uses_memory_adapters
    }

    pub fn set_uses_memory_adapters(&mut self, value: bool) {
        self.uses_memory_adapters = value;
    }

    pub fn max_upload_file_size_bytes(&self) -> u64 {
        self.max_upload_file_size_bytes
    }

    pub(crate) fn inner(&self) -> &Arc<RwLock<MemoryState>> {
        &self.inner
    }

    pub(crate) fn api_keys(&self) -> &Arc<RwLock<BTreeMap<String, Vec<ApiKeyRow>>>> {
        &self.api_keys
    }

    pub(crate) fn current_org_id(auth: &AuthContext) -> String {
        auth.org_id().to_string()
    }

    pub(crate) fn current_user_id(auth: &AuthContext) -> String {
        auth.actor_id()
            .map(|actor_id| actor_id.into_uuid().to_string())
            .unwrap_or_else(|| common::default_user_id())
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
