use avrag_auth::OrgId;
use common::UserId;

#[derive(Debug, serde::Serialize)]
pub struct OrgInfo {
    pub id: OrgId,
    pub name: String,
    pub created_at: i64,
    pub blocked: bool,
    pub user_count: i64,
    pub document_count: i64,
    pub query_count: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct UserInfo {
    pub id: UserId,
    pub email: String,
    pub org_id: OrgId,
    pub role: String,
    pub created_at: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct UsageStats {
    pub org_id: OrgId,
    pub period: String,
    pub query_count: i64,
    pub document_count: i64,
    pub chunk_count: i64,
    pub storage_bytes: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct HealthStatus {
    pub status: String,
    pub version: String,
    pub uptime_secs: i64,
}

#[derive(Debug, Clone)]
pub struct AuditLogQuery {
    pub query: Option<String>,
    pub action: Option<String>,
    pub resource_type: Option<String>,
    pub actor: Option<String>,
    pub window: Option<String>,
    pub page: usize,
    pub per_page: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct AuditLogEntry {
    pub id: i64,
    pub actor_id: Option<String>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: String,
    pub org_id: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct AuditLogPage {
    pub items: Vec<AuditLogEntry>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
}
