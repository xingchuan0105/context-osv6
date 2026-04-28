use app::AppState;
use axum::{
    Extension, Json, Router,
    extract::{Path, Query},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use common::ApiResponse;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::RequestState;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/organizations", get(list_orgs))
        .route("/admin/organizations/{org_id}", get(get_org))
        .route("/admin/users", get(list_users))
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

#[derive(Serialize)]
struct BillingOverview {
    active_subscriptions: i64,
    past_due_subscriptions: i64,
    unpaid_subscriptions: i64,
    canceled_subscriptions: i64,
}

#[derive(Serialize)]
struct RagHealthStatus {
    failed_documents: i64,
    queued_tasks: i64,
    processing_tasks: i64,
    dead_letter_tasks: i64,
    recent_guard_events: i64,
}

#[derive(Serialize)]
struct WorkerStatus {
    runtime_mode: &'static str,
    queued_tasks: i64,
    processing_tasks: i64,
    dead_letter_tasks: i64,
    failed_documents: i64,
}

#[derive(Serialize)]
struct DegradationStatus {
    failed_documents: i64,
    recent_guard_events: i64,
    share_access_events: i64,
}

#[derive(Serialize)]
struct FeatureFlagEntry {
    key: String,
    category: String,
    description: String,
    enabled: bool,
    effective_enabled: bool,
    config_ready: bool,
    requires_config: bool,
    source: String,
    updated_at: Option<i64>,
    has_pending_request: bool,
}

#[derive(Serialize)]
struct FeatureFlagChangeRequestRow {
    id: String,
    flag_key: String,
    current_enabled: bool,
    requested_enabled: bool,
    reason: String,
    status: String,
    requested_by: String,
    reviewed_by: Option<String>,
    review_note: Option<String>,
    created_at: i64,
    reviewed_at: Option<i64>,
    executed_at: Option<i64>,
}

macro_rules! repo_or_response {
    ($state:expr) => {
        match $state.pg() {
            Some(repo) => repo,
            None => {
                return Json(ApiResponse::<serde_json::Value>::err(
                    "postgres_not_configured",
                    "postgres backend is not configured",
                ))
                .into_response();
            }
        }
    };
}

fn parse_org_id(value: &str) -> Result<common::OrgId, Response> {
    value.parse::<common::OrgId>().map_err(|_| {
        Json(ApiResponse::<serde_json::Value>::err(
            "invalid_org_id",
            "org_id must be a UUID",
        ))
        .into_response()
    })
}

fn actor_uuid(state: &AppState) -> Result<Uuid, Response> {
    state
        .auth()
        .actor_id()
        .map(|actor| actor.into_uuid())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse::<serde_json::Value>::err(
                    "authenticated_user_required",
                    "admin action requires an authenticated user",
                )),
            )
                .into_response()
        })
}

async fn ensure_admin_access(state: &AppState) -> Result<(), Response> {
    let tx = begin_admin_tx(state).await?;
    commit_admin_tx(tx).await
}

async fn begin_admin_tx(
    state: &AppState,
) -> Result<sqlx::Transaction<'_, sqlx::Postgres>, Response> {
    let actor_id = actor_uuid(state)?;
    let Some(repo) = state.pg() else {
        return Err(Json(ApiResponse::<serde_json::Value>::err(
            "postgres_not_configured",
            "postgres backend is not configured",
        ))
        .into_response());
    };
    let mut tx = repo.raw().begin().await.map_err(|error| {
        Json(ApiResponse::<serde_json::Value>::err(
            "admin_access_check_failed",
            &error.to_string(),
        ))
        .into_response()
    })?;
    sqlx::query("select set_config('app.current_org', $1, true)")
        .bind(state.auth().org_id().to_string())
        .execute(tx.as_mut())
        .await
        .map_err(|error| {
            Json(ApiResponse::<serde_json::Value>::err(
                "admin_access_check_failed",
                &error.to_string(),
            ))
            .into_response()
        })?;
    let role =
        sqlx::query_scalar::<_, String>("select role from users where id = $1 and org_id = $2")
            .bind(actor_id)
            .bind(state.auth().org_id().into_uuid())
            .fetch_optional(tx.as_mut())
            .await
            .map_err(|error| {
                Json(ApiResponse::<serde_json::Value>::err(
                    "admin_access_check_failed",
                    &error.to_string(),
                ))
                .into_response()
            })?;
    if matches!(
        role.as_deref(),
        Some("super_admin" | "ops_admin" | "finance_admin")
    ) {
        sqlx::query("select set_config('app.current_role', $1, true)")
            .bind(role.expect("role checked as Some above"))
            .execute(tx.as_mut())
            .await
            .map_err(|error| {
                Json(ApiResponse::<serde_json::Value>::err(
                    "admin_access_check_failed",
                    &error.to_string(),
                ))
                .into_response()
            })?;
        return Ok(tx);
    }
    Err((
        StatusCode::FORBIDDEN,
        Json(ApiResponse::<serde_json::Value>::err(
            "admin_access_denied",
            "admin access denied",
        )),
    )
        .into_response())
}

