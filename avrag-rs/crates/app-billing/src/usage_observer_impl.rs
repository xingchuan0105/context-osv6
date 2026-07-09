//! Postgres-backed exit-metering observer for LLM / embedding calls.

use std::sync::Arc;

use app_core::{
    BillableFeature, MeteringContext, UsageLimitStorePort, UsageLimitUsageRecord, UsageSource,
};
use async_trait::async_trait;
use avrag_llm::{ChatUsageRecord, EmbeddingUsageRecord, TenantContext, UsageObserver};
use tokio::sync::RwLock;

/// Writes exit-metered usage into `llm_usage_events` via [`UsageLimitStorePort`].
#[derive(Clone)]
pub struct PgUsageObserver {
    store: Arc<dyn UsageLimitStorePort>,
    /// When false, rows do not count toward user rolling quotas (ADR 0006 §7 worker path).
    billable: bool,
}

impl std::fmt::Debug for PgUsageObserver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PgUsageObserver")
            .field("billable", &self.billable)
            .finish_non_exhaustive()
    }
}

impl PgUsageObserver {
    pub fn new(store: Arc<dyn UsageLimitStorePort>) -> Self {
        Self {
            store,
            billable: true,
        }
    }

    pub fn with_billable(mut self, billable: bool) -> Self {
        self.billable = billable;
        self
    }

    /// Test/diagnostics: whether recorded rows count toward user rolling quotas.
    #[cfg(test)]
    pub(crate) fn is_billable(&self) -> bool {
        self.billable
    }

    /// Map free-text feature tags set by `LlmClient::with_feature` to billable buckets.
    ///
    /// Prefer **exact / prefix** matches over substring `contains`, so tags like
    /// `planner` / `agent_loop` / `write:refine` stay deterministic.
    pub fn map_feature(feature: &str) -> BillableFeature {
        let f = feature.trim().to_ascii_lowercase();
        if f.is_empty() {
            return BillableFeature::Chat;
        }
        // Exact tags first.
        match f.as_str() {
            "summary" | "document_summary" => return BillableFeature::Summary,
            "planner" | "plan" | "retrieval_planner" => return BillableFeature::Planner,
            "search" | "web_search" => return BillableFeature::Search,
            "triplet" | "graph" | "graph_extraction" => {
                return BillableFeature::GraphExtraction;
            }
            "rag" | "answer" | "internal_answer" => return BillableFeature::Answer,
            "chat" | "agent_loop" | "section_index" | "ingestion" | "heavytail_writer" => {
                return BillableFeature::Chat;
            }
            "document_embedding" | "document_embedding_mm" | "embedding" => {
                // Embeddings roll under answer/RAG product meter today.
                return BillableFeature::Answer;
            }
            _ => {}
        }
        // Prefix tags (write phases, namespaced features).
        if f.starts_with("write:") || f.starts_with("write_") {
            return BillableFeature::Chat;
        }
        if f.starts_with("summary") {
            return BillableFeature::Summary;
        }
        if f.starts_with("planner") || f.starts_with("plan:") {
            return BillableFeature::Planner;
        }
        if f.starts_with("search") {
            return BillableFeature::Search;
        }
        if f.starts_with("triplet") || f.starts_with("graph") {
            return BillableFeature::GraphExtraction;
        }
        if f.starts_with("rag") || f.starts_with("answer") {
            return BillableFeature::Answer;
        }
        if f.starts_with("embedding") || f.contains("embedding") {
            return BillableFeature::Answer;
        }
        BillableFeature::Chat
    }

