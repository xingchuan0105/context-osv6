use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AuthUserDto {
    pub id: String,
    pub email: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub full_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AuthPayload {
    pub token: String,
    pub user: AuthUserDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_ticket: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AuthEnvelope {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<AuthPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RegisterRequest {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub full_name: Option<String>,
    /// 用户服务协议版本号（注册必填；缺省或空字符串返回 `consent_required`）
    pub terms_version: Option<String>,
    /// 隐私政策版本号（注册必填；缺省或空字符串返回 `consent_required`）
    pub privacy_version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RecordLegalAcceptanceRequest {
    pub terms_version: String,
    pub privacy_version: String,
    /// `payment` or `re_acceptance` (registration uses `/register`)
    pub context: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LegalStatusPayload {
    pub needs_re_acceptance: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_terms_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accepted_privacy_version: Option<String>,
    pub published_terms_version: String,
    pub published_privacy_version: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LegalStatusEnvelope {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<LegalStatusPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct UpdateProfileRequest {
    #[serde(default)]
    pub full_name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct WorkspaceDraftPreference {
    #[serde(default)]
    pub workspace_id: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct WorkspacePreference {
    #[serde(default)]
    pub workspace_id: String,
    #[serde(default)]
    pub pinned_source_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct WorkspaceNotePreference {
    #[serde(default)]
    pub note_id: String,
    #[serde(default)]
    pub workspace_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub promoted_document_id: Option<String>,
    #[serde(default)]
    pub promoted_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct DashboardPreferences {
    #[serde(default)]
    pub favorite_workspace_ids: Vec<String>,
    #[serde(default)]
    pub workspace_drafts: Vec<WorkspaceDraftPreference>,
    #[serde(default)]
    pub workspace_preferences: Vec<WorkspacePreference>,
    #[serde(default)]
    pub workspace_notes: Vec<WorkspaceNotePreference>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NotificationPreferences {
    #[serde(default = "bool_true")]
    pub email_enabled: bool,
    #[serde(default = "bool_true")]
    pub product_enabled: bool,
    #[serde(default = "bool_true")]
    pub security_enabled: bool,
    #[serde(default)]
    pub weekly_digest_enabled: bool,
    #[serde(default)]
    pub quiet_hours_start: Option<String>,
    #[serde(default)]
    pub quiet_hours_end: Option<String>,
}

impl Default for NotificationPreferences {
    fn default() -> Self {
        Self {
            email_enabled: true,
            product_enabled: true,
            security_enabled: true,
            weekly_digest_enabled: false,
            quiet_hours_start: None,
            quiet_hours_end: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct UserPreferencesPayload {
    #[serde(default)]
    pub dashboard: DashboardPreferences,
    #[serde(default)]
    pub notifications: NotificationPreferences,
    #[serde(default)]
    pub agent_memory: contracts::preferences::AgentPreferenceMemory,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct UpdateShareSettingsRequest {
    #[serde(default)]
    pub access_level: Option<String>,
    #[serde(default)]
    pub allow_download: Option<bool>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct ChangePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct ResetRequest {
    pub email: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct VerifyResetTokenRequest {
    pub token: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct ResetPasswordRequest {
    pub token: String,
    pub new_password: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct SendResetCodeRequest {
    pub email: String,
    #[serde(default)]
    pub lang: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct VerifyResetCodeRequest {
    pub email: String,
    pub code: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct ConfirmResetPasswordRequest {
    pub reset_ticket: String,
    pub new_password: String,
}

fn bool_true() -> bool {
    true
}
