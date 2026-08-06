use std::collections::HashMap;
use std::sync::Arc;

use app_core::{
    WorkspaceAccessSnapshot, PublicShareChatContextSnapshot, ShareAccessLevel, ShareAccessLogEntry,
    ShareAnalyticsEntry, ShareWorkspaceMember, ShareStorePort, SharedWorkspaceSnapshot,
};
use async_trait::async_trait;
use avrag_share::max_shared_workspaces_for_plan;
use contracts::auth_runtime::AuthContext;
use chrono::{DateTime, Utc};
use common::AppError;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone)]
struct TokenRecord {
    workspace_id: Uuid,
    access_level: ShareAccessLevel,
    expires_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Default)]
pub struct MemoryShareStore {
    workspaces: Arc<RwLock<HashMap<Uuid, WorkspaceAccessSnapshot>>>,
    member_access: Arc<RwLock<HashMap<(Uuid, Uuid), String>>>,
    tokens: Arc<RwLock<HashMap<String, TokenRecord>>>,
    shared_workspaces: Arc<RwLock<HashMap<String, SharedWorkspaceSnapshot>>>,
    public_chat_contexts: Arc<RwLock<HashMap<String, PublicShareChatContextSnapshot>>>,
    invites: Arc<RwLock<Vec<ShareWorkspaceMember>>>,
    /// owner_user_id → plan_id for quota tests (default free).
    owner_plans: Arc<RwLock<HashMap<Uuid, String>>>,
}

#[allow(dead_code)]
impl MemoryShareStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn seed_workspace_owner(&self, workspace_id: Uuid, owner_id: Uuid) {
        self.workspaces.write().await.insert(
            workspace_id,
            WorkspaceAccessSnapshot {
                owner_id: Some(owner_id),
                notebook_access_level: "private".to_string(),
                share_enabled: false,
            },
        );
    }

    pub async fn seed_member_access(&self, workspace_id: Uuid, user_id: Uuid, role: &str) {
        self.member_access
            .write()
            .await
            .insert((workspace_id, user_id), role.to_string());
    }

    pub async fn seed_owner_plan(&self, owner_id: Uuid, plan_id: &str) {
        self.owner_plans
            .write()
            .await
            .insert(owner_id, plan_id.to_string());
    }

    pub async fn seed_shared_workspace(&self, token: &str, snapshot: SharedWorkspaceSnapshot) {
        self.shared_workspaces
            .write()
            .await
            .insert(token.to_string(), snapshot);
    }

    pub async fn seed_public_chat_context(
        &self,
        token: &str,
        snapshot: PublicShareChatContextSnapshot,
    ) {
        self.public_chat_contexts
            .write()
            .await
            .insert(token.to_string(), snapshot);
    }

    pub async fn invited_members(&self) -> Vec<ShareWorkspaceMember> {
        self.invites.read().await.clone()
    }

    pub async fn is_share_enabled(&self, workspace_id: Uuid) -> bool {
        self.workspaces
            .read()
            .await
            .get(&workspace_id)
            .map(|s| s.share_enabled)
            .unwrap_or(false)
    }

    pub async fn count_enabled_for_owner(&self, owner_id: Uuid) -> i64 {
        self.workspaces
            .read()
            .await
            .values()
            .filter(|s| s.owner_id == Some(owner_id) && s.share_enabled)
            .count() as i64
    }
}

