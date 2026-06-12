use app_bootstrap::AppState;
use app_core::{
    AdminAuditLogPage, AdminAuditLogQuery, AdminBillingOverview, AdminDegradationStatus,
    AdminFeatureFlagChangeRequest, AdminFeatureFlagEntry, AdminOrgInfo, AdminRagHealthStatus,
    AdminStorePort, AdminUsageStats, AdminUserInfo, AdminWorkerStatus,
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

fn audit_query(query: AuditQuery) -> AdminAuditLogQuery {
    AdminAuditLogQuery {
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
    call_admin_store::<Vec<AdminOrgInfo>, _, _>(&state, |store| {
        let auth = state.auth().clone();
        async move { store.list_orgs(&auth).await }
    })
    .await
}

async fn get_org(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(org_id): Path<String>,
) -> Response {
    let org_id = match parse_org_id(&org_id) {
        Ok(org_id) => org_id,
        Err(response) => return response,
    };
    call_admin_store::<AdminOrgInfo, _, _>(&state, |store| {
        let auth = state.auth().clone();
        async move { store.get_org(&auth, org_id).await }
    })
    .await
}

async fn list_users(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(query): Query<OrgQuery>,
) -> Response {
    let org_id = match parse_org_id(&query.org_id) {
        Ok(org_id) => org_id,
        Err(response) => return response,
    };
    call_admin_store::<Vec<AdminUserInfo>, _, _>(&state, |store| {
        let auth = state.auth().clone();
        async move { store.list_users(&auth, org_id).await }
    })
    .await
}

async fn delete_user(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(user_id): Path<String>,
) -> Response {
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
    call_admin_store::<(), _, _>(&state, |store| {
        let auth = state.auth().clone();
        async move { store.delete_user(&auth, org_id, user_id).await }
    })
    .await
}

async fn get_usage(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(query): Query<UsageQuery>,
) -> Response {
    let org_id = match parse_org_id(&query.org_id) {
        Ok(org_id) => org_id,
        Err(response) => return response,
    };
    let period = query.period.unwrap_or_else(|| "30d".to_string());
    call_admin_store::<AdminUsageStats, _, _>(&state, |store| {
        let auth = state.auth().clone();
        async move { store.get_usage(&auth, org_id, &period).await }
    })
    .await
}

async fn block_org(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(body): Json<BlockOrgRequest>,
) -> Response {
    let org_id = match parse_org_id(&body.org_id) {
        Ok(org_id) => org_id,
        Err(response) => return response,
    };
    call_admin_store::<(), _, _>(&state, |store| {
        let auth = state.auth().clone();
        async move { store.set_org_blocked(&auth, org_id, body.blocked).await }
    })
    .await
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
    if state.auth().actor_id().is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<AdminAuditLogPage>::err(
                "authenticated_user_required",
                "admin action requires an authenticated user",
            )),
        )
            .into_response();
    }
    let store = admin_store_or_response!(state);
    let format = query.format.clone();
    let audit_query = audit_query(query);
    let auth = state.auth().clone();
    if format.as_deref() == Some("csv") {
        match store.export_audit_logs_csv(&auth, &audit_query).await {
            Ok(csv) => {
                return (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
                    csv,
                )
                    .into_response();
            }
            Err(error) => return app_error_response::<AdminAuditLogPage>(error),
        }
    }
    match store.list_audit_logs(&auth, &audit_query).await {
        Ok(page) => Json(ApiResponse::ok(page)).into_response(),
        Err(error) => app_error_response::<AdminAuditLogPage>(error),
    }
}
