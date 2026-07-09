use std::sync::Arc;

use contracts::auth_runtime::AuthContext;
use uuid::Uuid;

/// Lightweight wrapper around the analytics service for per-app storage.
///
/// Holds the service reference. Per-request context (actor_id, request_id)
/// is created on-demand via `into_context()`.
///
/// **Canonical product-event entry points live here** — prefer
/// [`Self::record_product_event_for_auth`] / [`AnalyticsContext::record_product_event`]
/// over crate-local wrappers.
#[derive(Clone)]
pub struct AnalyticsServiceCtx {
    service: Option<Arc<analytics::AnalyticsService>>,
}

impl AnalyticsServiceCtx {
    pub fn new(service: Option<Arc<analytics::AnalyticsService>>) -> Self {
        Self { service }
    }

    pub fn is_available(&self) -> bool {
        self.service.is_some()
    }

    pub fn service(&self) -> Option<&Arc<analytics::AnalyticsService>> {
        self.service.as_ref()
    }

    pub fn into_context(
        &self,
        actor_id: Option<Uuid>,
        request_id: Option<String>,
    ) -> AnalyticsContext {
        AnalyticsContext {
            analytics: self.service.clone(),
            actor_id,
            request_id,
        }
    }

    /// Single free-function-style helper used by app-documents / transport / chat.
    pub async fn record_product_event_for_auth(
        &self,
        auth: &AuthContext,
        event_name: analytics::ProductEventName,
        surface: analytics::Surface,
        result: analytics::ResultTag,
        session_id: Option<Uuid>,
        notebook_id: Option<Uuid>,
        metadata: serde_json::Value,
    ) {
        self.into_context(
            auth.actor_id().map(|actor| actor.into_uuid()),
            auth.request_id().map(str::to_string),
        )
        .record_product_event(
            event_name,
            surface,
            result,
            session_id,
            notebook_id,
            metadata,
        )
        .await;
    }

    /// When auth is not available but a user id is (e.g. JWT login path).
    pub async fn record_product_event_for_user(
        &self,
        user_id: Uuid,
        request_id: Option<String>,
        event_name: analytics::ProductEventName,
        surface: analytics::Surface,
        result: analytics::ResultTag,
        session_id: Option<Uuid>,
        notebook_id: Option<Uuid>,
        metadata: serde_json::Value,
    ) {
        self.into_context(Some(user_id), request_id)
            .record_product_event(
                event_name,
                surface,
                result,
                session_id,
                notebook_id,
                metadata,
            )
            .await;
    }
}

/// Per-request analytics recording context.
///
/// Holds the analytics service and auth info needed for event recording.
/// Created from `AnalyticsServiceCtx` via `into_context()`.
#[derive(Clone)]
pub struct AnalyticsContext {
    analytics: Option<Arc<analytics::AnalyticsService>>,
    actor_id: Option<Uuid>,
    request_id: Option<String>,
}

impl AnalyticsContext {
    pub fn new(
        analytics: Option<Arc<analytics::AnalyticsService>>,
        actor_id: Option<Uuid>,
        request_id: Option<String>,
    ) -> Self {
        Self {
            analytics,
            actor_id,
            request_id,
        }
    }

    pub fn is_available(&self) -> bool {
        self.analytics.is_some()
    }

    pub async fn record_product_event(
        &self,
        event_name: analytics::ProductEventName,
        surface: analytics::Surface,
        result: analytics::ResultTag,
        session_id: Option<Uuid>,
        notebook_id: Option<Uuid>,
        metadata: serde_json::Value,
    ) {
        let Some(ref analytics) = self.analytics else {
            return;
        };
        let Some(user_id) = self.actor_id else {
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
            request_id: self.request_id.clone(),
            trace_id: None,
            client_platform: "web".to_string(),
            metadata,
        };
        if let Err(error) = analytics.record_product_event(&event).await {
            telemetry::prometheus::record_dependency_failure("analytics");
            tracing::warn!(error = %error, event_name = ?event_name, "failed to record product event");
        }
    }

