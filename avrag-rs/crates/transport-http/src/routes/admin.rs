use app_bootstrap::AppState;
use app_core::{
    AdminBillingOverview, AdminDegradationStatus, AdminFeatureFlagChangeRequest,
    AdminFeatureFlagEntry, AdminRagHealthStatus, AdminStorePort, AdminWorkerStatus,
};
use axum::{
    Extension, Json, Router,
    extract::{Path, Query},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use common::{ApiResponse, AppError};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::RequestState;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/organizations", get(list_orgs))
        .route("/admin/organizations/{org_id}", get(get_org))
        .route("/admin/users", get(list_users))
        .route("/admin/users/{user_id}", axum::routing::delete(delete_user))
        .route("/admin/usage", get(get_usage))
        .route("/admin/billing/block", axum::routing::post(block_org))
        .route("/admin/health", get(health))
        .route("/admin/billing", get(billing_overview))
        .route("/admin/rag-health", get(rag_health))
        .route("/admin/system/workers", get(worker_status))
        .route("/admin/system/degradation", get(degradation_status))
        .route("/admin/feature-flags", get(feature_flags))
        .route(
            "/admin/feature-flags/change-requests",
            get(feature_flag_change_requests),
        )
        .route(
            "/admin/feature-flags/{key}/change-requests",
            axum::routing::post(create_feature_flag_change_request),
        )
        .route(
            "/admin/feature-flags/change-requests/{request_id}/review",
            axum::routing::post(review_feature_flag_change_request),
        )
        .route("/admin/audit-logs", get(audit_logs))
}

#[derive(Deserialize)]
struct OrgQuery {
    org_id: String,
}

#[derive(Deserialize)]
struct UsageQuery {
    org_id: String,
    period: Option<String>,
}

#[derive(Deserialize)]
struct BlockOrgRequest {
    org_id: String,
    blocked: bool,
}

#[derive(Deserialize)]
struct AuditQuery {
    query: Option<String>,
    action: Option<String>,
    resource_type: Option<String>,
    actor: Option<String>,
    window: Option<String>,
    page: Option<usize>,
    per_page: Option<usize>,
    format: Option<String>,
}

#[derive(Deserialize)]
struct FeatureFlagChangeRequestQuery {
    status: Option<String>,
}

#[derive(Deserialize)]
struct CreateFeatureFlagChangeRequest {
    enabled: bool,
    reason: String,
}

#[derive(Deserialize)]
struct ReviewFeatureFlagChangeRequest {
    approved: bool,
    review_note: Option<String>,
}

macro_rules! repo_or_response {
    ($state:expr) => {
        match $state.postgres_repo() {
            Some(repo) => repo,
            None => {
                return app_error_response::<serde_json::Value>(AppError::validation(
                    "postgres_not_configured",
                    "postgres backend is not configured",
                ));
            }
        }
    };
}

macro_rules! admin_store_or_response {
    ($state:expr) => {
        match $state.admin_store() {
            Some(store) => store,
            None => {
                return app_error_response::<serde_json::Value>(AppError::validation(
                    "postgres_not_configured",
                    "postgres backend is not configured",
                ));
            }
        }
    };
}

fn app_error_response<T: serde::Serialize>(error: AppError) -> Response {
    let (status, code, message) = match &error {
        AppError::Validation {
            code,
            message,
            http_status,
            ..
        }
        | AppError::NotFound {
            code,
            message,
            http_status,
            ..
        }
        | AppError::Conflict {
            code,
            message,
            http_status,
            ..
        }
        | AppError::Internal {
            code,
            message,
            http_status,
            ..
        }
        | AppError::RateLimited {
            code,
            message,
            http_status,
            ..
        } => (
            StatusCode::from_u16(*http_status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            *code,
            message.clone(),
        ),
    };
    (
        status,
        Json(ApiResponse::<T>::err(code, &message)),
    )
        .into_response()
}

async fn call_admin_store<T, F, Fut>(state: &AppState, f: F) -> Response
where
    T: serde::Serialize,
    F: FnOnce(Arc<dyn AdminStorePort>) -> Fut,
    Fut: std::future::Future<Output = Result<T, AppError>>,
{
    if state.auth().actor_id().is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<T>::err(
                "authenticated_user_required",
                "admin action requires an authenticated user",
            )),
        )
            .into_response();
    }
    let store = admin_store_or_response!(state);
    match f(store).await {
        Ok(value) => Json(ApiResponse::ok(value)).into_response(),
        Err(error) => app_error_response::<T>(error),
    }
}

