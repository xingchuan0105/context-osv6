use app_core::AnalyticsServiceCtx;
use app_billing;
use avrag_auth::AuthContext;
use uuid::Uuid;

pub(crate) async fn record_product_event_if_available(
    auth: &AuthContext,
    analytics: &AnalyticsServiceCtx,
    event_name: analytics::ProductEventName,
    surface: analytics::Surface,
    result: analytics::ResultTag,
    session_id: Option<Uuid>,
    notebook_id: Option<Uuid>,
    metadata: serde_json::Value,
) {
    let ctx = analytics.into_context(
        auth.actor_id().map(|actor| actor.into_uuid()),
        auth.request_id().map(str::to_string),
    );
    ctx.record_product_event(
        event_name,
        surface,
        result,
        session_id,
        notebook_id,
        metadata,
    )
    .await;
}

pub(crate) async fn record_storage_cost_event_if_available(
    auth: &AuthContext,
    analytics: &AnalyticsServiceCtx,
    event_name: analytics::CostEventName,
    feature: &str,
    notebook_id: Option<Uuid>,
    storage_bytes_delta: i64,
    source: &str,
    metadata: serde_json::Value,
) {
    app_billing::record_storage_cost_event_if_available(
        auth,
        &analytics.service().cloned(),
        event_name,
        feature,
        notebook_id,
        storage_bytes_delta,
        source,
        metadata,
    )
    .await;
}
