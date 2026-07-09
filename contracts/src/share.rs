use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareTokenResponse {
    pub share_token: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareSettings {
    pub share_token: String,
    pub access_level: String,
    pub expires_at: Option<String>,
    pub allow_download: bool,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedKnowledgeBase {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
}

#[typeshare]
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

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedSource {
    pub id: String,
    pub file_name: String,
    pub status: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedWorkspacePayload {
    pub knowledge_base: SharedKnowledgeBase,
    pub share: SharedShareInfo,
    pub sources: Vec<SharedSource>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareAnalyticsResponse {
    #[typeshare(serialized_as = "number")]
    pub total_views: i64,
    #[typeshare(serialized_as = "number")]
    pub total_unique_visitors: i64,
    #[typeshare(serialized_as = "Record<string, number>")]
    pub views_by_day: std::collections::BTreeMap<String, i64>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessLogEntry {
    pub id: String,
    pub visitor_id: String,
    pub accessed_at: String,
    pub action: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessLogsResponse {
    pub logs: Vec<AccessLogEntry>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberRow {
    pub member_id: String,
    pub user_id: String,
    pub email: String,
    pub role: String,
    pub status: String,
    pub invited_at: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembersResponse {
    pub members: Vec<MemberRow>,
}
