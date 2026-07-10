use app_bootstrap::AppState;
use app_core::{AdminAuditLogPage, AdminAuditLogQuery};
use axum::{
    Extension, Json, Router,
    extract::{Path, Query},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use common::{ApiResponse, AppError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::middleware::RequestState;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/accounts", get(list_accounts))
        .route("/admin/accounts/{owner_user_id}", get(get_account))
        .route("/admin/users", get(list_users))
        .route("/admin/users/{user_id}", axum::routing::delete(delete_user))
        .route("/admin/usage", get(get_usage))
        .route("/admin/billing/block", axum::routing::post(block_account))
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
struct ListAccountsQuery {
    page: Option<usize>,
    per_page: Option<usize>,
}

#[derive(Deserialize)]
struct AccountQuery {
    owner_user_id: String,
}

#[derive(Deserialize)]
struct UsageQuery {
    owner_user_id: String,
    period: Option<String>,
}

#[derive(Deserialize)]
struct BlockAccountRequest {
    owner_user_id: String,
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
    (status, Json(ApiResponse::<T>::err(code, &message))).into_response()
}

async fn ops_json<T: serde::Serialize>(
    result: Result<T, AppError>,
) -> Response {
    match result {
        Ok(value) => Json(ApiResponse::ok(value)).into_response(),
        Err(error) => app_error_response::<T>(error),
    }
}

#[allow(clippy::result_large_err)]
fn parse_owner_user_id(value: &str) -> Result<common::UserId, Response> {
    value.parse::<common::UserId>().map_err(|_| {
        Json(ApiResponse::<serde_json::Value>::err(
            "invalid_owner_user_id",
            "owner_user_id must be a UUID",
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

async fn list_accounts(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(query): Query<ListAccountsQuery>,
) -> Response {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = app_core::admin_clamp_account_list_per_page(query.per_page.unwrap_or(100));
    ops_json(state.admin_ops().list_accounts(page, per_page).await).await
}

async fn get_account(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(owner_user_id): Path<String>,
) -> Response {
    let owner_user_id = match parse_owner_user_id(&owner_user_id) {
        Ok(owner_user_id) => owner_user_id,
        Err(response) => return response,
    };
    ops_json(state.admin_ops().get_account(owner_user_id).await).await
}

async fn list_users(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(query): Query<AccountQuery>,
) -> Response {
    let owner_user_id = match parse_owner_user_id(&query.owner_user_id) {
        Ok(owner_user_id) => owner_user_id,
        Err(response) => return response,
    };
    ops_json(state.admin_ops().list_users(owner_user_id).await).await
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
    let owner_user_id = state.auth().user_id();
    ops_json(state.admin_ops().delete_user(owner_user_id, user_id).await).await
}

async fn get_usage(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(query): Query<UsageQuery>,
) -> Response {
    let owner_user_id = match parse_owner_user_id(&query.owner_user_id) {
        Ok(owner_user_id) => owner_user_id,
        Err(response) => return response,
    };
    let period = query.period.unwrap_or_else(|| "30d".to_string());
    ops_json(state.admin_ops().get_usage(owner_user_id, &period).await).await
}

async fn block_account(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(body): Json<BlockAccountRequest>,
) -> Response {
    let owner_user_id = match parse_owner_user_id(&body.owner_user_id) {
        Ok(owner_user_id) => owner_user_id,
        Err(response) => return response,
    };
    ops_json(state.admin_ops().set_account_blocked(owner_user_id, body.blocked).await).await
}

#[derive(Serialize)]
struct AdminHealthStatus {
    status: String,
    version: String,
    uptime_secs: i64,
}

async fn health() -> Response {
    Json(ApiResponse::ok(AdminHealthStatus {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_secs: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0),
    }))
    .into_response()
}

async fn billing_overview(Extension(RequestState(state)): Extension<RequestState>) -> Response {
    ops_json(state.admin_ops().billing_overview().await).await
}

async fn rag_health(Extension(RequestState(state)): Extension<RequestState>) -> Response {
    ops_json(state.admin_ops().rag_health().await).await
}

async fn worker_status(Extension(RequestState(state)): Extension<RequestState>) -> Response {
    ops_json(state.admin_ops().worker_status().await).await
}

async fn degradation_status(Extension(RequestState(state)): Extension<RequestState>) -> Response {
    ops_json(state.admin_ops().degradation_status().await).await
}

async fn feature_flags(Extension(RequestState(state)): Extension<RequestState>) -> Response {
    ops_json(state.admin_ops().list_feature_flags().await).await
}

async fn feature_flag_change_requests(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(query): Query<FeatureFlagChangeRequestQuery>,
) -> Response {
    ops_json(
        state
            .admin_ops()
            .list_feature_flag_change_requests(query.status.as_deref())
            .await,
    )
    .await
}

async fn create_feature_flag_change_request(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(key): Path<String>,
    Json(body): Json<CreateFeatureFlagChangeRequest>,
) -> Response {
    ops_json(
        state
            .admin_ops()
            .create_feature_flag_change_request(&key, body.enabled, &body.reason)
            .await,
    )
    .await
}

async fn review_feature_flag_change_request(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(request_id): Path<String>,
    Json(body): Json<ReviewFeatureFlagChangeRequest>,
) -> Response {
    ops_json(
        state
            .admin_ops()
            .review_feature_flag_change_request(
                &request_id,
                body.approved,
                body.review_note.as_deref(),
            )
            .await,
    )
    .await
}

async fn audit_logs(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(query): Query<AuditQuery>,
) -> Response {
    let format = query.format.clone();
    let audit_query = audit_query(query);
    let ops = state.admin_ops();
    if format.as_deref() == Some("csv") {
        return match ops.export_audit_logs_csv(&audit_query).await {
            Ok(csv) => (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
                csv,
            )
                .into_response(),
            Err(error) => app_error_response::<AdminAuditLogPage>(error),
        };
    }
    ops_json(ops.list_audit_logs(&audit_query).await).await
}
