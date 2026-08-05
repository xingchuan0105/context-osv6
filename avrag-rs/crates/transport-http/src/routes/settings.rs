//! Account settings routes (ADR-0010 PR7 cloud BYOK secrets).

use app_bootstrap::AppState;
use axum::{
    Extension, Json, Router,
    extract::{Path, Query},
    routing::put,
};
use common::ApiResponse;
use serde::Deserialize;
use uuid::Uuid;

use crate::middleware::RequestState;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        // Cloud BYOK: create/update (PUT idempotent upsert by scope).
        .route(
            "/settings/provider-secrets",
            put(upsert_provider_secret)
                .post(upsert_provider_secret)
                .get(list_provider_secrets),
        )
        .route(
            "/settings/provider-secrets/{id}",
            axum::routing::delete(revoke_provider_secret),
        )
}

#[derive(Deserialize)]
struct ListParams {
    /// When true, include soft-revoked rows (still fingerprint-only).
    include_revoked: Option<bool>,
}

async fn upsert_provider_secret(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(body): Json<avrag_billing::UpsertProviderSecretRequest>,
) -> Json<ApiResponse<avrag_billing::ProviderSecretResponse>> {
    Json(state.billing_api().upsert_provider_secret(body).await)
}

async fn list_provider_secrets(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(params): Query<ListParams>,
) -> Json<ApiResponse<avrag_billing::ProviderSecretListResponse>> {
    Json(
        state
            .billing_api()
            .list_provider_secrets(params.include_revoked.unwrap_or(false))
            .await,
    )
}

async fn revoke_provider_secret(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(id): Path<Uuid>,
) -> Json<ApiResponse<avrag_billing::ProviderSecretResponse>> {
    Json(state.billing_api().revoke_provider_secret(id).await)
}
