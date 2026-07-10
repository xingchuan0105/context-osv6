use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputGuardType {
    PromptInjection,
    PrivilegeEscalation,
    ScopeViolation,
}

impl std::fmt::Display for InputGuardType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InputGuardType::PromptInjection => write!(f, "input:prompt_injection"),
            InputGuardType::PrivilegeEscalation => write!(f, "input:privilege_escalation"),
            InputGuardType::ScopeViolation => write!(f, "input:scope_violation"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputGuardType {
    CitationProvability,
    PIIDetection,
    HarmfulContent,
}

impl std::fmt::Display for OutputGuardType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputGuardType::CitationProvability => write!(f, "output:citation_provability"),
            OutputGuardType::PIIDetection => write!(f, "output:pii_detection"),
            OutputGuardType::HarmfulContent => write!(f, "output:harmful_content"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnswerContextChunk {
    pub chunk_id: String,
    #[serde(default)]
    pub doc_id: Option<String>,
    pub chunk_type: String,
    #[serde(default)]
    pub page: Option<i64>,
    pub text: String,
    #[serde(default)]
    pub asset_id: Option<String>,
    #[serde(default)]
    pub caption: Option<String>,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub parser_backend: Option<String>,
    #[serde(default)]
    pub source_locator: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyRow {
    pub id: String,
    pub owner_user_id: String,
    pub workspace_id: String,
    pub key_prefix: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub rate_limit_rpm: u32,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub last_used_at: Option<String>,
    pub is_active: bool,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyListResponse {
    pub api_keys: Vec<ApiKeyRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub rate_limit_rpm: Option<u32>,
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateApiKeyResponse {
    pub api_key: ApiKeyRow,
    pub plaintext_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationRow {
    pub id: String,
    pub owner_user_id: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsResponse {
    pub notifications: Vec<NotificationRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareTokenResponse {
    pub share_token: String,
}
