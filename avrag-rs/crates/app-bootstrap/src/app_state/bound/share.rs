//! Bound face — share.

use app_core::{ShareStorePort, StorageContext};
use contracts::auth_runtime::AuthContext;
use std::sync::Arc;
use uuid::Uuid;


pub struct BoundShare<'a> {
    pub(crate) auth: &'a AuthContext,
    pub(crate) storage: &'a StorageContext,
    pub(crate) docs: &'a app_documents::DocumentContext,
}

impl<'a> BoundShare<'a> {
    fn require_store(&self) -> Result<Arc<dyn ShareStorePort>, common::AppError> {
        self.storage
            .share_store()
            .ok_or_else(|| common::AppError::internal("postgres backend is not configured"))
    }

    /// Lightweight share-token mint (legacy helper used by product tests / admin UI).
    pub async fn create_share_token(
        &self,
        workspace_id: &str,
    ) -> Result<common::ShareTokenResponse, common::AppError> {
        self.docs
            .get_notebook(self.auth, self.storage, workspace_id)
            .await
            .ok_or_else(|| {
                common::AppError::not_found("notebook_not_found", "notebook not found")
            })?;
        Ok(common::ShareTokenResponse {
            share_token: format!("share_{}", common::new_id()),
        })
    }

    pub async fn create_share_link(
        &self,
        workspace_id: String,
        access_level: avrag_share::AccessLevel,
        expires_in_secs: Option<i64>,
    ) -> Result<common::ShareTokenResponse, common::AppError> {
        let store = self.require_store()?;
        avrag_share::handle_create_share_link(
            self.auth.clone(),
            workspace_id,
            access_level,
            expires_in_secs,
            store,
        )
        .await
    }

    pub async fn revoke_share_link(&self, token: String) -> Result<(), common::AppError> {
        let store = self.require_store()?;
        avrag_share::handle_revoke_share_link(self.auth.clone(), token, store).await
    }

    pub async fn get_share_settings(
        &self,
        workspace_id: String,
    ) -> Result<avrag_share::ShareSettings, common::AppError> {
        let store = self.require_store()?;
        avrag_share::handle_get_share_settings(self.auth.clone(), workspace_id, store).await
    }

    pub async fn update_share_settings(
        &self,
        workspace_id: String,
        access_level: Option<String>,
        allow_download: Option<bool>,
    ) -> Result<avrag_share::ShareSettings, common::AppError> {
        let store = self.require_store()?;
        avrag_share::handle_update_share_settings(
            self.auth.clone(),
            workspace_id,
            access_level,
            allow_download,
            store,
        )
        .await
    }

    pub async fn update_share_access_level(
        &self,
        workspace_id: String,
        access_level: String,
    ) -> Result<String, common::AppError> {
        let store = self.require_store()?;
        avrag_share::handle_update_access_level(
            self.auth.clone(),
            workspace_id,
            access_level,
            store,
        )
        .await
    }

    pub async fn get_share_analytics(
        &self,
        workspace_id: String,
    ) -> Result<Vec<avrag_share::ShareAnalytics>, common::AppError> {
        let store = self.require_store()?;
        avrag_share::handle_get_share_analytics(self.auth.clone(), workspace_id, store).await
    }

    pub async fn get_share_access_logs(
        &self,
        workspace_id: String,
    ) -> Result<Vec<avrag_share::ShareAccessLog>, common::AppError> {
        let store = self.require_store()?;
        avrag_share::handle_get_share_access_logs(self.auth.clone(), workspace_id, None, store)
            .await
    }

    pub async fn validate_share_token(
        &self,
        token: &str,
    ) -> Result<Option<String>, common::AppError> {
        let store = self.require_store()?;
        avrag_share::handle_validate_token(token, store).await
    }

    pub async fn list_share_members(
        &self,
        workspace_id: String,
    ) -> Result<Vec<avrag_share::NotebookMember>, common::AppError> {
        let store = self.require_store()?;
        avrag_share::handle_list_members(self.auth.clone(), workspace_id, store).await
    }

    pub async fn invite_share_member(
        &self,
        workspace_id: String,
        email: String,
        role: avrag_share::AccessLevel,
    ) -> Result<(), common::AppError> {
        let store = self.require_store()?;
        avrag_share::handle_invite_member(self.auth.clone(), workspace_id, email, role, store)
            .await
            .map(|_| ())
    }

    pub async fn accept_share_invite(
        &self,
        workspace_id: String,
        member_id: String,
    ) -> Result<(), common::AppError> {
        let store = self.require_store()?;
        avrag_share::handle_accept_invite(self.auth.clone(), workspace_id, member_id, store).await
    }

    pub async fn decline_share_invite(
        &self,
        workspace_id: String,
        member_id: String,
    ) -> Result<(), common::AppError> {
        let store = self.require_store()?;
        avrag_share::handle_decline_invite(self.auth.clone(), workspace_id, member_id, store).await
    }

    pub async fn remove_share_member(
        &self,
        workspace_id: String,
        member_id: String,
    ) -> Result<(), common::AppError> {
        let store = self.require_store()?;
        avrag_share::handle_remove_member(self.auth.clone(), workspace_id, member_id, store).await
    }

    pub async fn get_shared_notebook(
        &self,
        token: &str,
    ) -> Result<Option<avrag_share::SharedNotebookPayload>, common::AppError> {
        let store = self.require_store()?;
        avrag_share::handle_get_shared_notebook(token, store).await
    }

    pub async fn share_member_count(&self, workspace_id: &str) -> i64 {
        let Some(store) = self.storage.share_store() else {
            return 0;
        };
        avrag_share::handle_list_members(self.auth.clone(), workspace_id.to_string(), store)
            .await
            .map(|members| members.len() as i64)
            .unwrap_or(0)
    }

    pub async fn share_enabled_for_notebook(&self, workspace_id: &str) -> bool {
        let Some(store) = self.storage.share_store() else {
            return false;
        };
        avrag_share::handle_get_share_settings(self.auth.clone(), workspace_id.to_string(), store)
            .await
            .map(|settings| {
                settings
                    .share_tokens
                    .iter()
                    .any(|token| token.revoked_at.is_none() && !token.token.trim().is_empty())
                    && !settings.access_level.eq_ignore_ascii_case("private")
            })
            .unwrap_or(false)
    }

    pub async fn resolve_share_chat_notebook_scope(&self, token: &str) -> Option<Uuid> {
        let store = self.storage.share_store()?;
        let workspace_id = avrag_share::handle_validate_token(token, store)
            .await
            .ok()??;
        Uuid::parse_str(&workspace_id).ok()
    }
}

