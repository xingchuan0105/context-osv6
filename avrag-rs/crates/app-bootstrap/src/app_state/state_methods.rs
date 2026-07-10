use super::AppState;
use crate::AppBootstrapResult;
use crate::adapters::RedisRateLimitBackend;
use anyhow::Result as AnyResult;
use app_chat::agents::service::UnifiedAgentService;
use app_core::{AdminStorePort, AppConfig, BillingStorePort, ShareStorePort};
use contracts::auth_runtime::AuthContext;
use avrag_storage_pg::PgAppRepository;
use common::AppError;
use std::sync::Arc;
use uuid::Uuid;

impl From<AppBootstrapResult> for AppState {
    fn from(result: AppBootstrapResult) -> Self {
        Self {
            auth: result.auth,
            storage: result.storage,
            llm_ctx: result.llm_ctx,
            orchestrator: result.orchestrator,
            analytics: result.analytics,
            billing: result.billing,
            admin: result.admin,
            documents: result.documents,
            chat: result.chat,
            postgres: result.postgres,
            redis_url: result.redis_url,
            rate_limit_backend: result.rate_limit_backend,
            password_reset_service: crate::services::PasswordResetService::from_env(),
        }
    }
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        crate::new_memory(config).into()
    }

    pub async fn bootstrap(config: AppConfig) -> AnyResult<Self> {
        Ok(crate::bootstrap(config).await?.into())
    }

    /// Returns the runtime mode identifier ("postgres" or "memory").
    pub fn runtime_mode(&self) -> &'static str {
        self.storage.runtime_mode()
    }

    pub fn auth(&self) -> &AuthContext {
        &self.auth
    }

    // --- Product path (ADR-0007): use product Apps on AppState ---
    // Prefer: conversation() / agent() / write() / workspace() / share() /
    // billing_api() / admin_api() / admin_ops() / prefs() (see product_apps/).
    // Raw domain accessors below are for bootstrap, middleware, and tests.

    /// Raw chat context. Prefer `conversation()` / `agent()` / `write()` for product paths.
    #[deprecated(note = "use conversation() for execute, agent() for sessions")]
    pub fn chat(&self) -> &app_chat::ChatContext {
        &self.chat
    }

    /// Raw document context (needs caller-supplied auth/storage). Prefer `docs()`.
    pub fn documents(&self) -> &app_documents::DocumentContext {
        &self.documents
    }

    /// Raw admin context. Prefer `admin_api()` (keys/notifications) or `admin_ops()` (console).
    pub fn admin(&self) -> &app_admin::AdminContext {
        &self.admin
    }

    /// Storage / ports container (infra). Prefer product Apps for product work.
    pub fn storage(&self) -> &app_core::StorageContext {
        &self.storage
    }

    pub fn analytics_service_ctx(&self) -> &app_core::AnalyticsServiceCtx {
        &self.analytics
    }

    pub fn orchestrator(&self) -> &app_chat::OrchestratorContext {
        &self.orchestrator
    }

    pub fn with_auth(&self, auth: AuthContext) -> Self {
        let mut new_state = self.clone();
        new_state.auth = auth.clone();
        new_state.chat.auth = auth;
        new_state
    }

    pub fn uses_memory_adapters(&self) -> bool {
        self.storage.uses_memory_adapters()
    }

    pub async fn pg_ready(&self) -> bool {
        self.storage.pg_ready().await
    }

    pub fn postgres_configured(&self) -> bool {
        self.postgres.is_some()
    }

    pub fn postgres_pool(&self) -> Option<&sqlx::PgPool> {
        self.postgres.as_ref().map(|repo| repo.raw())
    }

    pub fn auth_store(&self) -> Option<std::sync::Arc<dyn app_core::AuthStorePort>> {
        self.storage.auth_store()
    }

    pub fn password_reset_service(&self) -> &crate::services::PasswordResetService {
        &self.password_reset_service
    }

    pub fn postgres_repo(&self) -> Option<Arc<PgAppRepository>> {
        self.postgres.clone()
    }

    /// Infra: raw admin store. Prefer `admin_ops()` in HTTP product handlers.
    pub fn admin_store(&self) -> Option<Arc<dyn AdminStorePort>> {
        self.storage.admin_store()
    }

    /// Infra: raw billing store. Prefer `billing_api()`.
    pub fn billing_store(&self) -> Option<Arc<dyn BillingStorePort>> {
        self.storage.billing_store()
    }

    /// Infra: raw share store. Prefer `share()` product App (includes `check_access`).
    pub fn share_store(&self) -> Option<Arc<dyn ShareStorePort>> {
        self.storage.share_store()
    }

    #[deprecated(note = "Use postgres_repo/postgres_pool or typed port delegates instead")]
    pub fn pg(&self) -> Option<Arc<PgAppRepository>> {
        self.postgres_repo()
    }

    pub fn agent_service(&self) -> Option<Arc<UnifiedAgentService>> {
        self.orchestrator.agent_service()
    }

    pub fn set_agent_service(&mut self, service: UnifiedAgentService) {
        self.orchestrator.set_agent_service(service);
        self.chat.orchestrator = self.orchestrator.clone();
    }

    pub fn set_uses_memory_adapters(&self, value: bool) {
        // Flag is Arc-shared across StorageContext clones (AppState + ChatContext).
        self.storage.set_uses_memory_adapters(value);
    }

    pub fn llm_ctx(&self) -> &app_chat::LlmContext {
        &self.llm_ctx
    }

    pub fn billing(&self) -> &app_billing::BillingContext {
        &self.billing
    }

    // 安全改造：提供辅助方法替代直接从 config 读取
    pub fn memory_llm_temperature(&self) -> Option<f32> {
        self.llm_ctx.memory_llm_temperature()
    }

    pub fn agent_llm_temperature(&self) -> Option<f32> {
        self.llm_ctx.agent_llm_temperature()
    }

    pub fn default_user_id(&self) -> String {
        // 返回默认用户 ID
        common::default_user_id()
    }

    pub fn redis_url(&self) -> &str {
        &self.redis_url
    }

    pub fn rate_limit_backend(&self) -> Option<&RedisRateLimitBackend> {
        self.rate_limit_backend.as_deref()
    }

    pub fn max_upload_file_size_bytes(&self) -> u64 {
        self.storage.max_upload_file_size_bytes()
    }

    pub fn analytics(&self) -> Option<&Arc<analytics::AnalyticsService>> {
        self.analytics.service()
    }

    pub fn analytics_ctx(&self) -> app_core::analytics_context::AnalyticsContext {
        self.analytics.into_context(
            self.auth.actor_id().map(|a| a.into_uuid()),
            self.auth.request_id().map(str::to_string),
        )
    }

    /// Build an analytics context for an explicitly-resolved user id (e.g. an
    /// auth flow where the actor was just resolved and is not yet on `self.auth`).
    pub fn analytics_ctx_for_user(
        &self,
        user_id: Uuid,
    ) -> app_core::analytics_context::AnalyticsContext {
        self.analytics
            .into_context(Some(user_id), self.auth.request_id().map(str::to_string))
    }

    pub async fn record_product_event_if_available(
        &self,
        event_name: analytics::ProductEventName,
        surface: analytics::Surface,
        result: analytics::ResultTag,
        session_id: Option<Uuid>,
        workspace_id: Option<Uuid>,
        metadata: serde_json::Value,
    ) {
        self.analytics
            .record_product_event_for_auth(
                &self.auth,
                event_name,
                surface,
                result,
                session_id,
                workspace_id,
                metadata,
            )
            .await;
    }
}