#[async_trait]
impl ShareStorePort for MemoryShareStore {
    async fn query_workspace_access(
        &self,
        _auth: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<Option<WorkspaceAccessSnapshot>, AppError> {
        Ok(self.workspaces.read().await.get(&workspace_id).cloned())
    }

    async fn query_member_access(
        &self,
        _auth: &AuthContext,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<String>, AppError> {
        Ok(self
            .member_access
            .read()
            .await
            .get(&(workspace_id, user_id))
            .cloned())
    }

    async fn get_share_settings(
        &self,
        _auth: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<app_core::WorkspaceShareSettingsRow, AppError> {
        let access_level = self
            .workspaces
            .read()
            .await
            .get(&workspace_id)
            .map(|s| s.notebook_access_level.clone())
            .unwrap_or_else(|| "private".to_string());
        Ok(app_core::WorkspaceShareSettingsRow {
            access_level,
            allow_download: false,
            anon_question_limit: 10,
            member_question_limit: None,
            tokens: Vec::new(),
        })
    }

    async fn list_members(
        &self,
        _auth: &AuthContext,
        _workspace_id: Uuid,
    ) -> Result<Vec<ShareWorkspaceMember>, AppError> {
        Ok(Vec::new())
    }

    async fn update_workspace_access_level(
        &self,
        _auth: &AuthContext,
        workspace_id: Uuid,
        access_level: &str,
    ) -> Result<(), AppError> {
        if let Some(snapshot) = self.workspaces.write().await.get_mut(&workspace_id) {
            snapshot.notebook_access_level = access_level.to_string();
        }
        Ok(())
    }

    async fn update_share_settings(
        &self,
        _auth: &AuthContext,
        workspace_id: Uuid,
        access_level: Option<&str>,
        _allow_download: Option<bool>,
        _anon_question_limit: Option<i32>,
        _member_question_limit: Option<Option<i32>>,
    ) -> Result<(), AppError> {
        if let Some(level) = access_level {
            if let Some(snapshot) = self.workspaces.write().await.get_mut(&workspace_id) {
                snapshot.notebook_access_level = level.to_string();
            }
        }
        Ok(())
    }

    async fn create_share_token(
        &self,
        _auth: &AuthContext,
        workspace_id: Uuid,
        access_level: ShareAccessLevel,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<String, AppError> {
        let token = Uuid::new_v4().to_string();
        self.tokens.write().await.insert(
            token.clone(),
            TokenRecord {
                workspace_id,
                access_level,
                expires_at,
                revoked_at: None,
            },
        );
        Ok(token)
    }

    async fn validate_token(
        &self,
        token: &str,
    ) -> Result<Option<(Uuid, ShareAccessLevel)>, AppError> {
        let tokens = self.tokens.read().await;
        let Some(record) = tokens.get(token) else {
            return Ok(None);
        };
        if record.revoked_at.is_some() {
            return Ok(None);
        }
        if let Some(expires_at) = record.expires_at {
            if expires_at <= Utc::now() {
                return Ok(None);
            }
        }
        Ok(Some((record.workspace_id, record.access_level)))
    }

    async fn revoke_token(
        &self,
        _auth: &AuthContext,
        token: &str,
    ) -> Result<Option<Uuid>, AppError> {
        let mut tokens = self.tokens.write().await;
        if let Some(record) = tokens.get_mut(token) {
            record.revoked_at = Some(Utc::now());
            return Ok(Some(record.workspace_id));
        }
        Ok(None)
    }

    async fn get_share_analytics(
        &self,
        _auth: &AuthContext,
        _workspace_id: Uuid,
    ) -> Result<Vec<ShareAnalyticsEntry>, AppError> {
        Ok(Vec::new())
    }

    async fn get_share_access_logs(
        &self,
        _auth: &AuthContext,
        _workspace_id: Uuid,
        _limit: usize,
    ) -> Result<Vec<ShareAccessLogEntry>, AppError> {
        Ok(Vec::new())
    }

    async fn load_shared_workspace(
        &self,
        token: &str,
    ) -> Result<Option<SharedWorkspaceSnapshot>, AppError> {
        Ok(self.shared_workspaces.read().await.get(token).cloned())
    }

    async fn resolve_public_share_chat_context(
        &self,
        token: &str,
    ) -> Result<Option<PublicShareChatContextSnapshot>, AppError> {
        Ok(self.public_chat_contexts.read().await.get(token).cloned())
    }

    async fn invite_member(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        email: &str,
        access_level: ShareAccessLevel,
    ) -> Result<ShareWorkspaceMember, AppError> {
        let member = ShareWorkspaceMember {
            id: Uuid::new_v4().to_string(),
            workspace_id: workspace_id.to_string(),
            user_id: None,
            email: Some(email.to_string()),
            access_level,
            invite_status: "pending".to_string(),
            invited_by: auth.actor_id().map(|actor| actor.into_uuid().to_string()),
            invited_at: Utc::now().timestamp(),
            accepted_at: None,
        };
        self.invites.write().await.push(member.clone());
        Ok(member)
    }

    async fn accept_invite(
        &self,
        _auth: &AuthContext,
        _workspace_id: Uuid,
        _member_id: Uuid,
        _actor_id: Uuid,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn decline_invite(
        &self,
        _auth: &AuthContext,
        _workspace_id: Uuid,
        _member_id: Uuid,
        _actor_id: Uuid,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn add_member(
        &self,
        _auth: &AuthContext,
        _workspace_id: Uuid,
        _user_id: Uuid,
        _access_level: ShareAccessLevel,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn remove_member(
        &self,
        _auth: &AuthContext,
        _workspace_id: Uuid,
        _member_id: Uuid,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn record_share_product_event(
        &self,
        _event: analytics::ProductEvent,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn count_share_enabled_workspaces(
        &self,
        _auth: &AuthContext,
        owner_user_id: Uuid,
    ) -> Result<i64, AppError> {
        Ok(self.count_enabled_for_owner(owner_user_id).await)
    }

    async fn max_shared_workspaces_for_owner(
        &self,
        auth: &AuthContext,
        owner_user_id: Uuid,
    ) -> Result<i32, AppError> {
        let plan = self.owner_plan_id(auth, owner_user_id).await?;
        Ok(max_shared_workspaces_for_plan(&plan))
    }

    async fn owner_plan_id(
        &self,
        _auth: &AuthContext,
        owner_user_id: Uuid,
    ) -> Result<String, AppError> {
        Ok(self
            .owner_plans
            .read()
            .await
            .get(&owner_user_id)
            .cloned()
            .unwrap_or_else(|| "free".to_string()))
    }

    async fn set_share_enabled(
        &self,
        _auth: &AuthContext,
        workspace_id: Uuid,
        enabled: bool,
    ) -> Result<(), AppError> {
        if let Some(snapshot) = self.workspaces.write().await.get_mut(&workspace_id) {
            snapshot.share_enabled = enabled;
        }
        Ok(())
    }
}