async fn commit_admin_tx(tx: sqlx::Transaction<'_, sqlx::Postgres>) -> Result<(), Response> {
    tx.commit().await.map_err(|error| {
        Json(ApiResponse::<serde_json::Value>::err(
            "admin_access_check_failed",
            &error.to_string(),
        ))
        .into_response()
    })
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
    let mut tx = match begin_admin_tx(&state).await {
        Ok(tx) => tx,
        Err(response) => return response,
    };
    let row = sqlx::query(
        r#"
        select
          count(*) filter (where status = 'active') as active_subscriptions,
          count(*) filter (where status = 'past_due') as past_due_subscriptions,
          count(*) filter (where status = 'unpaid') as unpaid_subscriptions,
          count(*) filter (where status = 'canceled') as canceled_subscriptions
        from subscriptions
        "#,
    )
    .fetch_one(tx.as_mut())
    .await;
    match row {
        Ok(row) => {
            if let Err(response) = commit_admin_tx(tx).await {
                return response;
            }
            Json(ApiResponse::ok(BillingOverview {
                active_subscriptions: row.try_get("active_subscriptions").unwrap_or(0),
                past_due_subscriptions: row.try_get("past_due_subscriptions").unwrap_or(0),
                unpaid_subscriptions: row.try_get("unpaid_subscriptions").unwrap_or(0),
                canceled_subscriptions: row.try_get("canceled_subscriptions").unwrap_or(0),
            }))
            .into_response()
        }
        Err(error) => Json(ApiResponse::<BillingOverview>::err(
            "admin_billing_failed",
            &error.to_string(),
        ))
        .into_response(),
    }
}

async fn rag_health(Extension(RequestState(state)): Extension<RequestState>) -> Response {
    let mut tx = match begin_admin_tx(&state).await {
        Ok(tx) => tx,
        Err(response) => return response,
    };
    let response = RagHealthStatus {
        failed_documents: scalar_count(
            tx.as_mut(),
            "select count(*) from documents where status in ('failed','Failed')",
        )
        .await,
        queued_tasks: scalar_count(
            tx.as_mut(),
            "select count(*) from ingestion_tasks where status = 'queued'",
        )
        .await,
        processing_tasks: scalar_count(
            tx.as_mut(),
            "select count(*) from ingestion_tasks where status in ('claimed','processing')",
        )
        .await,
        dead_letter_tasks: scalar_count(
            tx.as_mut(),
            "select count(*) from ingestion_tasks where status = 'dead_letter' or dead_lettered_at is not null",
        )
        .await,
        recent_guard_events: scalar_count(
            tx.as_mut(),
            "select count(*) from audit_log where action like '%guard%' and created_at >= now() - interval '24 hours'",
        )
        .await,
    };
    if let Err(response) = commit_admin_tx(tx).await {
        return response;
    }
    Json(ApiResponse::ok(response)).into_response()
}

async fn worker_status(Extension(RequestState(state)): Extension<RequestState>) -> Response {
    let mut tx = match begin_admin_tx(&state).await {
        Ok(tx) => tx,
        Err(response) => return response,
    };
    let response = WorkerStatus {
        runtime_mode: "milvus",
        queued_tasks: scalar_count(
            tx.as_mut(),
            "select count(*) from ingestion_tasks where status = 'queued'",
        )
        .await,
        processing_tasks: scalar_count(
            tx.as_mut(),
            "select count(*) from ingestion_tasks where status in ('claimed','processing')",
        )
        .await,
        dead_letter_tasks: scalar_count(
            tx.as_mut(),
            "select count(*) from ingestion_tasks where status = 'dead_letter' or dead_lettered_at is not null",
        )
        .await,
        failed_documents: scalar_count(
            tx.as_mut(),
            "select count(*) from documents where status in ('failed','Failed')",
        )
        .await,
    };
    if let Err(response) = commit_admin_tx(tx).await {
        return response;
    }
    Json(ApiResponse::ok(response)).into_response()
}

