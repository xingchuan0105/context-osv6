use serde::{Deserialize, Serialize};
use typeshare::typeshare;

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceDraftPreference {
    pub notebook_id: String,
    pub notes: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct NotebookWorkspacePreference {
    pub notebook_id: String,
    #[serde(default)]
    pub pinned_source_ids: Vec<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotebookNotePreference {
    pub note_id: String,
    pub notebook_id: String,
    pub title: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub promoted_document_id: Option<String>,
    #[serde(default)]
    pub promoted_at: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DashboardPreferences {
    #[serde(default)]
    pub favorite_notebook_ids: Vec<String>,
    #[serde(default)]
    pub workspace_drafts: Vec<WorkspaceDraftPreference>,
    #[serde(default)]
    pub workspace_preferences: Vec<NotebookWorkspacePreference>,
    #[serde(default)]
    pub notebook_notes: Vec<NotebookNotePreference>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationPreferences {
    #[serde(default = "default_true")]
    pub email_enabled: bool,
    #[serde(default = "default_true")]
    pub product_enabled: bool,
    #[serde(default = "default_true")]
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

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AgentPreference {
    pub id: String,
    pub text: String,
    pub category: String,
    pub scope: String,
    pub confidence: String,
    pub source: String,
    pub updated_at: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct BlockedAgentPreference {
    pub id: String,
    pub text: String,
    pub blocked_at: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DailyPreferenceLog {
    pub date: String,
    #[serde(default)]
    pub added: Vec<String>,
    #[serde(default)]
    pub no_change: Vec<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AgentPreferenceMemory {
    #[serde(default)]
    pub active: Vec<AgentPreference>,
    #[serde(default)]
    pub superseded: Vec<AgentPreference>,
    #[serde(default)]
    pub blocked: Vec<BlockedAgentPreference>,
    #[serde(default)]
    pub daily_log: Vec<DailyPreferenceLog>,
    #[serde(default)]
    pub last_consolidated_at: Option<String>,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct UserPreferences {
    #[serde(default)]
    pub dashboard: DashboardPreferences,
    #[serde(default)]
    pub notifications: NotificationPreferences,
    #[serde(default)]
    pub agent_memory: AgentPreferenceMemory,
}

fn default_true() -> bool {
    true
}
