use avrag_share::{
    AccessLevel, ShareAccessLog, ShareAnalytics, ShareSettings,
    SharedNotebookPayload,
};
use common::{AppError, ShareTokenResponse};
use uuid::Uuid;

use super::AppState;

impl AppState {
    pub async fn create_share_link(
        &self,
        notebook_id: String,
        access_level: AccessLevel,
        expires_in_secs: Option<i64>,
    ) -> Result<ShareTokenResponse, AppError> {
        let store = self
            .share_store()
            .ok_or_else(|| AppError::internal("postgres backend is not configured"))?;
        avrag_share::handle_create_share_link(
            self.auth().clone(),
            notebook_id,
            access_level,
            expires_in_secs,
            store,
        )
        .await
    }

    pub async fn revoke_share_link(&self, token: String) -> Result<(), AppError> {
        let store = self
            .share_store()
            .ok_or_else(|| AppError::internal("postgres backend is not configured"))?;
        avrag_share::handle_revoke_share_link(self.auth().clone(), token, store).await
    }

    pub async fn get_share_settings(&self, notebook_id: String) -> Result<ShareSettings, AppError> {
        let store = self
            .share_store()
            .ok_or_else(|| AppError::internal("postgres backend is not configured"))?;
        avrag_share::handle_get_share_settings(self.auth().clone(), notebook_id, store).await
    }

    pub async fn update_share_settings(
        &self,
        notebook_id: String,
        access_level: Option<String>,
        allow_download: Option<bool>,
    ) -> Result<ShareSettings, AppError> {
        let store = self
            .share_store()
            .ok_or_else(|| AppError::internal("postgres backend is not configured"))?;
        avrag_share::handle_update_share_settings(
            self.auth().clone(),
            notebook_id,
            access_level,
            allow_download,
            store,
        )
        .await
    }

    pub async fn update_share_access_level(
        &self,
        notebook_id: String,
        access_level: String,
    ) -> Result<String, AppError> {
        let store = self
            .share_store()
            .ok_or_else(|| AppError::internal("postgres backend is not configured"))?;
        avrag_share::handle_update_access_level(self.auth().clone(), notebook_id, access_level, store)
            .await
    }

    pub async fn get_share_analytics(
        &self,
        notebook_id: String,
    ) -> Result<Vec<ShareAnalytics>, AppError> {
        let store = self
            .share_store()
            .ok_or_else(|| AppError::internal("postgres backend is not configured"))?;
        avrag_share::handle_get_share_analytics(self.auth().clone(), notebook_id, store).await
    }

    pub async fn get_share_access_logs(
        &self,
        notebook_id: String,
    ) -> Result<Vec<ShareAccessLog>, AppError> {
        let store = self
            .share_store()
            .ok_or_else(|| AppError::internal("postgres backend is not configured"))?;
        avrag_share::handle_get_share_access_logs(self.auth().clone(), notebook_id, None, store).await
    }

    pub async fn validate_share_token(&self, token: &str) -> Result<Option<String>, AppError> {
        let store = self
            .share_store()
            .ok_or_else(|| AppError::internal("postgres backend is not configured"))?;
        avrag_share::handle_validate_token(token, store).await
    }

    pub async fn list_share_members(
        &self,
        notebook_id: String,
    ) -> Result<Vec<avrag_share::NotebookMember>, AppError> {
        let store = self
            .share_store()
            .ok_or_else(|| AppError::internal("postgres backend is not configured"))?;
        avrag_share::handle_list_members(self.auth().clone(), notebook_id, store).await
    }

    pub async fn invite_share_member(
        &self,
        notebook_id: String,
        email: String,
        role: AccessLevel,
    ) -> Result<(), AppError> {
        let store = self
            .share_store()
            .ok_or_else(|| AppError::internal("postgres backend is not configured"))?;
        avrag_share::handle_invite_member(self.auth().clone(), notebook_id, email, role, store)
            .await
            .map(|_| ())
    }

    pub async fn accept_share_invite(
        &self,
        notebook_id: String,
        member_id: String,
    ) -> Result<(), AppError> {
        let store = self
            .share_store()
            .ok_or_else(|| AppError::internal("postgres backend is not configured"))?;
        avrag_share::handle_accept_invite(self.auth().clone(), notebook_id, member_id, store).await
    }

    pub async fn decline_share_invite(
        &self,
        notebook_id: String,
        member_id: String,
    ) -> Result<(), AppError> {
        let store = self
            .share_store()
            .ok_or_else(|| AppError::internal("postgres backend is not configured"))?;
        avrag_share::handle_decline_invite(self.auth().clone(), notebook_id, member_id, store).await
    }

    pub async fn remove_share_member(
        &self,
        notebook_id: String,
        member_id: String,
    ) -> Result<(), AppError> {
        let store = self
            .share_store()
            .ok_or_else(|| AppError::internal("postgres backend is not configured"))?;
        avrag_share::handle_remove_member(self.auth().clone(), notebook_id, member_id, store).await
    }

    pub async fn get_shared_notebook(
        &self,
        token: &str,
    ) -> Result<Option<SharedNotebookPayload>, AppError> {
        let store = self
            .share_store()
            .ok_or_else(|| AppError::internal("postgres backend is not configured"))?;
        avrag_share::handle_get_shared_notebook(token, store).await
    }

    pub async fn share_member_count(&self, notebook_id: &str) -> i64 {
        let Some(store) = self.share_store() else {
            return 0;
        };
        avrag_share::handle_list_members(self.auth().clone(), notebook_id.to_string(), store)
            .await
            .map(|members| members.len() as i64)
            .unwrap_or(0)
    }

    pub async fn share_enabled_for_notebook(&self, notebook_id: &str) -> bool {
        let Some(store) = self.share_store() else {
            return false;
        };
        avrag_share::handle_get_share_settings(self.auth().clone(), notebook_id.to_string(), store)
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
        let store = self.share_store()?;
        let notebook_id = avrag_share::handle_validate_token(token, store).await.ok()??;
        Uuid::parse_str(&notebook_id).ok()
    }
}