    pub async fn record_cost_event(&self, record: CostEventRecord<'_>) {
        let Some(ref analytics) = self.analytics else {
            return;
        };
        let Some(user_id) = self.actor_id else {
            return;
        };

        let event = analytics::CostEvent {
            event_id: Uuid::new_v4(),
            event_time: chrono::Utc::now(),
            user_id,
            session_id: record.session_id,
            notebook_id: record.notebook_id,
            event_name: record.event_name,
            feature: record.feature.to_string(),
            provider: crate::util::non_empty_or_unknown(&record.usage.provider),
            model: crate::util::non_empty_or_unknown(&record.usage.model),
            prompt_tokens: i64::from(record.usage.prompt_tokens),
            completion_tokens: i64::from(record.usage.completion_tokens),
            embedding_tokens: 0,
            usage_units: super::compute_usage_units(
                &record.usage.provider,
                &record.usage.model,
                record.usage.prompt_tokens,
                record.usage.completion_tokens,
            ),
            storage_bytes_delta: 0,
            external_call_count: 0,
            source: record.source.to_string(),
            metadata: record.metadata,
        };
        if let Err(error) = analytics.record_cost_event(&event).await {
            telemetry::prometheus::record_dependency_failure("analytics");
            tracing::warn!(error = %error, event_name = ?record.event_name, "failed to record cost event");
        }
    }

    pub async fn record_storage_cost_event(
        &self,
        event_name: analytics::CostEventName,
        feature: &str,
        notebook_id: Option<Uuid>,
        storage_bytes_delta: i64,
        source: &str,
        metadata: serde_json::Value,
    ) {
        let Some(ref analytics) = self.analytics else {
            return;
        };
        let Some(user_id) = self.actor_id else {
            return;
        };

        let event = analytics::CostEvent {
            event_id: Uuid::new_v4(),
            event_time: chrono::Utc::now(),
            user_id,
            session_id: None,
            notebook_id,
            event_name,
            feature: feature.to_string(),
            provider: "internal".to_string(),
            model: "storage".to_string(),
            prompt_tokens: 0,
            completion_tokens: 0,
            embedding_tokens: 0,
            usage_units: 0,
            storage_bytes_delta,
            external_call_count: 0,
            source: source.to_string(),
            metadata,
        };
        if let Err(error) = analytics.record_cost_event(&event).await {
            telemetry::prometheus::record_dependency_failure("analytics");
            tracing::warn!(error = %error, event_name = ?event_name, "failed to record storage cost event");
        }
    }

    pub async fn record_external_search_cost_event(
        &self,
        provider: &str,
        model: &str,
        notebook_id: Option<Uuid>,
        external_call_count: i64,
        metadata: serde_json::Value,
    ) {
        let Some(ref analytics) = self.analytics else {
            return;
        };
        let Some(user_id) = self.actor_id else {
            return;
        };

        let event = analytics::CostEvent {
            event_id: Uuid::new_v4(),
            event_time: chrono::Utc::now(),
            user_id,
            session_id: None,
            notebook_id,
            event_name: analytics::CostEventName::ExternalSearchUsageMetered,
            feature: "search".to_string(),
            provider: crate::util::non_empty_or_unknown(provider),
            model: crate::util::non_empty_or_unknown(model),
            prompt_tokens: 0,
            completion_tokens: 0,
            embedding_tokens: 0,
            usage_units: 0,
            storage_bytes_delta: 0,
            external_call_count,
            source: "external_search".to_string(),
            metadata,
        };
        if let Err(error) = analytics.record_cost_event(&event).await {
            telemetry::prometheus::record_dependency_failure("analytics");
            tracing::warn!(error = %error, "failed to record external search cost event");
        }
    }
}

/// Mirrors `CostEventRecord` from state_methods for use with AnalyticsContext.
pub struct CostEventRecord<'a> {
    pub event_name: analytics::CostEventName,
    pub feature: &'a str,
    pub session_id: Option<Uuid>,
    pub notebook_id: Option<Uuid>,
    pub usage: &'a avrag_llm::LlmUsage,
    pub source: &'a str,
    pub metadata: serde_json::Value,
}
