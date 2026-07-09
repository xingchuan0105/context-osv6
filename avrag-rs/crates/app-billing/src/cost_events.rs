use std::sync::Arc;

use analytics::AnalyticsService;
use app_core::util::non_empty_or_unknown;
use contracts::auth_runtime::AuthContext;
use avrag_llm::LlmUsage;
use uuid::Uuid;

pub struct CostEventRecord<'a> {
    pub event_name: analytics::CostEventName,
    pub feature: &'a str,
    pub session_id: Option<Uuid>,
    pub workspace_id: Option<Uuid>,
    pub usage: &'a LlmUsage,
    pub source: &'a str,
    pub metadata: serde_json::Value,
}

pub async fn record_cost_event_if_available(
    auth: &AuthContext,
    analytics: &Option<Arc<AnalyticsService>>,
    record: CostEventRecord<'_>,
) {
    let Some(analytics) = analytics.as_ref() else {
        return;
    };
    let Some(user_id) = auth.actor_id().map(|actor| actor.into_uuid()) else {
        return;
    };

    let event = analytics::CostEvent {
        event_id: Uuid::new_v4(),
        event_time: chrono::Utc::now(),
        user_id,
        session_id: record.session_id,
        workspace_id: record.workspace_id,
        event_name: record.event_name,
        feature: record.feature.to_string(),
        provider: non_empty_or_unknown(&record.usage.provider),
        model: non_empty_or_unknown(&record.usage.model),
        prompt_tokens: i64::from(record.usage.prompt_tokens),
        completion_tokens: i64::from(record.usage.completion_tokens),
        embedding_tokens: 0,
        usage_units: avrag_billing::usage_limit::compute_usage_units(
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

pub async fn record_storage_cost_event_if_available(
    auth: &AuthContext,
    analytics: &Option<Arc<AnalyticsService>>,
    event_name: analytics::CostEventName,
    feature: &str,
    workspace_id: Option<Uuid>,
    storage_bytes_delta: i64,
    source: &str,
    metadata: serde_json::Value,
) {
    let Some(analytics) = analytics.as_ref() else {
        return;
    };
    let Some(user_id) = auth.actor_id().map(|actor| actor.into_uuid()) else {
        return;
    };

    let event = analytics::CostEvent {
        event_id: Uuid::new_v4(),
        event_time: chrono::Utc::now(),
        user_id,
        session_id: None,
        workspace_id,
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

pub async fn record_external_search_cost_event_if_available(
    auth: &AuthContext,
    analytics: &Option<Arc<AnalyticsService>>,
    provider: &str,
    model: &str,
    workspace_id: Option<Uuid>,
    external_call_count: i64,
    metadata: serde_json::Value,
) {
    let Some(analytics) = analytics.as_ref() else {
        return;
    };
    let Some(user_id) = auth.actor_id().map(|actor| actor.into_uuid()) else {
        return;
    };

    let event = analytics::CostEvent {
        event_id: Uuid::new_v4(),
        event_time: chrono::Utc::now(),
        user_id,
        session_id: None,
        workspace_id,
        event_name: analytics::CostEventName::ExternalSearchUsageMetered,
        feature: "search".to_string(),
        provider: non_empty_or_unknown(provider),
        model: non_empty_or_unknown(model),
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
