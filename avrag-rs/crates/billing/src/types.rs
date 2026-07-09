use hmac::Hmac;
use sha2::Sha256;

pub(crate) type HmacSha256 = Hmac<Sha256>;

pub use app_core::billing_domain::*;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreateUsageExportRequest {
    pub from: String,
    pub to: String,
    #[serde(default = "default_export_format")]
    pub format: String,
}

fn default_export_format() -> String {
    "csv".to_string()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UsageExportAccepted {
    pub export_id: String,
    pub status: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UsageExportStatusResponse {
    pub export_id: String,
    pub status: String,
    pub format: String,
    pub from: String,
    pub to: String,
    pub row_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}
