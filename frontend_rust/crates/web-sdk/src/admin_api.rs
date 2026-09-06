/// `/api/v1/admin/*` 客户端：URL 构造 + `ApiResponse` 信封解析。
/// DTO 单一源是 `contracts::admin`（与后端 `app-core/admin_domain.rs` 对齐），此处不再重复定义。
pub use contracts::admin::{
    AdminAccountInfo, AdminAuditLogEntry, AdminAuditLogPage, AdminAuditLogQuery,
    AdminBillingOverview, AdminBroadcastResult, AdminDegradationStatus,
    AdminFeatureFlagChangeRequest, AdminFeatureFlagEntry, AdminHealthStatus,
    AdminRagHealthStatus, AdminUserInfo, AdminUsageStats, AdminWorkerStatus,
};

use crate::conversation_api::{encode_path_segment, trim_base_url};
use crate::transport::TransportError;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// `/api/v1/admin/*` 统一响应信封（后端 `common::ApiResponse<T>`）。
#[derive(Debug, Clone)]
pub struct AdminEnvelope<T> {
    pub data: Option<T>,
    pub error: Option<AdminApiError>,
    pub ok: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AdminApiError {
    pub code: String,
    pub message: String,
}

fn parse_admin_envelope<T: DeserializeOwned>(body: &[u8]) -> Result<AdminEnvelope<T>, TransportError> {
    let value: serde_json::Value = serde_json::from_slice(body).map_err(TransportError::from)?;
    let ok = value.get("ok").and_then(serde_json::Value::as_bool).unwrap_or(false);
    let error = match value.get("error") {
        None | Some(serde_json::Value::Null) => None,
        Some(raw) => Some(serde_json::from_value::<AdminApiError>(raw.clone()).map_err(TransportError::from)?),
    };
    let data = match value.get("data") {
        None | Some(serde_json::Value::Null) => None,
        Some(raw) => Some(serde_json::from_value::<T>(raw.clone()).map_err(TransportError::from)?),
    };
    Ok(AdminEnvelope { data, error, ok })
}

fn envelope_into_data<T>(envelope: AdminEnvelope<T>) -> Result<T, TransportError> {
    if !envelope.ok {
        let err = envelope.error.unwrap_or(AdminApiError {
            code: "admin_error".to_string(),
            message: "admin api returned ok=false".to_string(),
        });
        return Err(TransportError::HttpStatus {
            status: 0,
            body: format!("{}: {}", err.code, err.message),
        });
    }
    envelope.data.ok_or(TransportError::EmptyBody)
}

fn parse_admin_data<T: DeserializeOwned>(body: &[u8]) -> Result<T, TransportError> {
    envelope_into_data(parse_admin_envelope::<T>(body)?)
}

/// 广播请求体（admin 域少有的请求 DTO；响应见 `AdminBroadcastResult`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminBroadcastRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_type: Option<String>,
    pub title: String,
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

pub fn admin_accounts_url(base_url: &str, page: usize, per_page: usize) -> String {
    let mut url = format!("{}/api/v1/admin/accounts", trim_base_url(base_url));
    if page > 0 || per_page > 0 {
        url.push('?');
        let mut params: Vec<String> = Vec::new();
        if page > 0 {
            params.push(format!("page={page}"));
        }
        if per_page > 0 {
            params.push(format!("per_page={per_page}"));
        }
        url.push_str(&params.join("&"));
    }
    url
}

pub fn admin_account_url(base_url: &str, owner_user_id: &str) -> String {
    format!(
        "{}/api/v1/admin/accounts/{}",
        trim_base_url(base_url),
        encode_path_segment(owner_user_id)
    )
}

pub fn admin_users_url(base_url: &str, owner_user_id: &str) -> String {
    format!(
        "{}/api/v1/admin/users?owner_user_id={}",
        trim_base_url(base_url),
        encode_path_segment(owner_user_id)
    )
}

pub fn admin_user_url(base_url: &str, user_id: &str) -> String {
    format!(
        "{}/api/v1/admin/users/{}",
        trim_base_url(base_url),
        encode_path_segment(user_id)
    )
}

pub fn admin_usage_url(base_url: &str, owner_user_id: &str, period: &str) -> String {
    format!(
        "{}/api/v1/admin/usage?owner_user_id={}&period={}",
        trim_base_url(base_url),
        encode_path_segment(owner_user_id),
        encode_path_segment(period)
    )
}

pub fn admin_health_url(base_url: &str) -> String {
    format!("{}/api/v1/admin/health", trim_base_url(base_url))
}

pub fn admin_billing_url(base_url: &str) -> String {
    format!("{}/api/v1/admin/billing", trim_base_url(base_url))
}

pub fn admin_billing_block_url(base_url: &str) -> String {
    format!("{}/api/v1/admin/billing/block", trim_base_url(base_url))
}

pub fn admin_rag_health_url(base_url: &str) -> String {
    format!("{}/api/v1/admin/rag-health", trim_base_url(base_url))
}

pub fn admin_workers_url(base_url: &str) -> String {
    format!("{}/api/v1/admin/system/workers", trim_base_url(base_url))
}

pub fn admin_degradation_url(base_url: &str) -> String {
    format!("{}/api/v1/admin/system/degradation", trim_base_url(base_url))
}

pub fn admin_feature_flags_url(base_url: &str) -> String {
    format!("{}/api/v1/admin/feature-flags", trim_base_url(base_url))
}

pub fn admin_feature_flag_change_requests_url(base_url: &str, status: Option<&str>) -> String {
    let mut url = format!(
        "{}/api/v1/admin/feature-flags/change-requests",
        trim_base_url(base_url)
    );
    if let Some(status) = status.filter(|s| !s.is_empty()) {
        url.push_str(&format!("?status={}", encode_path_segment(status)));
    }
    url
}

pub fn admin_feature_flag_change_request_create_url(base_url: &str, flag_key: &str) -> String {
    format!(
        "{}/api/v1/admin/feature-flags/{}/change-requests",
        trim_base_url(base_url),
        encode_path_segment(flag_key)
    )
}

pub fn admin_feature_flag_change_request_review_url(base_url: &str, request_id: &str) -> String {
    format!(
        "{}/api/v1/admin/feature-flags/change-requests/{}/review",
        trim_base_url(base_url),
        encode_path_segment(request_id)
    )
}

pub fn admin_audit_logs_url(base_url: &str, query: &AdminAuditLogQuery) -> String {
    format!(
        "{}/api/v1/admin/audit-logs{}",
        trim_base_url(base_url),
        audit_query_string(query)
    )
}

fn audit_query_string(query: &AdminAuditLogQuery) -> String {
    let mut params: Vec<String> = Vec::new();
    let mut push = |key: &str, value: Option<&String>| {
        if let Some(value) = value.filter(|v| !v.is_empty()) {
            params.push(format!("{}={}", key, encode_path_segment(value)));
        }
    };
    push("query", query.query.as_ref());
    push("action", query.action.as_ref());
    push("resource_type", query.resource_type.as_ref());
    push("actor", query.actor.as_ref());
    push("window", query.window.as_ref());
    if let Some(page) = query.page.filter(|p| *p > 0) {
        params.push(format!("page={page}"));
    }
    if let Some(per_page) = query.per_page.filter(|p| *p > 0) {
        params.push(format!("per_page={per_page}"));
    }
    if params.is_empty() {
        String::new()
    } else {
        format!("?{}", params.join("&"))
    }
}

pub fn admin_broadcast_url(base_url: &str) -> String {
    format!(
        "{}/api/v1/admin/notifications/broadcast",
        trim_base_url(base_url)
    )
}

pub fn parse_admin_accounts(body: &[u8]) -> Result<Vec<AdminAccountInfo>, TransportError> {
    parse_admin_data(body)
}

pub fn parse_admin_account(body: &[u8]) -> Result<AdminAccountInfo, TransportError> {
    parse_admin_data(body)
}

pub fn parse_admin_users(body: &[u8]) -> Result<Vec<AdminUserInfo>, TransportError> {
    parse_admin_data(body)
}

pub fn parse_admin_usage(body: &[u8]) -> Result<AdminUsageStats, TransportError> {
    parse_admin_data(body)
}

pub fn parse_admin_health(body: &[u8]) -> Result<AdminHealthStatus, TransportError> {
    parse_admin_data(body)
}

pub fn parse_admin_billing(body: &[u8]) -> Result<AdminBillingOverview, TransportError> {
    parse_admin_data(body)
}

pub fn parse_admin_rag_health(body: &[u8]) -> Result<AdminRagHealthStatus, TransportError> {
    parse_admin_data(body)
}

pub fn parse_admin_workers(body: &[u8]) -> Result<AdminWorkerStatus, TransportError> {
    parse_admin_data(body)
}

pub fn parse_admin_degradation(body: &[u8]) -> Result<AdminDegradationStatus, TransportError> {
    parse_admin_data(body)
}

pub fn parse_admin_feature_flags(
    body: &[u8],
) -> Result<Vec<AdminFeatureFlagEntry>, TransportError> {
    parse_admin_data(body)
}

pub fn parse_admin_feature_flag_change_requests(
    body: &[u8],
) -> Result<Vec<AdminFeatureFlagChangeRequest>, TransportError> {
    parse_admin_data(body)
}

pub fn parse_admin_feature_flag_change_request(
    body: &[u8],
) -> Result<AdminFeatureFlagChangeRequest, TransportError> {
    parse_admin_data(body)
}

pub fn parse_admin_audit_logs(body: &[u8]) -> Result<AdminAuditLogPage, TransportError> {
    parse_admin_data(body)
}

pub fn parse_admin_broadcast(body: &[u8]) -> Result<AdminBroadcastResult, TransportError> {
    parse_admin_data(body)
}
