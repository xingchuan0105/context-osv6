use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareTokenResponse {
    pub share_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareSettings {
    pub share_token: String,
    pub access_level: String,
    pub expires_at: Option<String>,
    pub allow_download: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedKnowledgeBase {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedShareInfo {
    pub permission: String,
    pub expires_at: Option<String>,
    #[serde(default)]
    pub allow_download: bool,
    #[serde(default = "default_scope")]
    pub scope: String,
}

fn default_scope() -> String {
    "full".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedSource {
    pub id: String,
    pub file_name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedNotebookPayload {
    pub knowledge_base: SharedKnowledgeBase,
    pub share: SharedShareInfo,
    pub sources: Vec<SharedSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareAnalyticsResponse {
    pub total_views: i64,
    pub total_unique_visitors: i64,
    pub views_by_day: std::collections::BTreeMap<String, i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessLogEntry {
    pub id: String,
    pub visitor_id: String,
    pub accessed_at: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessLogsResponse {
    pub logs: Vec<AccessLogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberRow {
    pub member_id: String,
    pub user_id: String,
    pub email: String,
    pub role: String,
    pub status: String,
    pub invited_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembersResponse {
    pub members: Vec<MemberRow>,
}
