use std::collections::HashMap;
use std::sync::Arc;

use app_core::{
    NotebookAccessSnapshot, PublicShareChatContextSnapshot, ShareAccessLevel, ShareAccessLogEntry,
    ShareAnalyticsEntry, ShareNotebookMember, ShareStorePort, SharedNotebookSnapshot,
};
use async_trait::async_trait;
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
    notebooks: Arc<RwLock<HashMap<Uuid, NotebookAccessSnapshot>>>,
    member_access: Arc<RwLock<HashMap<(Uuid, Uuid), String>>>,
    tokens: Arc<RwLock<HashMap<String, TokenRecord>>>,
    shared_notebooks: Arc<RwLock<HashMap<String, SharedNotebookSnapshot>>>,
    public_chat_contexts: Arc<RwLock<HashMap<String, PublicShareChatContextSnapshot>>>,
    invites: Arc<RwLock<Vec<ShareNotebookMember>>>,
}

#[allow(dead_code)]
impl MemoryShareStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn seed_notebook_owner(&self, workspace_id: Uuid, owner_id: Uuid) {
        self.notebooks.write().await.insert(
            workspace_id,
            NotebookAccessSnapshot {
                owner_id: Some(owner_id),
                notebook_access_level: "private".to_string(),
            },
        );
    }

    pub async fn seed_member_access(&self, workspace_id: Uuid, user_id: Uuid, role: &str) {
        self.member_access
            .write()
            .await
            .insert((workspace_id, user_id), role.to_string());
    }

    pub async fn seed_shared_notebook(&self, token: &str, snapshot: SharedNotebookSnapshot) {
        self.shared_notebooks
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

    pub async fn invited_members(&self) -> Vec<ShareNotebookMember> {
        self.invites.read().await.clone()
    }
}

#[async_trait]
impl ShareStorePort for MemoryShareStore {
    async fn query_notebook_access(
        &self,
        _auth: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<Option<NotebookAccessSnapshot>, AppError> {
        Ok(self.notebooks.read().await.get(&workspace_id).cloned())
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
        _workspace_id: Uuid,
    ) -> Result<(String, bool, Vec<app_core::ShareTokenSnapshot>), AppError> {
        Ok(("private".to_string(), false, Vec::new()))
    }

    async fn list_members(
        &self,
        _auth: &AuthContext,
        _workspace_id: Uuid,
    ) -> Result<Vec<ShareNotebookMember>, AppError> {
        Ok(Vec::new())
    }

    async fn update_notebook_access_level(
        &self,
        _auth: &AuthContext,
        _workspace_id: Uuid,
        _access_level: &str,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn update_share_settings(
        &self,
        _auth: &AuthContext,
        _workspace_id: Uuid,
        _access_level: Option<&str>,
        _allow_download: Option<bool>,
    ) -> Result<(), AppError> {
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

    async fn load_shared_notebook(
        &self,
        token: &str,
    ) -> Result<Option<SharedNotebookSnapshot>, AppError> {
        Ok(self.shared_notebooks.read().await.get(token).cloned())
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
    ) -> Result<ShareNotebookMember, AppError> {
        let member = ShareNotebookMember {
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
}
