//! License domain types and pure helpers.
use serde::{Deserialize, Serialize};

pub(crate) const LICENSE_FILENAME: &str = "license.json";
pub(crate) const DEVICE_SALT: &str = "avrag-desktop-salt";
pub(crate) const TRIAL_DURATION_SECS: i64 = 7 * 24 * 60 * 60;
pub(crate) const OFFLINE_GRACE_SECS: i64 = 30 * 24 * 60 * 60;
pub(crate) const HEARTBEAT_INTERVAL_SECS: i64 = 24 * 60 * 60;

pub(crate) const KEYGEN_PUBLIC_KEY: Option<&str> = option_env!("KEYGEN_PUBLIC_KEY");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseKind {
    Trial,
    Standard,
    Pro,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseStatusKind {
    Unactivated,
    Trial,
    Active,
    Expired,
    Revoked,
    OfflineGrace,
    UpgradeRequired,
}

pub fn license_allows_chat(kind: LicenseStatusKind) -> bool {
    matches!(
        kind,
        LicenseStatusKind::Active
            | LicenseStatusKind::Trial
            | LicenseStatusKind::OfflineGrace
            | LicenseStatusKind::UpgradeRequired
    )
}

/// Domain license failure. Converts to [`crate::commands::api::IpcApiError`] at
/// the Tauri command boundary so IPC always uses one error shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseError {
    pub code: String,
    pub message: String,
}

impl LicenseError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl From<String> for LicenseError {
    fn from(message: String) -> Self {
        Self::new("internal_error", message)
    }
}

impl From<LicenseError> for crate::commands::api::IpcApiError {
    fn from(err: LicenseError) -> Self {
        let status = match err.code.as_str() {
            "trial_already_used" | "device_id" | "app_data_dir" | "license_file" => 400,
            "public_key_missing" => 500,
            _ => 400,
        };
        crate::commands::api::IpcApiError::new(status, err.code, err.message)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateClaims {
    pub device_id: String,
    pub expires_at: Option<i64>,
    pub major_version_included: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseFile {
    pub key: String,
    pub license_id: String,
    pub device_id: String,
    #[serde(default)]
    pub machine_id: Option<String>,
    pub certificate: String,
    pub kind: LicenseKind,
    pub issued_at: i64,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub last_heartbeat: Option<i64>,
    #[serde(default)]
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialResult {
    pub expires_at: i64,
    pub days_remaining: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivationResult {
    pub license_id: String,
    pub kind: LicenseKind,
    pub status: LicenseStatusKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatResult {
    pub success: bool,
    pub status: LicenseStatusKind,
    pub next_heartbeat_at: Option<i64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseStatus {
    pub kind: LicenseStatusKind,
    pub days_remaining: Option<i32>,
    pub offline_grace_days: Option<i32>,
    pub license_kind: Option<LicenseKind>,
    pub expires_at: Option<i64>,
    pub dev_mode: bool,
}

