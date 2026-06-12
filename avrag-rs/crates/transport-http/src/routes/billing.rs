use app::AppState;
use axum::{Extension, Json, Router, extract::Query, routing::get};
use common::{ApiResponse, UserId};
use serde::Deserialize;

use crate::RequestState;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/billing/plans", get(get_plans))
        .route("/billing/subscription", get(get_subscription))
        .route("/billing/usage", get(get_usage))
        .route("/billing/usage/window", get(get_usage_window))
        .route("/billing/usage/history", get(get_usage_history))
        .route("/billing/usage/forecast", get(get_usage_forecast))
        .route(
            "/billing/checkout-session",
            axum::routing::post(create_checkout),
        )
        .route(
            "/billing/portal-session",
            axum::routing::post(create_portal),
        )
}

async fn get_plans(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Json<ApiResponse<serde_json::Value>> {
    Json(state.billing_get_plans().await)
}

async fn get_subscription(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Json<ApiResponse<avrag_billing::SubscriptionResponse>> {
    Json(state.billing_get_subscription().await)
}

async fn get_usage(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Json<ApiResponse<avrag_billing::UsageResponse>> {
    Json(state.billing_get_usage().await)
}

async fn get_usage_window(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Json<ApiResponse<avrag_billing::UsageWindowResponse>> {
    Json(state.billing_get_usage_window().await)
}

async fn create_checkout(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(body): Json<avrag_billing::CreateCheckoutRequest>,
) -> Json<ApiResponse<avrag_billing::CheckoutResponse>> {
    Json(state.billing_create_checkout(body).await)
}

async fn create_portal(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Json<ApiResponse<avrag_billing::PortalResponse>> {
    Json(state.billing_create_portal().await)
}

#[derive(Deserialize)]
struct HistoryParams {
    days: Option<i32>,
}

async fn get_usage_history(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(params): Query<HistoryParams>,
) -> Json<ApiResponse<avrag_billing::UsageHistoryResponse>> {
    Json(state.billing_get_usage_history(params.days.unwrap_or(7)).await)
}

async fn get_usage_forecast(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Json<ApiResponse<avrag_billing::UsageForecastResponse>> {
    Json(state.billing_get_usage_forecast().await)
}