async fn degradation_status(Extension(RequestState(state)): Extension<RequestState>) -> Response {
    let mut tx = match begin_admin_tx(&state).await {
        Ok(tx) => tx,
        Err(response) => return response,
    };
    let response = DegradationStatus {
        failed_documents: scalar_count(
            tx.as_mut(),
            "select count(*) from documents where status in ('failed','Failed')",
        )
        .await,
        recent_guard_events: scalar_count(
            tx.as_mut(),
            "select count(*) from audit_log where action like '%guard%' and created_at >= now() - interval '24 hours'",
        )
        .await,
        share_access_events: scalar_count(
            tx.as_mut(),
            "select count(*) from share_access_logs where created_at >= now() - interval '24 hours'",
        )
        .await,
    };
    if let Err(response) = commit_admin_tx(tx).await {
        return response;
    }
    Json(ApiResponse::ok(response)).into_response()
}

async fn scalar_count(conn: &mut sqlx::PgConnection, sql: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(sql)
        .fetch_one(conn)
        .await
        .unwrap_or(0)
}

async fn feature_flags(Extension(RequestState(state)): Extension<RequestState>) -> Response {
    let mut tx = match begin_admin_tx(&state).await {
        Ok(tx) => tx,
        Err(response) => return response,
    };
    let rows = sqlx::query(
        r#"
        select f.key, f.enabled, f.source, extract(epoch from f.updated_at)::bigint as updated_at,
          exists(select 1 from feature_flag_change_requests r where r.flag_key = f.key and r.status = 'pending') as has_pending_request
        from feature_flags f
        order by f.key asc
        "#,
    )
    .fetch_all(tx.as_mut())
    .await;
    match rows {
        Ok(rows) => {
            if let Err(response) = commit_admin_tx(tx).await {
                return response;
            }
            Json(ApiResponse::ok(
                rows.into_iter()
                    .map(|row| FeatureFlagEntry {
                        key: row.try_get("key").unwrap_or_default(),
                        category: "runtime".to_string(),
                        description: String::new(),
                        enabled: row.try_get("enabled").unwrap_or(false),
                        effective_enabled: row.try_get("enabled").unwrap_or(false),
                        config_ready: true,
                        requires_config: false,
                        source: row
                            .try_get("source")
                            .unwrap_or_else(|_| "admin_panel".to_string()),
                        updated_at: row.try_get("updated_at").ok(),
                        has_pending_request: row.try_get("has_pending_request").unwrap_or(false),
                    })
                    .collect::<Vec<_>>(),
            ))
            .into_response()
        }
        Err(error) => Json(ApiResponse::<Vec<FeatureFlagEntry>>::err(
            "feature_flags_failed",
            &error.to_string(),
        ))
        .into_response(),
    }
}

async fn feature_flag_change_requests(
    Extension(RequestState(state)): Extension<RequestState>,
    Query(query): Query<FeatureFlagChangeRequestQuery>,
) -> Response {
    let mut tx = match begin_admin_tx(&state).await {
        Ok(tx) => tx,
        Err(response) => return response,
    };
    let rows = if let Some(status) = query.status.filter(|value| !value.trim().is_empty()) {
        sqlx::query("select *, extract(epoch from created_at)::bigint as created_epoch, extract(epoch from reviewed_at)::bigint as reviewed_epoch, extract(epoch from executed_at)::bigint as executed_epoch from feature_flag_change_requests where status = $1 order by created_at desc")
            .bind(status)
            .fetch_all(tx.as_mut())
            .await
    } else {
        sqlx::query("select *, extract(epoch from created_at)::bigint as created_epoch, extract(epoch from reviewed_at)::bigint as reviewed_epoch, extract(epoch from executed_at)::bigint as executed_epoch from feature_flag_change_requests order by created_at desc")
            .fetch_all(tx.as_mut())
            .await
    };
    match rows {
        Ok(rows) => {
            if let Err(response) = commit_admin_tx(tx).await {
                return response;
            }
            Json(ApiResponse::ok(
                rows.into_iter()
                    .map(map_feature_flag_change_request)
                    .collect::<Vec<_>>(),
            ))
            .into_response()
        }
        Err(error) => Json(ApiResponse::<Vec<FeatureFlagChangeRequestRow>>::err(
            "feature_flag_requests_failed",
            &error.to_string(),
        ))
        .into_response(),
    }
}

