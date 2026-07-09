//! Postgres-backed exit-metering observer for LLM / embedding calls.

use std::sync::Arc;

use app_core::{
    BillableFeature, MeteringContext, UsageLimitStorePort, UsageLimitUsageRecord, UsageSource,
};
use async_trait::async_trait;
use avrag_llm::{ChatUsageRecord, EmbeddingUsageRecord, TenantContext, UsageObserver};

/// Writes exit-metered usage into `llm_usage_events` via [`UsageLimitStorePort`].
#[derive(Clone)]
pub struct PgUsageObserver {
    store: Arc<dyn UsageLimitStorePort>,
}

impl std::fmt::Debug for PgUsageObserver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PgUsageObserver").finish_non_exhaustive()
    }
}

impl PgUsageObserver {
    pub fn new(store: Arc<dyn UsageLimitStorePort>) -> Self {
        Self { store }
    }

    fn map_feature(feature: &str) -> BillableFeature {
        let f = feature.trim().to_ascii_lowercase();
        if f.contains("summary") {
            BillableFeature::Summary
        } else if f.contains("planner") || f.contains("plan") {
            BillableFeature::Planner
        } else if f.contains("search") {
            BillableFeature::Search
        } else if f.contains("graph") || f.contains("triplet") {
            BillableFeature::GraphExtraction
        } else if f.contains("rag") || f.contains("answer") {
            BillableFeature::Answer
        } else if f.starts_with("write:") || f.contains("writer") || f.contains("section_index") {
            // Write-mode phases and section-index stay on chat quota buckets.
            BillableFeature::Chat
        } else if f.contains("embedding") {
            BillableFeature::Answer
        } else {
            BillableFeature::Chat
        }
    }
}

#[async_trait]
impl UsageObserver for PgUsageObserver {
    async fn record_chat(&self, tenant: &TenantContext, record: &ChatUsageRecord) {
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

    async fn record_embedding(&self, tenant: &TenantContext, record: &EmbeddingUsageRecord) {
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
