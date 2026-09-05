use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSecretRow {
    pub id: String,
    pub purpose: String,
    pub provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_active: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSecretsResponse {
    pub secrets: Vec<ProviderSecretRow>,
}

pub fn provider_secrets_url(base_url: &str) -> String {
    let trimmed = crate::conversation_api::trim_base_url(base_url);
    format!("{trimmed}/api/v1/settings/provider-secrets")
}

pub fn parse_provider_secrets(
    body: &[u8],
) -> Result<ProviderSecretsResponse, crate::transport::TransportError> {
    serde_json::from_slice(body).map_err(crate::transport::TransportError::from)
}

pub fn has_quick_chat_byok(secrets: &[ProviderSecretRow]) -> bool {
    secrets.iter().any(|s| {
        s.purpose == "quick_chat" && s.revoked_at.is_none() && s.is_active != Some(false)
    })
}

pub fn model_role_label(model_role: &str, has_byok: bool) -> String {
    match model_role {
        "quick_chat" => {
            if has_byok {
                "对话 · qwen3.8-flash (自定义密钥)".to_string()
            } else {
                "对话 · qwen3.8-flash (默认)".to_string()
            }
        }
        "agent" => "工作区 Agent".to_string(),
        other => other.to_string(),
    }
}
