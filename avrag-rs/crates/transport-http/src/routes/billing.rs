use app::AppState;
use axum::{Extension, Json, Router, routing::get};
use common::{ApiResponse, UserId};

use crate::RequestState;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/billing/plans", get(get_plans))
        .route("/billing/subscription", get(get_subscription))
        .route("/billing/usage", get(get_usage))
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
    Json(avrag_billing::handle_get_plans(repo, state.auth().org_id()).await)
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
    Json(avrag_billing::handle_get_subscription(repo, state.auth().org_id()).await)
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
    Json(avrag_billing::handle_get_usage(repo, state.auth().org_id()).await)
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
        avrag_billing::handle_create_checkout(
            repo,
            state.auth().org_id(),
            UserId::from(actor_id.into_uuid()),
            body,
        )
        .await,
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
    Json(avrag_billing::handle_create_portal(repo, state.auth().org_id()).await)
}
