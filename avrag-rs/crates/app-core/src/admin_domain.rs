use serde::Serialize;

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