    pub async fn record_chat_for(&self, tenant: &TenantContext, record: &ChatUsageRecord) {
        let ctx = MeteringContext {
            user_id: tenant.user_id,
            org_id: tenant.org_id,
            feature: Self::map_feature(&record.feature),
            stage: if record.stage.is_empty() {
                record.feature.clone()
            } else {
                record.stage.clone()
            },
            session_id: record.session_id,
            document_id: record.document_id,
            request_id: record.request_id.clone(),
            trace_id: record.trace_id.clone(),
        };
        let usage = UsageLimitUsageRecord {
            provider: &record.provider,
            model: &record.model,
            prompt_tokens: record.prompt_tokens,
            completion_tokens: record.completion_tokens,
            total_tokens: record.total_tokens,
            usage_source: UsageSource::Actual,
            usage_kind: "chat",
            billable: self.billable,
        };
        if let Err(e) = self.store.insert_llm_usage_event(&ctx, usage).await {
            tracing::warn!(
                org_id = %tenant.org_id,
                user_id = %tenant.user_id,
                error = %e,
                "PgUsageObserver::record_chat failed; continuing"
            );
        }
    }

    pub async fn record_embedding_for(
        &self,
        tenant: &TenantContext,
        record: &EmbeddingUsageRecord,
    ) {
        let usage_kind = if record.actual_tokens.is_some() {
            "embedding_multimodal"
        } else {
            "embedding_text"
        };
        let usage_source = if record.actual_tokens.is_some() {
            UsageSource::Actual
        } else {
            UsageSource::Estimated
        };
        let total_tokens = record
            .actual_tokens
            .unwrap_or(record.estimated_tokens);
        let ctx = MeteringContext {
            user_id: tenant.user_id,
            org_id: tenant.org_id,
            feature: Self::map_feature(&record.feature),
            stage: "embedding".to_string(),
            session_id: None,
            document_id: None,
            request_id: None,
            trace_id: None,
        };
        let usage = UsageLimitUsageRecord {
            provider: &record.provider,
            model: &record.model,
            prompt_tokens: total_tokens,
            completion_tokens: 0,
            total_tokens,
            usage_source,
            usage_kind,
            billable: self.billable,
        };
        if let Err(e) = self.store.insert_llm_usage_event(&ctx, usage).await {
            tracing::warn!(
                org_id = %tenant.org_id,
                user_id = %tenant.user_id,
                error = %e,
                "PgUsageObserver::record_embedding failed; continuing"
            );
        }
    }
}

#[async_trait]
impl UsageObserver for PgUsageObserver {
    async fn record_chat(&self, tenant: &TenantContext, record: &ChatUsageRecord) {
        self.record_chat_for(tenant, record).await;
    }

    async fn record_embedding(&self, tenant: &TenantContext, record: &EmbeddingUsageRecord) {
        self.record_embedding_for(tenant, record).await;
    }
}

/// Worker-facing observer that attributes usage to the **current task tenant**,
/// ignoring the tenant baked into long-lived `LlmClient`/`EmbeddingClient`s.
#[derive(Clone)]
pub struct TaskTenantUsageObserver {
    inner: PgUsageObserver,
    tenant: Arc<RwLock<TenantContext>>,
}

impl std::fmt::Debug for TaskTenantUsageObserver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskTenantUsageObserver")
            .finish_non_exhaustive()
    }
}

impl TaskTenantUsageObserver {
    /// Worker metering: rebinds task tenant; rows are **non-billable** (ADR 0006 §7).
    pub fn new(store: Arc<dyn UsageLimitStorePort>, initial: TenantContext) -> Self {
        Self {
            inner: PgUsageObserver::new(store).with_billable(false),
            tenant: Arc::new(RwLock::new(initial)),
        }
    }

    pub async fn rebind(&self, tenant: TenantContext) {
        *self.tenant.write().await = tenant;
    }

    pub fn tenant_handle(&self) -> Arc<RwLock<TenantContext>> {
        self.tenant.clone()
    }

    #[cfg(test)]
    pub(crate) fn records_billable(&self) -> bool {
        self.inner.is_billable()
    }
}

#[async_trait]
impl UsageObserver for TaskTenantUsageObserver {
    async fn record_chat(&self, _tenant: &TenantContext, record: &ChatUsageRecord) {
        let tenant = self.tenant.read().await.clone();
        self.inner.record_chat_for(&tenant, record).await;
    }

