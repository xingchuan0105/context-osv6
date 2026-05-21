use std::sync::Arc;

use avrag_auth::OrgId;
use avrag_storage_pg::PgAppRepository;
use common::ApiResponse;
use uuid::Uuid;

use crate::models::{AuditLogPage, AuditLogQuery, HealthStatus, OrgInfo, UsageStats, UserInfo};
use crate::service::AdminService;

use avrag_auth::AuthContext;

pub async fn handle_list_orgs(
    ctx: AuthContext,
    repo: Arc<PgAppRepository>,
) -> ApiResponse<Vec<OrgInfo>> {
    let service = AdminService::new(repo);
    match service.list_orgs(&ctx).await {
        Ok(orgs) => ApiResponse::ok(orgs),
        Err(e) => ApiResponse::err("list_orgs_failed", &e.to_string()),
    }
}

pub async fn handle_get_org(
    ctx: AuthContext,
    org_id: OrgId,
    repo: Arc<PgAppRepository>,
) -> ApiResponse<OrgInfo> {
    let service = AdminService::new(repo);
    match service.get_org(&ctx, org_id).await {
        Ok(Some(org)) => ApiResponse::ok(org),
        Ok(None) => ApiResponse::err("org_not_found", "Organization not found"),
        Err(e) => ApiResponse::err("get_org_failed", &e.to_string()),
    }
}

pub async fn handle_list_users(
    ctx: AuthContext,
    org_id: OrgId,
    repo: Arc<PgAppRepository>,
) -> ApiResponse<Vec<UserInfo>> {
    let service = AdminService::new(repo);
    match service.list_users(&ctx, org_id).await {
        Ok(users) => ApiResponse::ok(users),
        Err(e) => ApiResponse::err("list_users_failed", &e.to_string()),
    }
}

pub async fn handle_delete_user(
    ctx: AuthContext,
    org_id: OrgId,
    user_id: Uuid,
    repo: Arc<PgAppRepository>,
) -> ApiResponse<()> {
    let service = AdminService::new(repo);
    match service.delete_user(&ctx, org_id, user_id).await {
        Ok(true) => ApiResponse::ok(()),
        Ok(false) => ApiResponse::err("user_not_found", "User not found in this organization"),
        Err(e) => ApiResponse::err("delete_user_failed", &e.to_string()),
    }
}

pub async fn handle_get_usage(
    ctx: AuthContext,
    org_id: OrgId,
    period: String,
    repo: Arc<PgAppRepository>,
) -> ApiResponse<UsageStats> {
    let service = AdminService::new(repo);
    match service.get_usage(&ctx, org_id, &period).await {
        Ok(stats) => ApiResponse::ok(stats),
        Err(e) => ApiResponse::err("get_usage_failed", &e.to_string()),
    }
}

pub async fn handle_block_org(
    ctx: AuthContext,
    org_id: OrgId,
    blocked: bool,
    repo: Arc<PgAppRepository>,
) -> ApiResponse<()> {
    let service = AdminService::new(repo);
    match service.set_org_blocked(&ctx, org_id, blocked).await {
        Ok(()) => ApiResponse::ok(()),
        Err(e) => ApiResponse::err("block_org_failed", &e.to_string()),
    }
}

pub async fn handle_health() -> ApiResponse<HealthStatus> {
    ApiResponse::ok(AdminService::get_health().await)
}

pub async fn handle_list_audit_logs(
    ctx: AuthContext,
    query: AuditLogQuery,
    repo: Arc<PgAppRepository>,
) -> ApiResponse<AuditLogPage> {
    let service = AdminService::new(repo);
    match service.list_audit_logs(&ctx, &query).await {
        Ok(page) => ApiResponse::ok(page),
        Err(error) => ApiResponse::err("list_audit_logs_failed", &error.to_string()),
    }
}

pub async fn handle_export_audit_logs_csv(
    ctx: AuthContext,
    query: AuditLogQuery,
    repo: Arc<PgAppRepository>,
) -> ApiResponse<String> {
    let service = AdminService::new(repo);
    match service.export_audit_logs_csv(&ctx, &query).await {
        Ok(csv) => ApiResponse::ok(csv),
        Err(error) => ApiResponse::err("audit_logs_export_failed", &error.to_string()),
    }
}
