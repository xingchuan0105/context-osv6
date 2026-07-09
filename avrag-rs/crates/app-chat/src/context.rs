use std::sync::Arc;

use app_admin::AdminContext;
use app_billing::{BillingContext, CostEventRecord};
use app_core::ChatPersistencePort;
use app_core::{AnalyticsServiceCtx, StorageContext};
use app_documents::DocumentContext;
use contracts::auth_runtime::AuthContext;
use common::AppError;
use uuid::Uuid;

use crate::llm_context::LlmContext;
use crate::orchestrator_context::OrchestratorContext;

/// Chat-scoped application context: auth, storage, orchestrator, billing, etc.
#[derive(Clone)]
pub struct ChatContext {
    pub auth: AuthContext,
    pub storage: StorageContext,
    pub llm_ctx: LlmContext,
    pub orchestrator: OrchestratorContext,
    pub analytics: AnalyticsServiceCtx,
    pub billing: BillingContext,
    pub admin: AdminContext,
    pub documents: DocumentContext,
}

impl ChatContext {
    pub fn new(
        auth: AuthContext,
        storage: StorageContext,
        llm_ctx: LlmContext,
        orchestrator: OrchestratorContext,
        analytics: AnalyticsServiceCtx,
        billing: BillingContext,
        admin: AdminContext,
        documents: DocumentContext,
    ) -> Self {
        Self {
            auth,
            storage,
            llm_ctx,
            orchestrator,
            analytics,
            billing,
            admin,
            documents,
        }
    }

    pub fn chat_persistence(&self) -> Option<Arc<dyn ChatPersistencePort>> {
        self.storage.chat_persistence()
    }

    pub fn uses_memory_adapters(&self) -> bool {
        self.storage.uses_memory_adapters()
    }

    /// Retrieval is routed through orchestrator `RagRuntime` (see `app_core::RetrievalPort`).
    pub fn retrieval_runtime(&self) -> Option<&std::sync::Arc<avrag_rag_core::RagRuntime>> {
        self.orchestrator.rag_runtime()
    }

    pub fn default_user_id(&self) -> String {
        common::default_user_id()
    }

    pub fn analytics_ctx(&self) -> app_core::AnalyticsContext {
        self.analytics.into_context(
            self.auth.actor_id().map(|a| a.into_uuid()),
            self.auth.request_id().map(str::to_string),
        )
    }

    pub async fn record_product_event_if_available(
        &self,
        event_name: analytics::ProductEventName,
        surface: analytics::Surface,
        result: analytics::ResultTag,
        session_id: Option<Uuid>,
        notebook_id: Option<Uuid>,
        metadata: serde_json::Value,
    ) {
        self.analytics
            .record_product_event_for_auth(
                &self.auth,
                event_name,
                surface,
                result,
                session_id,
                notebook_id,
                metadata,
            )
            .await;
    }

    pub async fn record_cost_event_if_available(&self, record: CostEventRecord<'_>) {
        app_billing::record_cost_event_if_available(
            &self.auth,
            &self.analytics.service().cloned(),
            record,
        )
        .await;
    }

    pub async fn validate_rag_doc_scope(&self, doc_scope: &[String]) -> Result<(), AppError> {
        self.documents
            .validate_rag_doc_scope(&self.auth, &self.storage, doc_scope)
            .await
    }

    pub fn document_is_deleting_or_deleted(status: &contracts::documents::DocumentStatus) -> bool {
        matches!(
            status,
            contracts::documents::DocumentStatus::Deleting
                | contracts::documents::DocumentStatus::Deleted
        )
    }
}
