use std::sync::Arc;

use app_core::StorageContext;
use contracts::auth_runtime::AuthContext;
use avrag_storage_pg::PgAppRepository;

use crate::adapters::RedisRateLimitBackend;
use crate::services::PasswordResetService;

#[derive(Clone)]
pub struct AppState {
    pub(crate) auth: AuthContext,
    pub(crate) storage: StorageContext,
    pub(crate) llm_ctx: app_chat::LlmContext,
    pub(crate) orchestrator: app_chat::OrchestratorContext,
    pub(crate) analytics: app_core::AnalyticsServiceCtx,
    pub(crate) billing: app_billing::BillingContext,
    pub(crate) admin: app_admin::AdminContext,
    pub(crate) documents: app_documents::DocumentContext,
    pub(crate) chat: app_chat::ChatContext,
    pub(crate) postgres: Option<Arc<PgAppRepository>>,
    pub(crate) redis_url: String,
    pub(crate) rate_limit_backend: Option<Arc<RedisRateLimitBackend>>,
    pub(crate) password_reset_service: PasswordResetService,
}

pub use app_core::{MemoryState, RetrievedContext, StoredDocument};
