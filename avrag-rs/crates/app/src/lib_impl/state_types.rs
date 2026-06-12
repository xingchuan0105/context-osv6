use avrag_auth::AuthContext;
use app_core::StorageContext;

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
    pub(crate) redis_url: String,
}

pub use app_core::{MemoryState, RetrievedContext, StoredDocument};
