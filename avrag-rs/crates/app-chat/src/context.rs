use std::sync::Arc;

use app_admin::AdminContext;
use app_billing::{BillingContext, CostEventRecord};
use app_core::{AnalyticsServiceCtx, StorageContext};
use app_documents::DocumentContext;
use avrag_auth::AuthContext;
use app_core::ChatPersistencePort;
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
        let Some(ref analytics) = self.analytics.service() else {
            return;
        };
        let Some(user_id) = self.auth.actor_id().map(|actor| actor.into_uuid()) else {
            return;
        };

        let event = analytics::ProductEvent {
            event_id: Uuid::new_v4(),
            event_time: chrono::Utc::now(),
            user_id,
            session_id,
            notebook_id,
            surface,
            event_name,
            result,
            request_id: self.auth.request_id().map(str::to_string),
            trace_id: None,
            client_platform: "web".to_string(),
            metadata,
        };
        if let Err(error) = analytics.record_product_event(&event).await {
            telemetry::prometheus::record_dependency_failure("analytics");
            tracing::warn!(error = %error, event_name = ?event_name, "failed to record product event");
        }
    }

    pub async fn record_cost_event_if_available(&self, record: CostEventRecord<'_>) {
        app_billing::record_cost_event_if_available(
            &self.auth,
            &self.analytics.service().cloned(),
            record,
        )
        .await;
    }

    pub async fn validate_rag_doc_scope(
        &self,
        doc_scope: &[String],
    ) -> Result<(), AppError> {
        self.documents
            .validate_rag_doc_scope(&self.auth, &self.storage, doc_scope)
            .await
    }

    pub fn document_is_deleting_or_deleted(status: &contracts::documents::DocumentStatus) -> bool {
        matches!(
            status,
            contracts::documents::DocumentStatus::Deleting | contracts::documents::DocumentStatus::Deleted
        )
    }
}

pub(crate) fn map_anyhow_error(error: anyhow::Error) -> AppError {
    AppError::internal(error.to_string())
}
