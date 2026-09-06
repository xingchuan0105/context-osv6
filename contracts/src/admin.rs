//! Admin ops 协议契约（对齐后端 `app-core/src/admin_domain.rs` 序列化形状）。
//! 契约与后端同源维护：字段名 / 类型 / 可选性一一对应，ids 为字符串（UUID 透明序列化）。
//! 消费方：`frontend_rust/crates/web-sdk/src/admin_api.rs`。

use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminAccountInfo {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub blocked: bool,
    pub user_count: i64,
    pub document_count: i64,
    pub query_count: i64,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminUserInfo {
    pub id: String,
    pub email: String,
    pub role: String,
    pub created_at: i64,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminUsageStats {
    pub owner_user_id: String,
    pub period: String,
    pub query_count: i64,
    pub document_count: i64,
    pub chunk_count: i64,
    pub storage_bytes: i64,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminHealthStatus {
    pub status: String,
    pub version: String,
    pub uptime_secs: i64,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminBillingOverview {
    pub active_subscriptions: i64,
    pub past_due_subscriptions: i64,
    pub unpaid_subscriptions: i64,
    pub canceled_subscriptions: i64,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminRagHealthStatus {
    pub failed_documents: i64,
    pub queued_tasks: i64,
    pub processing_tasks: i64,
    pub dead_letter_tasks: i64,
    pub recent_guard_events: i64,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminWorkerStatus {
    pub runtime_mode: String,
    pub queued_tasks: i64,
    pub processing_tasks: i64,
    pub dead_letter_tasks: i64,
    pub failed_documents: i64,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminDegradationStatus {
    pub failed_documents: i64,
    pub recent_guard_events: i64,
    pub share_access_events: i64,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminFeatureFlagEntry {
    pub key: String,
    pub category: String,
    pub description: String,
    pub enabled: bool,
    pub effective_enabled: bool,
    pub config_ready: bool,
    pub requires_config: bool,
    pub source: String,
    #[typeshare(serialized_as = "Option<f64>")]
    pub updated_at: Option<i64>,
    pub has_pending_request: bool,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[typeshare(serialized_as = "f64")]
    pub created_at: i64,
    #[typeshare(serialized_as = "Option<f64>")]
    pub reviewed_at: Option<i64>,
    #[typeshare(serialized_as = "Option<f64>")]
    pub executed_at: Option<i64>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminAuditLogEntry {
    #[typeshare(serialized_as = "f64")]
    pub id: i64,
    pub actor_id: Option<String>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: String,
    pub owner_user_id: Option<String>,
    #[typeshare(serialized_as = "f64")]
    pub created_at: i64,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdminAuditLogQuery {
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
    #[typeshare(serialized_as = "Option<f64>")]
    pub page: Option<usize>,
    #[serde(default)]
    #[typeshare(serialized_as = "Option<f64>")]
    pub per_page: Option<usize>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminAuditLogPage {
    pub items: Vec<AdminAuditLogEntry>,
    #[typeshare(serialized_as = "f64")]
    pub total: usize,
    #[typeshare(serialized_as = "f64")]
    pub page: usize,
    #[typeshare(serialized_as = "f64")]
    pub per_page: usize,
}

/// 全平台公告广播结果（`POST /api/v1/admin/notifications/broadcast`）。
#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminBroadcastResult {
    pub created: i64,
}