async fn create_feature_flag_change_request(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(key): Path<String>,
    Json(body): Json<CreateFeatureFlagChangeRequest>,
) -> Response {
    let mut tx = match begin_admin_tx(&state).await {
        Ok(tx) => tx,
        Err(response) => return response,
    };
    let actor_id = match actor_uuid(&state) {
        Ok(actor_id) => actor_id,
        Err(response) => return response,
    };
    if let Err(error) = sqlx::query(
        "insert into feature_flags (key, enabled) values ($1, false) on conflict (key) do nothing",
    )
    .bind(&key)
    .execute(tx.as_mut())
    .await
    {
        return Json(ApiResponse::<FeatureFlagChangeRequestRow>::err(
            "feature_flag_request_failed",
            &error.to_string(),
        ))
        .into_response();
    }
    let current_enabled =
        sqlx::query_scalar::<_, bool>("select enabled from feature_flags where key = $1")
            .bind(&key)
            .fetch_one(tx.as_mut())
            .await
            .unwrap_or(false);
    let id = Uuid::new_v4().to_string();
    let row = sqlx::query("insert into feature_flag_change_requests (id, flag_key, current_enabled, requested_enabled, reason, status, requested_by) values ($1, $2, $3, $4, $5, 'pending', $6) returning *, extract(epoch from created_at)::bigint as created_epoch, extract(epoch from reviewed_at)::bigint as reviewed_epoch, extract(epoch from executed_at)::bigint as executed_epoch")
        .bind(&id)
        .bind(&key)
        .bind(current_enabled)
        .bind(body.enabled)
        .bind(body.reason)
        .bind(actor_id)
        .fetch_one(tx.as_mut())
        .await;
    match row {
        Ok(row) => {
            let response =
                Json(ApiResponse::ok(map_feature_flag_change_request(row))).into_response();
            if let Err(response) = commit_admin_tx(tx).await {
                return response;
            }
            response
        }
        Err(error) => Json(ApiResponse::<FeatureFlagChangeRequestRow>::err(
            "feature_flag_request_failed",
            &error.to_string(),
        ))
        .into_response(),
    }
}

async fn review_feature_flag_change_request(
    Extension(RequestState(state)): Extension<RequestState>,
    Path(request_id): Path<String>,
    Json(body): Json<ReviewFeatureFlagChangeRequest>,
) -> Response {
    let mut tx = match begin_admin_tx(&state).await {
        Ok(tx) => tx,
        Err(response) => return response,
    };
    let actor_id = match actor_uuid(&state) {
        Ok(actor_id) => actor_id,
        Err(response) => return response,
    };
    let status = if body.approved {
        "approved"
    } else {
        "rejected"
    };
    let row = sqlx::query("update feature_flag_change_requests set status = $2, reviewed_by = $3, review_note = $4, reviewed_at = now(), executed_at = case when $2 = 'approved' then now() else executed_at end where id = $1 returning *, extract(epoch from created_at)::bigint as created_epoch, extract(epoch from reviewed_at)::bigint as reviewed_epoch, extract(epoch from executed_at)::bigint as executed_epoch")
        .bind(&request_id)
        .bind(status)
        .bind(actor_id)
        .bind(body.review_note)
        .fetch_one(tx.as_mut())
        .await;
    match row {
        Ok(row) => {
            let response =
                Json(ApiResponse::ok(map_feature_flag_change_request(row))).into_response();
            if let Err(response) = commit_admin_tx(tx).await {
                return response;
            }
            response
        }
        Err(error) => Json(ApiResponse::<FeatureFlagChangeRequestRow>::err(
            "feature_flag_review_failed",
            &error.to_string(),
        ))
        .into_response(),
    }
}

fn map_feature_flag_change_request(row: sqlx::postgres::PgRow) -> FeatureFlagChangeRequestRow {
    FeatureFlagChangeRequestRow {
        id: row.try_get("id").unwrap_or_default(),
        flag_key: row.try_get("flag_key").unwrap_or_default(),
        current_enabled: row.try_get("current_enabled").unwrap_or(false),
        requested_enabled: row.try_get("requested_enabled").unwrap_or(false),
        reason: row.try_get("reason").unwrap_or_default(),
        status: row.try_get("status").unwrap_or_default(),
        requested_by: row
            .try_get::<Uuid, _>("requested_by")
            .map(|value| value.to_string())
            .unwrap_or_default(),
        reviewed_by: row
            .try_get::<Option<Uuid>, _>("reviewed_by")
            .ok()
            .flatten()
            .map(|value| value.to_string()),
        review_note: row.try_get("review_note").ok(),
        created_at: row.try_get("created_epoch").unwrap_or(0),
        reviewed_at: row
            .try_get::<Option<i64>, _>("reviewed_epoch")
            .ok()
            .flatten(),
        executed_at: row
            .try_get::<Option<i64>, _>("executed_epoch")
            .ok()
            .flatten(),
    }
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