#[allow(clippy::result_large_err)]
fn parse_org_id(value: &str) -> Result<common::OrgId, Response> {
    value.parse::<common::OrgId>().map_err(|_| {
        Json(ApiResponse::<serde_json::Value>::err(
            "invalid_org_id",
            "org_id must be a UUID",
        ))
        .into_response()
    })
}

async fn ensure_admin_access(state: &AppState) -> Result<(), Response> {
    if state.auth().actor_id().is_none() {
        return Err(
            Json(ApiResponse::<serde_json::Value>::err(
                "authenticated_user_required",
                "admin action requires an authenticated user",
            ))
            .into_response(),
        );
    }
    let Some(store) = state.admin_store() else {
        return Err(
            Json(ApiResponse::<serde_json::Value>::err(
                "postgres_not_configured",
                "postgres backend is not configured",
            ))
            .into_response(),
        );
    };
    store
        .ensure_admin_access(state.auth())
        .await
        .map_err(app_error_response::<serde_json::Value>)
}

fn audit_query(query: AuditQuery) -> avrag_admin::AuditLogQuery {
    avrag_admin::AuditLogQuery {
        query: query.query,
        action: query.action,
        resource_type: query.resource_type,
        actor: query.actor,
        window: query.window,
        page: query.page.unwrap_or(1),
        per_page: query.per_page.unwrap_or(50),
    }
}

async fn list_orgs(Extension(RequestState(state)): Extension<RequestState>) -> Response {
    if let Err(response) = ensure_admin_access(&state).await {
        return response;
    }
    let repo = repo_or_response!(state);
    Json(avrag_admin::handle_list_orgs(state.auth().clone(), repo).await).into_response()
}

async fn get_org(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(org_id): Path<String>,
) -> Response {
    if let Err(response) = ensure_admin_access(&state).await {
        return response;
    }
    let repo = repo_or_response!(state);
    let org_id = match parse_org_id(&org_id) {
        Ok(org_id) => org_id,
        Err(response) => return response,
    };
    Json(avrag_admin::handle_get_org(state.auth().clone(), org_id, repo).await).into_response()
}

async fn list_users(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(query): Query<OrgQuery>,
) -> Response {
    if let Err(response) = ensure_admin_access(&state).await {
        return response;
    }
    let repo = repo_or_response!(state);
    let org_id = match parse_org_id(&query.org_id) {
        Ok(org_id) => org_id,
        Err(response) => return response,
    };
    Json(avrag_admin::handle_list_users(state.auth().clone(), org_id, repo).await).into_response()
}

async fn delete_user(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(user_id): Path<String>,
) -> Response {
    if let Err(response) = ensure_admin_access(&state).await {
        return response;
    }
    let repo = repo_or_response!(state);
    let user_id = match user_id.parse::<Uuid>() {
        Ok(id) => id,
        Err(_) => {
            return Json(ApiResponse::<serde_json::Value>::err(
                "invalid_user_id",
                "user_id must be a UUID",
            ))
            .into_response();
        }
    };
    let org_id = state.auth().org_id();
    Json(avrag_admin::handle_delete_user(state.auth().clone(), org_id, user_id, repo).await)
        .into_response()
}

async fn get_usage(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(query): Query<UsageQuery>,
) -> Response {
    if let Err(response) = ensure_admin_access(&state).await {
        return response;
    }
    let repo = repo_or_response!(state);
    let org_id = match parse_org_id(&query.org_id) {
        Ok(org_id) => org_id,
        Err(response) => return response,
    };
    Json(
        avrag_admin::handle_get_usage(
            state.auth().clone(),
            org_id,
            query.period.unwrap_or_else(|| "30d".to_string()),
            repo,
        )
        .await,
    )
    .into_response()
}

