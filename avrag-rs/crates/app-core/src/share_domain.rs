use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShareAccessLevel {
    None,
    Read,
    Write,
    Admin,
}

impl ShareAccessLevel {
    pub fn from_role(role: &str) -> Self {
        match role {
            "viewer" | "read" | "partial" => Self::Read,
            "editor" | "write" | "full" => Self::Write,
            "admin" => Self::Admin,
            _ => Self::None,
        }
    }

    pub fn as_db(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Read => "read",
            Self::Write => "write",
            Self::Admin => "admin",
        }
    }

    pub fn allows_share_management(&self) -> bool {
        matches!(self, Self::Write | Self::Admin)
    }

    pub fn as_permission_label(&self) -> &'static str {
        match self {
            Self::Read | Self::None => "partial",
            Self::Write | Self::Admin => "full",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareNotebookMember {
    pub id: String,
    pub workspace_id: String,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub access_level: ShareAccessLevel,
    pub invite_status: String,
    pub invited_by: Option<String>,
    pub invited_at: i64,
    pub accepted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareSettingsSnapshot {
    pub access_level: String,
    pub allow_download: bool,
    pub share_tokens: Vec<ShareTokenSnapshot>,
    pub members: Vec<ShareNotebookMember>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareTokenSnapshot {
    pub token: String,
    pub access_level: String,
    pub expires_at: Option<String>,
    pub revoked_at: Option<String>,
    pub access_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedNotebookSnapshot {
    pub knowledge_base: SharedKnowledgeBaseSnapshot,
    pub share: SharedShareInfoSnapshot,
    pub sources: Vec<SharedSourceSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedKnowledgeBaseSnapshot {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedShareInfoSnapshot {
    pub permission: String,
    pub expires_at: Option<String>,
    pub allow_download: bool,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedSourceSnapshot {
    pub id: String,
    pub file_name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareAccessLogEntry {
    pub id: String,
    pub workspace_id: String,
    pub share_token: String,
    pub action: String,
    pub accessed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareAnalyticsEntry {
    pub token: String,
    pub access_level: String,
    pub total_views: i64,
    pub last_accessed_at: Option<i64>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PublicShareChatContextSnapshot {
    pub org_id: Uuid,
    pub workspace_id: Uuid,
    pub owner_user_id: Uuid,
    pub access_level: ShareAccessLevel,
}

#[derive(Debug, Clone)]
pub struct NotebookAccessSnapshot {
    pub owner_id: Option<Uuid>,
    pub notebook_access_level: String,
}