    async fn record_embedding(&self, _tenant: &TenantContext, record: &EmbeddingUsageRecord) {
        let tenant = self.tenant.read().await.clone();
        self.inner.record_embedding_for(&tenant, record).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_core::{
        MeteringContext, UsageLimitOverrideRow, UsageLimitPlanPolicyRow, UsageLimitStorePort,
        UsageLimitUsageRecord,
    };
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use common::AppError;
    use std::collections::HashMap;
    use std::sync::Arc;
    use uuid::Uuid;

    #[test]
    fn map_feature_is_deterministic_for_known_tags() {
        assert_eq!(
            PgUsageObserver::map_feature("summary"),
            BillableFeature::Summary
        );
        assert_eq!(
            PgUsageObserver::map_feature("planner"),
            BillableFeature::Planner
        );
        // "plan" as substring of "airplane" must NOT map to planner.
        assert_eq!(
            PgUsageObserver::map_feature("airplane_agent"),
            BillableFeature::Chat
        );
        assert_eq!(
            PgUsageObserver::map_feature("write:refine"),
            BillableFeature::Chat
        );
        assert_eq!(
            PgUsageObserver::map_feature("triplet"),
            BillableFeature::GraphExtraction
        );
        assert_eq!(
            PgUsageObserver::map_feature("document_embedding"),
            BillableFeature::Answer
        );
        assert_eq!(
            PgUsageObserver::map_feature("agent_loop"),
            BillableFeature::Chat
        );
    }

    struct StubUsageLimitStore;

    #[async_trait]
    impl UsageLimitStorePort for StubUsageLimitStore {
        async fn insert_llm_usage_event(
            &self,
            _ctx: &MeteringContext,
            _record: UsageLimitUsageRecord<'_>,
        ) -> Result<i64, AppError> {
            Ok(0)
        }

        async fn load_user_override(
            &self,
            _user_id: Uuid,
        ) -> Result<Option<UsageLimitOverrideRow>, AppError> {
            Ok(None)
        }

        async fn get_user_plan(&self, _user_id: Uuid) -> Result<String, AppError> {
            Ok("free".into())
        }

        async fn load_plan_policy(
            &self,
            _plan_id: &str,
        ) -> Result<Option<UsageLimitPlanPolicyRow>, AppError> {
            Ok(None)
        }

        async fn sum_usage_units_since(
            &self,
            _user_id: Uuid,
            _since: DateTime<Utc>,
        ) -> Result<i64, AppError> {
            Ok(0)
        }

        async fn oldest_usage_event_since(
            &self,
            _user_id: Uuid,
            _since: DateTime<Utc>,
        ) -> Result<Option<DateTime<Utc>>, AppError> {
            Ok(None)
        }

        async fn load_usage_breakdown(
            &self,
            _user_id: Uuid,
            _since: DateTime<Utc>,
        ) -> Result<HashMap<String, i64>, AppError> {
            Ok(HashMap::new())
        }

        async fn load_model_rates(
            &self,
            _provider: &str,
            _model: &str,
        ) -> Result<(f64, f64), AppError> {
            Ok((1.0, 2.0))
        }

        async fn has_user_override(&self, _user_id: Uuid) -> Result<bool, AppError> {
            Ok(false)
        }

        async fn has_estimated_usage(&self, _user_id: Uuid) -> Result<bool, AppError> {
            Ok(false)
        }
    }

    #[test]
    fn default_observer_is_billable() {
        let observer = PgUsageObserver::new(Arc::new(StubUsageLimitStore));
        assert!(observer.is_billable());
    }

    #[test]
    fn with_billable_false_marks_non_customer_rows() {
        let observer = PgUsageObserver::new(Arc::new(StubUsageLimitStore)).with_billable(false);
        assert!(!observer.is_billable());
    }

    #[test]
    fn task_tenant_observer_is_non_billable_for_worker_path() {
        let tenant = TenantContext {
            org_id: Uuid::nil(),
            user_id: Uuid::nil(),
        };
        let observer = TaskTenantUsageObserver::new(Arc::new(StubUsageLimitStore), tenant);
        assert!(
            !observer.records_billable(),
            "ADR 0006 §7: worker metering must not count toward user rolling quotas"
        );
    }
}
