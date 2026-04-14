use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagHealthStatus {
    pub failed_documents: i64,
    pub queued_tasks: i64,
    pub processing_tasks: i64,
    pub recent_guard_events: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadyResponse {
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgRow {
    pub id: String,
    pub name: String,
    pub plan: String,
    pub user_count: i64,
    pub notebook_count: i64,
    pub query_count: i64,
    pub blocked: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgListResponse {
    pub orgs: Vec<OrgRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgResponse {
    pub org: OrgRow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRow {
    pub id: String,
    pub email: String,
    pub full_name: String,
    pub org_id: String,
    pub role: String,
    pub created_at: String,
    pub last_active_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserListResponse {
    pub users: Vec<UserRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminUsageResponse {
    pub total_requests: i64,
    pub total_tokens: i64,
    pub total_documents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlagEntry {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlagChangeRequest {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerStatusResponse {
    pub runtime_mode: String,
    pub queued_tasks: i64,
    pub processing_tasks: i64,
    pub failed_documents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DegradationStatusResponse {
    pub failed_documents: i64,
    pub recent_guard_events: i64,
    pub share_access_events: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: i64,
    pub actor_id: Option<String>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: String,
    pub org_id: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuditLogQuery {
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub resource_type: Option<String>,
    #[serde(default)]
    pub actor: Option<String>,
    #[serde(default)]
    pub window: Option<String>,
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub per_page: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogListResponse {
    pub items: Vec<AuditLogEntry>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
}
