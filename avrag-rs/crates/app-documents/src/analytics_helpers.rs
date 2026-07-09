//! Document-crate helpers that **only** forward to the canonical analytics seam.
//!
//! Do not add new product-event entry points. The single source of truth is
//! [`AnalyticsServiceCtx::record_product_event_for_auth`] /
//! [`AnalyticsContext::record_product_event`].

use app_billing;
use app_core::AnalyticsServiceCtx;
use contracts::auth_runtime::AuthContext;
use uuid::Uuid;

/// Thin alias over the canonical analytics seam (`AnalyticsServiceCtx`).
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
    analytics
        .record_product_event_for_auth(
            auth,
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
