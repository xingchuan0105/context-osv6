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

macro_rules! repo_or_response {
    ($state:expr) => {
        match $state.pg() {
            Some(repo) => repo,
            None => {
                return Json(ApiResponse::err(
                    "postgres_not_configured",
                    "postgres backend is not configured",
                ));
            }
        }
    };
}

async fn get_plans(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Json<ApiResponse<serde_json::Value>> {
    let repo = repo_or_response!(state);
    let Some(actor_id) = state.auth().actor_id() else {
        return Json(ApiResponse::err(
            "authenticated_user_required",
            "authenticated user required",
        ));
    };
    Json(avrag_billing::handle_get_plans(repo, UserId::from(actor_id.into_uuid())).await)
}

async fn get_subscription(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Json<ApiResponse<avrag_billing::SubscriptionResponse>> {
    let Some(repo) = state.pg() else {
        return Json(ApiResponse::err(
            "postgres_not_configured",
            "postgres backend is not configured",
        ));
    };
    let Some(actor_id) = state.auth().actor_id() else {
        return Json(ApiResponse::err(
            "authenticated_user_required",
            "authenticated user required",
        ));
    };
    Json(avrag_billing::handle_get_subscription(repo, UserId::from(actor_id.into_uuid())).await)
}

async fn get_usage(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Json<ApiResponse<avrag_billing::UsageResponse>> {
    let Some(repo) = state.pg() else {
        return Json(ApiResponse::err(
            "postgres_not_configured",
            "postgres backend is not configured",
        ));
    };
    let Some(actor_id) = state.auth().actor_id() else {
        return Json(ApiResponse::err(
            "authenticated_user_required",
            "authenticated user required",
        ));
    };
    Json(avrag_billing::handle_get_usage(repo, UserId::from(actor_id.into_uuid())).await)
}

async fn get_usage_window(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Json<ApiResponse<avrag_billing::UsageWindowResponse>> {
    let Some(repo) = state.pg() else {
        return Json(ApiResponse::err(
            "postgres_not_configured",
            "postgres backend is not configured",
        ));
    };
    let Some(actor_id) = state.auth().actor_id() else {
        return Json(ApiResponse::err(
            "authenticated_user_required",
            "authenticated user required",
        ));
    };
    Json(avrag_billing::handle_get_usage_window(repo, UserId::from(actor_id.into_uuid())).await)
}

async fn create_checkout(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(body): Json<avrag_billing::CreateCheckoutRequest>,
) -> Json<ApiResponse<avrag_billing::CheckoutResponse>> {
    let Some(repo) = state.pg() else {
        return Json(ApiResponse::err(
            "postgres_not_configured",
            "postgres backend is not configured",
        ));
    };
    let Some(actor_id) = state.auth().actor_id() else {
        return Json(ApiResponse::err(
            "authenticated_user_required",
            "billing checkout requires an authenticated user",
        ));
    };
    Json(
        avrag_billing::handle_create_checkout(repo, UserId::from(actor_id.into_uuid()), body).await,
    )
}

async fn create_portal(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Json<ApiResponse<avrag_billing::PortalResponse>> {
    let Some(repo) = state.pg() else {
        return Json(ApiResponse::err(
            "postgres_not_configured",
            "postgres backend is not configured",
        ));
    };
    let Some(actor_id) = state.auth().actor_id() else {
        return Json(ApiResponse::err(
            "authenticated_user_required",
            "authenticated user required",
        ));
    };
    Json(avrag_billing::handle_create_portal(repo, UserId::from(actor_id.into_uuid())).await)
}

#[derive(Deserialize)]
struct HistoryParams {
    days: Option<i32>,
}

async fn get_usage_history(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(params): Query<HistoryParams>,
) -> Json<ApiResponse<avrag_billing::UsageHistoryResponse>> {
    let Some(repo) = state.pg() else {
        return Json(ApiResponse::err(
            "postgres_not_configured",
            "postgres backend is not configured",
        ));
    };
    let Some(actor_id) = state.auth().actor_id() else {
        return Json(ApiResponse::err(
            "authenticated_user_required",
            "authenticated user required",
        ));
    };
    Json(
        avrag_billing::handle_get_usage_history(
            repo,
            UserId::from(actor_id.into_uuid()),
            params.days.unwrap_or(7),
        )
        .await,
    )
}

async fn get_usage_forecast(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Json<ApiResponse<avrag_billing::UsageForecastResponse>> {
    let Some(repo) = state.pg() else {
        return Json(ApiResponse::err(
            "postgres_not_configured",
            "postgres backend is not configured",
        ));
    };
    let Some(actor_id) = state.auth().actor_id() else {
        return Json(ApiResponse::err(
            "authenticated_user_required",
            "authenticated user required",
        ));
    };
    Json(avrag_billing::handle_get_usage_forecast(repo, UserId::from(actor_id.into_uuid())).await)
}