pub use app_billing::CostEventRecord;

impl AppState {
    pub async fn record_cost_event_if_available(&self, record: CostEventRecord<'_>) {
        app_billing::record_cost_event_if_available(
            &self.auth,
            &self.analytics.service().cloned(),
            record,
        )
        .await;
    }

    pub async fn record_storage_cost_event_if_available(
        &self,
        event_name: analytics::CostEventName,
        feature: &str,
        workspace_id: Option<Uuid>,
        storage_bytes_delta: i64,
        source: &str,
        metadata: serde_json::Value,
    ) {
        app_billing::record_storage_cost_event_if_available(
            &self.auth,
            &self.analytics.service().cloned(),
            event_name,
            feature,
            workspace_id,
            storage_bytes_delta,
            source,
            metadata,
        )
        .await;
    }

    pub async fn record_external_search_cost_event_if_available(
        &self,
        provider: &str,
        model: &str,
        workspace_id: Option<Uuid>,
        external_call_count: i64,
        metadata: serde_json::Value,
    ) {
        app_billing::record_external_search_cost_event_if_available(
            &self.auth,
            &self.analytics.service().cloned(),
            provider,
            model,
            workspace_id,
            external_call_count,
            metadata,
        )
        .await;
    }
}

#[allow(dead_code)]
pub(crate) fn non_empty_or_unknown(value: &str) -> String {
    if value.trim().is_empty() {
        "unknown".to_string()
    } else {
        value.to_string()
    }
}

impl AppState {
    pub fn signed_upload_url(
        &self,
        document_id: &str,
        object_path: &str,
        expires_at_unix: Option<u64>,
    ) -> Result<String, AppError> {
        self.storage
            .objects()
            .signed_upload_url(document_id, object_path, expires_at_unix)
    }

    pub fn verify_upload_signature(
        &self,
        document_id: &str,
        object_path: &str,
        expires: u64,
        signature: &str,
    ) -> Result<(), AppError> {
        self.storage.objects().verify_upload_signature(
            document_id,
            object_path,
            expires,
            signature,
        )
    }
}
