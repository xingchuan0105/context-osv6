use common::{
    ApiKeyRow, AppError, CreateApiKeyRequest, CreateApiKeyResponse, NotificationRow,
    ShareTokenResponse, StatusOnlyResponse, new_id,
};

use super::AppState;

impl AppState {
    pub async fn list_api_keys(&self, notebook_id: &str) -> Result<Vec<ApiKeyRow>, AppError> {
        self.admin
            .list_api_keys(&self.auth, &self.storage, notebook_id)
            .await
    }

    pub async fn create_api_key(
        &self,
        notebook_id: &str,
        req: CreateApiKeyRequest,
    ) -> Result<CreateApiKeyResponse, AppError> {
        self.admin
            .create_api_key(&self.auth, &self.storage, notebook_id, req)
            .await
    }

    pub async fn revoke_api_key(
        &self,
        notebook_id: &str,
        key_id: &str,
    ) -> Result<StatusOnlyResponse, AppError> {
        self.admin
            .revoke_api_key(&self.auth, &self.storage, notebook_id, key_id)
            .await
    }

    pub async fn list_notifications(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<NotificationRow>, AppError> {
        self.admin
            .list_notifications(&self.auth, &self.storage, limit, offset)
            .await
    }

    pub async fn mark_notification_read(
        &self,
        notification_id: &str,
    ) -> Result<StatusOnlyResponse, AppError> {
        self.admin
            .mark_notification_read(&self.auth, &self.storage, notification_id)
            .await
    }

    pub async fn create_share_token(
        &self,
        notebook_id: &str,
    ) -> Result<ShareTokenResponse, AppError> {
        self.get_notebook(notebook_id)
            .await
            .ok_or_else(|| AppError::not_found("notebook_not_found", "notebook not found"))?;
        Ok(ShareTokenResponse {
            share_token: format!("share_{}", new_id()),
        })
    }
}
