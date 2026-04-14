use avrag_storage_pg::PgAppRepository;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessLevel {
    None,
    Read,
    Write,
    Admin,
}

impl AccessLevel {
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

    pub fn as_invite_role(&self) -> &'static str {
        match self {
            Self::Admin => "owner",
            Self::Write => "editor",
            Self::Read | Self::None => "viewer",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookMember {
    pub id: String,
    pub notebook_id: String,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub access_level: AccessLevel,
    pub invite_status: String,
    pub invited_by: Option<String>,
    pub invited_at: i64,
    pub accepted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareSettings {
    pub access_level: String,
    pub allow_download: bool,
    pub share_tokens: Vec<ShareTokenInfo>,
    pub members: Vec<NotebookMember>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareTokenInfo {
    pub token: String,
    pub access_level: String,
    pub expires_at: Option<String>,
    pub revoked_at: Option<String>,
    pub access_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedNotebookPayload {
    pub knowledge_base: SharedKnowledgeBase,
    pub share: SharedShareInfo,
    pub sources: Vec<SharedSource>,
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
pub struct ShareAccessLog {
    pub id: String,
    pub notebook_id: String,
    pub share_token: String,
    pub action: String,
    pub accessed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareAnalytics {
    pub token: String,
    pub access_level: String,
    pub total_views: i64,
    pub last_accessed_at: Option<i64>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PublicShareChatContext {
    pub org_id: uuid::Uuid,
    pub notebook_id: uuid::Uuid,
    pub owner_user_id: uuid::Uuid,
    pub access_level: AccessLevel,
}

pub struct ShareService {
    pub(crate) repo: Arc<PgAppRepository>,
}
