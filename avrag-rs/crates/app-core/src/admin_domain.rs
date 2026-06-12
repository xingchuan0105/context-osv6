use avrag_auth::OrgId;
use chrono::{DateTime, Utc};
use common::UserId;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AdminOrgInfo {
    pub id: OrgId,
    pub name: String,
    pub created_at: i64,
    pub blocked: bool,
    pub user_count: i64,
    pub document_count: i64,
    pub query_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdminUserInfo {
    pub id: UserId,
    pub email: String,
    pub org_id: OrgId,
    pub role: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdminUsageStats {
    pub org_id: OrgId,
    pub period: String,
    pub query_count: i64,
    pub document_count: i64,
    pub chunk_count: i64,
    pub storage_bytes: i64,
}

#[derive(Debug, Clone)]
pub struct AdminAuditLogQuery {
    pub query: Option<String>,
    pub action: Option<String>,
    pub resource_type: Option<String>,
    pub actor: Option<String>,
    pub window: Option<String>,
    pub page: usize,
    pub per_page: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdminAuditLogEntry {
    pub id: i64,
    pub actor_id: Option<String>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: String,
    pub org_id: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdminAuditLogPage {
    pub items: Vec<AdminAuditLogEntry>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
}

pub fn admin_usage_period_start(period: &str) -> DateTime<Utc> {
    let days = match period {
        "7d" => 7,
        "90d" => 90,
        _ => 30,
    };
    Utc::now() - chrono::TimeDelta::days(days)
}

pub fn admin_clamp_audit_per_page(value: usize) -> usize {
    value.clamp(1, 200)
}

pub fn admin_audit_window_start(window: Option<&str>) -> Option<DateTime<Utc>> {
    let duration = match window {
        Some("24h") => Some(chrono::TimeDelta::hours(24)),
        Some("7d") => Some(chrono::TimeDelta::days(7)),
        Some("30d") => Some(chrono::TimeDelta::days(30)),
        Some("90d") => Some(chrono::TimeDelta::days(90)),
        _ => None,
    }?;
    Some(Utc::now() - duration)
}

pub fn admin_audit_logs_to_csv(items: &[AdminAuditLogEntry]) -> String {
    let mut lines =
        vec!["id,action,resource_type,resource_id,actor_id,org_id,created_at".to_string()];
    for item in items {
        lines.push(format!(
            "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"",
            item.id,
            item.action.replace('"', "\"\""),
            item.resource_type.replace('"', "\"\""),
            item.resource_id.replace('"', "\"\""),
            item.actor_id
                .clone()
                .unwrap_or_default()
                .replace('"', "\"\""),
            item.org_id.clone().unwrap_or_default().replace('"', "\"\""),
            item.created_at
        ));
    }
    lines.join("\n")
}

#[derive(Debug, Clone, Serialize)]
pub struct AdminBillingOverview {
    pub active_subscriptions: i64,
    pub past_due_subscriptions: i64,
    pub unpaid_subscriptions: i64,
    pub canceled_subscriptions: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdminRagHealthStatus {
    pub failed_documents: i64,
    pub queued_tasks: i64,
    pub processing_tasks: i64,
    pub dead_letter_tasks: i64,
    pub recent_guard_events: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdminWorkerStatus {
    pub runtime_mode: &'static str,
    pub queued_tasks: i64,
    pub processing_tasks: i64,
    pub dead_letter_tasks: i64,
    pub failed_documents: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdminDegradationStatus {
    pub failed_documents: i64,
    pub recent_guard_events: i64,
    pub share_access_events: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdminFeatureFlagEntry {
    pub key: String,
    pub category: String,
    pub description: String,
    pub enabled: bool,
    pub effective_enabled: bool,
    pub config_ready: bool,
    pub requires_config: bool,
    pub source: String,
    pub updated_at: Option<i64>,
    pub has_pending_request: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdminFeatureFlagChangeRequest {
    pub id: String,
    pub flag_key: String,
    pub current_enabled: bool,
    pub requested_enabled: bool,
    pub reason: String,
    pub status: String,
    pub requested_by: String,
    pub reviewed_by: Option<String>,
    pub review_note: Option<String>,
    pub created_at: i64,
    pub reviewed_at: Option<i64>,
    pub executed_at: Option<i64>,
}
