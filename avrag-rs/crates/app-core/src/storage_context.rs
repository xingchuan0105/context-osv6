use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use avrag_auth::AuthContext;
use common::{ApiKeyRow, AppError};
use tokio::sync::RwLock;

use crate::admin_store::AdminStorePort;
use crate::auth_store::AuthStorePort;
use crate::billing_quota::BillingQuotaPort;
use crate::billing_store::BillingStorePort;
use crate::chat_persistence::ChatPersistencePort;
use crate::share_store::ShareStorePort;
use crate::config_helpers::{
    is_remote_asset_reference, sign_upload_payload, upload_signing_secret,
};
use crate::domain_rows::DocumentAssetRow;
use crate::object_store_port::ObjectStorePort;
use crate::postgres_health::PostgresHealthPort;
use crate::document_store::DocumentStorePort;
use crate::state_types::MemoryState;

#[derive(Clone)]
pub struct StorageContext {
    postgres_health: Option<Arc<dyn PostgresHealthPort>>,
    postgres_configured: bool,
    document_store: Option<Arc<dyn DocumentStorePort>>,
    auth_store: Option<Arc<dyn AuthStorePort>>,
    admin_store: Option<Arc<dyn AdminStorePort>>,
    billing_quota: Option<Arc<dyn BillingQuotaPort>>,
    billing_store: Option<Arc<dyn BillingStorePort>>,
    share_store: Option<Arc<dyn ShareStorePort>>,
    chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    inner: Arc<RwLock<MemoryState>>,
    api_keys: Arc<RwLock<BTreeMap<String, Vec<ApiKeyRow>>>>,
    max_upload_file_size_bytes: u64,
    uses_memory_adapters: bool,
    object_store: Arc<dyn ObjectStorePort>,
    public_base_url: String,
    object_root: String,
    upload_expire_sec: u64,
    download_expire_sec: u64,
}

impl StorageContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        postgres_health: Option<Arc<dyn PostgresHealthPort>>,
        postgres_configured: bool,
        document_store: Option<Arc<dyn DocumentStorePort>>,
        auth_store: Option<Arc<dyn AuthStorePort>>,
        admin_store: Option<Arc<dyn AdminStorePort>>,
        billing_quota: Option<Arc<dyn BillingQuotaPort>>,
        billing_store: Option<Arc<dyn BillingStorePort>>,
        share_store: Option<Arc<dyn ShareStorePort>>,
        chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
        inner: Arc<RwLock<MemoryState>>,
        api_keys: Arc<RwLock<BTreeMap<String, Vec<ApiKeyRow>>>>,
        max_upload_file_size_bytes: u64,
        uses_memory_adapters: bool,
        object_store: Arc<dyn ObjectStorePort>,
        public_base_url: String,
        object_root: String,
        upload_expire_sec: u64,
        download_expire_sec: u64,
    ) -> Self {
        Self {
            postgres_health,
            postgres_configured,
            document_store,
            auth_store,
            admin_store,
            billing_quota,
            billing_store,
            share_store,
            chat_persistence,
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

    pub fn document_store(&self) -> Option<Arc<dyn DocumentStorePort>> {
        self.document_store.clone()
    }

    pub fn auth_store(&self) -> Option<Arc<dyn AuthStorePort>> {
        self.auth_store.clone()
    }

    pub fn admin_store(&self) -> Option<Arc<dyn AdminStorePort>> {
        self.admin_store.clone()
    }

    pub fn billing_quota(&self) -> Option<Arc<dyn BillingQuotaPort>> {
        self.billing_quota.clone()
    }

    pub fn billing_store(&self) -> Option<Arc<dyn BillingStorePort>> {
        self.billing_store.clone()
    }

    pub fn share_store(&self) -> Option<Arc<dyn ShareStorePort>> {
        self.share_store.clone()
    }

    pub fn chat_persistence(&self) -> Option<Arc<dyn ChatPersistencePort>> {
        self.chat_persistence.clone()
    }

    pub async fn pg_ready(&self) -> bool {
        if let Some(health) = &self.postgres_health {
            return health.ping().await.is_ok();
        }
        false
    }

    pub fn runtime_mode(&self) -> &'static str {
        if !self.postgres_configured {
            return "memory";
        }
        if self.document_store.is_some() && !self.uses_memory_adapters {
            "postgres"
        } else {
            "postgres_degraded"
        }
    }

    pub fn postgres_configured(&self) -> bool {
        self.postgres_configured
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

    pub fn inner(&self) -> &Arc<RwLock<MemoryState>> {
        &self.inner
    }

    pub fn api_keys(&self) -> &Arc<RwLock<BTreeMap<String, Vec<ApiKeyRow>>>> {
        &self.api_keys
    }

    pub fn current_org_id(auth: &AuthContext) -> String {
        auth.org_id().to_string()
    }

    pub fn current_user_id(auth: &AuthContext) -> String {
        auth.actor_id()
            .map(|actor_id| actor_id.into_uuid().to_string())
            .unwrap_or_else(|| common::default_user_id())
    }

    pub fn object_store(&self) -> &Arc<dyn ObjectStorePort> {
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
