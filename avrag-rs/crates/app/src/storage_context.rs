use std::collections::BTreeMap;
use std::sync::Arc;

use avrag_auth::AuthContext;
use avrag_storage_pg::PgAppRepository;
use common::ApiKeyRow;
use tokio::sync::RwLock;

use crate::lib_impl::state_types::MemoryState;

#[derive(Clone)]
pub struct StorageContext {
    pg: Option<Arc<PgAppRepository>>,
    inner: Arc<RwLock<MemoryState>>,
    api_keys: Arc<RwLock<BTreeMap<String, Vec<ApiKeyRow>>>>,
    max_upload_file_size_bytes: u64,
    uses_memory_adapters: bool,
}

impl StorageContext {
    pub(crate) fn new(
        pg: Option<Arc<PgAppRepository>>,
        inner: Arc<RwLock<MemoryState>>,
        api_keys: Arc<RwLock<BTreeMap<String, Vec<ApiKeyRow>>>>,
        max_upload_file_size_bytes: u64,
        uses_memory_adapters: bool,
    ) -> Self {
        Self {
            pg,
            inner,
            api_keys,
            max_upload_file_size_bytes,
            uses_memory_adapters,
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
}
