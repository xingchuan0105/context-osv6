use std::sync::Arc;

use app_core::UsageLimitStorePort;
use async_trait::async_trait;
use avrag_llm::{ChatUsageRecord, EmbeddingUsageRecord, TenantContext, UsageObserver};

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
}

#[async_trait]
impl UsageObserver for PgUsageObserver {
    async fn record_chat(&self, tenant: &TenantContext, record: &ChatUsageRecord) {
        let result = self
            .store
            .insert_usage_from_observer(
                tenant.org_id,
                tenant.user_id,
                "chat",
                &record.feature,
                &record.stage,
                &record.provider,
                &record.model,
                record.prompt_tokens as i64,
                record.completion_tokens as i64,
                record.total_tokens as i64,
                "actual",
                record.session_id,
                record.document_id,
                record.request_id.clone(),
                record.trace_id.clone(),
            )
            .await;
        if let Err(e) = result {
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
            "actual"
        } else {
            "estimated"
        };
        let total_tokens = record
            .actual_tokens
            .map(|t| t as i64)
            .unwrap_or(record.estimated_tokens as i64);

        let result = self
            .store
            .insert_usage_from_observer(
                tenant.org_id,
                tenant.user_id,
                usage_kind,
                &record.feature,
                "live",
                &record.provider,
                &record.model,
                record.estimated_tokens as i64,
                0,
                total_tokens,
                usage_source,
                None,
                None,
                None,
                None,
            )
            .await;
        if let Err(e) = result {
            tracing::warn!(
                org_id = %tenant.org_id,
                user_id = %tenant.user_id,
                error = %e,
                "PgUsageObserver::record_embedding failed; continuing"
            );
        }
    }
}
