use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUserDto {
    pub id: String,
    pub email: String,
    #[serde(default)]
    pub full_name: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthPayload {
    pub token: String,
    pub user: AuthUserDto,
    pub reset_ticket: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthEnvelope {
    pub success: bool,
    pub data: Option<AuthPayload>,
    pub error: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub full_name: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendResetCodeRequest {
    pub email: String,
    pub lang: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResetCodeRequest {
    pub email: String,
    pub code: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmResetPasswordRequest {
    pub reset_ticket: String,
    pub new_password: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRuntimeCapabilitiesResponse {
    pub password_reset_enabled: bool,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmptyResponse {}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationRow {
    pub id: String,
    pub org_id: String,
    pub user_id: String,
    pub event_type: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub data: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub read_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsResponse {
    pub notifications: Vec<NotificationRow>,
}
