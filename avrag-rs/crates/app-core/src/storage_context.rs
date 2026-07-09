use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use common::{ApiKeyRow, AppError};
use tokio::sync::RwLock;

use crate::admin_store::AdminStorePort;
use crate::auth_store::AuthStorePort;
use crate::billing_quota::BillingQuotaPort;
use crate::billing_store::BillingStorePort;
use crate::chat_persistence::ChatPersistencePort;
use crate::config_helpers::{
    is_remote_asset_reference, sign_upload_payload, upload_signing_secret,
};
use crate::document_store::DocumentStorePort;
use crate::domain_rows::DocumentAssetRow;
use crate::object_store_port::ObjectStorePort;
use crate::postgres_health::PostgresHealthPort;
use crate::share_store::ShareStorePort;
use crate::state_types::MemoryState;

/// Domain store ports held by storage.
#[derive(Clone)]
pub struct StorageStores {
    pub document_store: Option<Arc<dyn DocumentStorePort>>,
    pub auth_store: Option<Arc<dyn AuthStorePort>>,
    pub admin_store: Option<Arc<dyn AdminStorePort>>,
    pub billing_quota: Option<Arc<dyn BillingQuotaPort>>,
    pub billing_store: Option<Arc<dyn BillingStorePort>>,
    pub share_store: Option<Arc<dyn ShareStorePort>>,
    pub chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
}

/// Infra flags and health for storage runtime mode.
#[derive(Clone)]
pub struct StorageInfra {
    pub postgres_health: Option<Arc<dyn PostgresHealthPort>>,
    pub postgres_configured: bool,
    pub uses_memory_adapters: bool,
    pub max_upload_file_size_bytes: u64,
}

impl StorageInfra {
    pub fn set_uses_memory_adapters(&mut self, value: bool) {
        self.uses_memory_adapters = value;
    }
}

/// Object-store handle and signed-URL configuration.
#[derive(Clone)]
pub struct ObjectStoreConfig {
    pub object_store: Arc<dyn ObjectStorePort>,
    pub public_base_url: String,
    pub object_root: String,
    pub upload_expire_sec: u64,
    pub download_expire_sec: u64,
}

impl ObjectStoreConfig {
    pub fn object_root_path(&self) -> &Path {
        Path::new(&self.object_root)
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

    pub async fn resolve_citation_asset_url(&self, asset: &DocumentAssetRow) -> Option<String> {
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

/// In-memory state handles (memory mode / fallback).
#[derive(Clone)]
pub struct MemoryStateHandles {
    pub inner: Arc<RwLock<MemoryState>>,
    pub api_keys: Arc<RwLock<BTreeMap<String, Vec<ApiKeyRow>>>>,
    pub api_key_hashes: Arc<RwLock<BTreeMap<String, crate::api_key::MemoryApiKeyRecord>>>,
}

/// Grouped constructor input for [`StorageContext`].
#[derive(Clone)]
pub struct StorageContextParts {
    pub infra: StorageInfra,
    pub stores: StorageStores,
    pub memory: MemoryStateHandles,
    pub objects: ObjectStoreConfig,
}

/// Facade over grouped storage concerns. Accessor signatures stay stable for callers.
#[derive(Clone)]
pub struct StorageContext {
    infra: StorageInfra,
    stores: StorageStores,
    memory: MemoryStateHandles,
    objects: ObjectStoreConfig,
}

impl StorageContext {
    pub fn from_parts(parts: StorageContextParts) -> Self {
        Self {
            infra: parts.infra,
            stores: parts.stores,
            memory: parts.memory,
            objects: parts.objects,
        }
    }

    pub fn document_store(&self) -> Option<Arc<dyn DocumentStorePort>> {
        self.stores.document_store.clone()
    }

    pub fn auth_store(&self) -> Option<Arc<dyn AuthStorePort>> {
        self.stores.auth_store.clone()
    }

    pub fn admin_store(&self) -> Option<Arc<dyn AdminStorePort>> {
        self.stores.admin_store.clone()
    }

    pub fn billing_quota(&self) -> Option<Arc<dyn BillingQuotaPort>> {
        self.stores.billing_quota.clone()
    }

    pub fn billing_store(&self) -> Option<Arc<dyn BillingStorePort>> {
        self.stores.billing_store.clone()
    }

    pub fn share_store(&self) -> Option<Arc<dyn ShareStorePort>> {
        self.stores.share_store.clone()
    }

    pub fn chat_persistence(&self) -> Option<Arc<dyn ChatPersistencePort>> {
        self.stores.chat_persistence.clone()
    }

    pub async fn pg_ready(&self) -> bool {
        if let Some(health) = &self.infra.postgres_health {
            return health.ping().await.is_ok();
        }
        false
    }

    pub fn runtime_mode(&self) -> &'static str {
        if !self.infra.postgres_configured {
            return "memory";
        }
        if self.stores.document_store.is_some() && !self.infra.uses_memory_adapters {
            "postgres"
        } else {
            "postgres_degraded"
        }
    }

    pub fn postgres_configured(&self) -> bool {
        self.infra.postgres_configured
    }

    pub fn uses_memory_adapters(&self) -> bool {
        self.infra.uses_memory_adapters
    }

    pub fn set_uses_memory_adapters(&mut self, value: bool) {
        self.infra.set_uses_memory_adapters(value);
    }

    pub fn max_upload_file_size_bytes(&self) -> u64 {
        self.infra.max_upload_file_size_bytes
    }

    pub fn inner(&self) -> &Arc<RwLock<MemoryState>> {
        &self.memory.inner
    }

    pub fn api_keys(&self) -> &Arc<RwLock<BTreeMap<String, Vec<ApiKeyRow>>>> {
        &self.memory.api_keys
    }

    pub fn api_key_hashes(
        &self,
    ) -> &Arc<RwLock<BTreeMap<String, crate::api_key::MemoryApiKeyRecord>>> {
        &self.memory.api_key_hashes
    }

    pub fn object_store(&self) -> &Arc<dyn ObjectStorePort> {
        &self.objects.object_store
    }

    pub fn object_root_path(&self) -> &Path {
        self.objects.object_root_path()
    }

    pub fn public_base_url(&self) -> &str {
        &self.objects.public_base_url
    }

    pub fn download_expire_sec(&self) -> u64 {
        self.objects.download_expire_sec
    }

    pub fn upload_expire_sec(&self) -> u64 {
        self.objects.upload_expire_sec
    }

    pub fn signed_upload_url(
        &self,
        document_id: &str,
        object_path: &str,
        expires_at_unix: Option<u64>,
    ) -> Result<String, AppError> {
        self.objects
            .signed_upload_url(document_id, object_path, expires_at_unix)
    }

    pub fn verify_upload_signature(
        &self,
        document_id: &str,
        object_path: &str,
        expires: u64,
        signature: &str,
    ) -> Result<(), AppError> {
        self.objects
            .verify_upload_signature(document_id, object_path, expires, signature)
    }

    pub async fn resolve_citation_asset_url(&self, asset: &DocumentAssetRow) -> Option<String> {
        self.objects.resolve_citation_asset_url(asset).await
    }
}
