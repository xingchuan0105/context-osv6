use app_core::ShareStorePort;
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
        app_core::ShareAccessLevel::from_role(role).into()
    }

    pub fn as_db(&self) -> &'static str {
        app_core::ShareAccessLevel::from(*self).as_db()
    }

    pub fn allows_share_management(&self) -> bool {
        app_core::ShareAccessLevel::from(*self).allows_share_management()
    }

    pub fn as_permission_label(&self) -> &'static str {
        app_core::ShareAccessLevel::from(*self).as_permission_label()
    }

    pub fn as_invite_role(&self) -> &'static str {
        match self {
            Self::Admin => "owner",
            Self::Write => "editor",
            Self::Read | Self::None => "viewer",
        }
    }
}

impl From<app_core::ShareAccessLevel> for AccessLevel {
    fn from(value: app_core::ShareAccessLevel) -> Self {
        match value {
            app_core::ShareAccessLevel::None => Self::None,
            app_core::ShareAccessLevel::Read => Self::Read,
            app_core::ShareAccessLevel::Write => Self::Write,
            app_core::ShareAccessLevel::Admin => Self::Admin,
        }
    }
}

impl From<AccessLevel> for app_core::ShareAccessLevel {
    fn from(value: AccessLevel) -> Self {
        match value {
            AccessLevel::None => Self::None,
            AccessLevel::Read => Self::Read,
            AccessLevel::Write => Self::Write,
            AccessLevel::Admin => Self::Admin,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMember {
    pub id: String,
    pub workspace_id: String,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub access_level: AccessLevel,
    pub invite_status: String,
    pub invited_by: Option<String>,
    pub invited_at: i64,
    pub accepted_at: Option<i64>,
}

impl From<app_core::ShareWorkspaceMember> for WorkspaceMember {
    fn from(value: app_core::ShareWorkspaceMember) -> Self {
        Self {
            id: value.id,
            workspace_id: value.workspace_id,
            user_id: value.user_id,
            email: value.email,
            access_level: value.access_level.into(),
            invite_status: value.invite_status,
            invited_by: value.invited_by,
            invited_at: value.invited_at,
            accepted_at: value.accepted_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareSettings {
    pub access_level: String,
    pub allow_download: bool,
    pub share_tokens: Vec<ShareTokenInfo>,
    pub members: Vec<WorkspaceMember>,
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
pub struct SharedWorkspacePayload {
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
    pub workspace_id: String,
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
    pub workspace_id: uuid::Uuid,
    pub owner_user_id: uuid::Uuid,
    pub access_level: AccessLevel,
}

pub struct ShareService {
    pub(crate) store: Arc<dyn ShareStorePort>,
}

impl ShareService {
    pub fn new(store: Arc<dyn ShareStorePort>) -> Self {
        Self { store }
    }
}
