use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagHealthStatus {
    #[typeshare(serialized_as = "number")]
    pub failed_documents: i64,
    #[typeshare(serialized_as = "number")]
    pub queued_tasks: i64,
    #[typeshare(serialized_as = "number")]
    pub processing_tasks: i64,
    #[typeshare(serialized_as = "number")]
    pub recent_guard_events: i64,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadyResponse {
    pub ready: bool,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgRow {
    pub id: String,
    pub name: String,
    pub plan: String,
    #[typeshare(serialized_as = "number")]
    pub user_count: i64,
    #[typeshare(serialized_as = "number")]
    pub notebook_count: i64,
    #[typeshare(serialized_as = "number")]
    pub query_count: i64,
    pub blocked: bool,
    pub created_at: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgListResponse {
    pub orgs: Vec<OrgRow>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgResponse {
    pub org: OrgRow,
}

#[typeshare]
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

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserListResponse {
    pub users: Vec<UserRow>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminUsageResponse {
    #[typeshare(serialized_as = "number")]
    pub total_requests: i64,
    #[typeshare(serialized_as = "number")]
    pub total_tokens: i64,
    #[typeshare(serialized_as = "number")]
    pub total_documents: i64,
}

#[typeshare]
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
    #[typeshare(serialized_as = "number")]
    pub updated_at: Option<i64>,
    pub has_pending_request: bool,
}

#[typeshare]
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
    #[typeshare(serialized_as = "number")]
    pub created_at: i64,
    #[typeshare(serialized_as = "number")]
    pub reviewed_at: Option<i64>,
    #[typeshare(serialized_as = "number")]
    pub executed_at: Option<i64>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerStatusResponse {
    pub runtime_mode: String,
    #[typeshare(serialized_as = "number")]
    pub queued_tasks: i64,
    #[typeshare(serialized_as = "number")]
    pub processing_tasks: i64,
    #[typeshare(serialized_as = "number")]
    pub failed_documents: i64,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DegradationStatusResponse {
    #[typeshare(serialized_as = "number")]
    pub failed_documents: i64,
    #[typeshare(serialized_as = "number")]
    pub recent_guard_events: i64,
    #[typeshare(serialized_as = "number")]
    pub share_access_events: i64,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    #[typeshare(serialized_as = "number")]
    pub id: i64,
    pub actor_id: Option<String>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: String,
    pub org_id: Option<String>,
    #[typeshare(serialized_as = "number")]
    pub created_at: i64,
}

#[typeshare]
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
    #[typeshare(serialized_as = "number")]
    pub page: Option<usize>,
    #[serde(default)]
    #[typeshare(serialized_as = "number")]
    pub per_page: Option<usize>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogListResponse {
    pub items: Vec<AuditLogEntry>,
    #[typeshare(serialized_as = "number")]
    pub total: usize,
    #[typeshare(serialized_as = "number")]
    pub page: usize,
    #[typeshare(serialized_as = "number")]
    pub per_page: usize,
}
