use crate::conversation_api::trim_base_url;
use crate::transport::TransportError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DashboardPreferences {
    #[serde(default)]
    pub favorite_workspace_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct NotificationPreferences {
    #[serde(default)]
    pub email_enabled: bool,
    #[serde(default)]
    pub product_enabled: bool,
    #[serde(default)]
    pub security_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct UserPreferences {
    #[serde(default)]
    pub dashboard: DashboardPreferences,
    #[serde(default)]
    pub notifications: NotificationPreferences,
}

pub fn preferences_url(base_url: &str) -> String {
    format!("{}/api/auth/preferences", trim_base_url(base_url))
}

pub fn parse_preferences(body: &[u8]) -> Result<UserPreferences, TransportError> {
    serde_json::from_slice(body).map_err(TransportError::from)
}