async fn block_org(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(body): Json<BlockOrgRequest>,
) -> Response {
    if let Err(response) = ensure_admin_access(&state).await {
        return response;
    }
    let repo = repo_or_response!(state);
    let org_id = match parse_org_id(&body.org_id) {
        Ok(org_id) => org_id,
        Err(response) => return response,
    };
    Json(avrag_admin::handle_block_org(state.auth().clone(), org_id, body.blocked, repo).await)
        .into_response()
}

async fn health() -> Response {
    Json(avrag_admin::handle_health().await).into_response()
}

async fn billing_overview(Extension(RequestState(state)): Extension<RequestState>) -> Response {
    call_admin_store::<AdminBillingOverview, _, _>(&state, |store| {
        let auth = state.auth().clone();
        async move { store.billing_overview(&auth).await }
    })
    .await
}

async fn rag_health(Extension(RequestState(state)): Extension<RequestState>) -> Response {
    call_admin_store::<AdminRagHealthStatus, _, _>(&state, |store| {
        let auth = state.auth().clone();
        async move { store.rag_health(&auth).await }
    })
    .await
}

async fn worker_status(Extension(RequestState(state)): Extension<RequestState>) -> Response {
    call_admin_store::<AdminWorkerStatus, _, _>(&state, |store| {
        let auth = state.auth().clone();
        async move { store.worker_status(&auth).await }
    })
    .await
}

async fn degradation_status(Extension(RequestState(state)): Extension<RequestState>) -> Response {
    call_admin_store::<AdminDegradationStatus, _, _>(&state, |store| {
        let auth = state.auth().clone();
        async move { store.degradation_status(&auth).await }
    })
    .await
}

async fn feature_flags(Extension(RequestState(state)): Extension<RequestState>) -> Response {
    call_admin_store::<Vec<AdminFeatureFlagEntry>, _, _>(&state, |store| {
        let auth = state.auth().clone();
        async move { store.list_feature_flags(&auth).await }
    })
    .await
}

async fn feature_flag_change_requests(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(query): Query<FeatureFlagChangeRequestQuery>,
) -> Response {
    let status = query.status;
    call_admin_store::<Vec<AdminFeatureFlagChangeRequest>, _, _>(&state, |store| {
        let auth = state.auth().clone();
        async move {
            store
                .list_feature_flag_change_requests(&auth, status.as_deref())
                .await
        }
    })
    .await
}

async fn create_feature_flag_change_request(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(key): Path<String>,
    Json(body): Json<CreateFeatureFlagChangeRequest>,
) -> Response {
    call_admin_store::<AdminFeatureFlagChangeRequest, _, _>(&state, |store| {
        let auth = state.auth().clone();
        async move {
            store
                .create_feature_flag_change_request(&auth, &key, body.enabled, &body.reason)
                .await
        }
    })
    .await
}

async fn review_feature_flag_change_request(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(request_id): Path<String>,
    Json(body): Json<ReviewFeatureFlagChangeRequest>,
) -> Response {
    call_admin_store::<AdminFeatureFlagChangeRequest, _, _>(&state, |store| {
        let auth = state.auth().clone();
        async move {
            store
                .review_feature_flag_change_request(
                    &auth,
                    &request_id,
                    body.approved,
                    body.review_note.as_deref(),
                )
                .await
        }
    })
    .await
}

async fn audit_logs(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(query): Query<AuditQuery>,
) -> Response {
    if let Err(response) = ensure_admin_access(&state).await {
        return response;
    }
    let repo = repo_or_response!(state);
    let format = query.format.clone();
    let query = audit_query(query);
    if format.as_deref() == Some("csv") {
        let response =
            avrag_admin::handle_export_audit_logs_csv(state.auth().clone(), query, repo).await;
        if let Some(csv) = response.data {
            return (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
                csv,
            )
                .into_response();
        }
        return Json(response).into_response();
    }
    Json(avrag_admin::handle_list_audit_logs(state.auth().clone(), query, repo).await)
        .into_response()
}
