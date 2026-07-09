//! Bound face — admin.

use app_core::StorageContext;
use avrag_storage_pg::PgAppRepository;
use common::AppError;
use contracts::auth_runtime::AuthContext;
use std::sync::Arc;

use super::WorkspaceApiKeyAuth;

pub struct BoundAdmin<'a> {
    pub(crate) admin: &'a app_admin::AdminContext,
    pub(crate) auth: &'a AuthContext,
    pub(crate) storage: &'a StorageContext,
    pub(crate) postgres: Option<Arc<PgAppRepository>>,
}

impl<'a> BoundAdmin<'a> {
    pub async fn validate_workspace_api_key(
        &self,
        plaintext_key: &str,
    ) -> Result<Option<WorkspaceApiKeyAuth>, AppError> {
        if let Some(repo) = self.postgres.as_ref() {
            let validated = repo
                .auth()
                .validate_api_key(plaintext_key)
                .await
                .map_err(crate::pg_error::map_pg_error)?;
            return Ok(validated.map(|key| WorkspaceApiKeyAuth {
                key_id: key.id,
                org_id: key.org_id,
                notebook_id: key.notebook_id,
                permissions: key.permissions,
                rate_limit_rpm: key.rate_limit_rpm,
            }));
        }

        Ok(self
            .admin
            .validate_api_key(self.storage, plaintext_key)
            .await?
            .map(|record| WorkspaceApiKeyAuth {
                key_id: record.id,
                org_id: record.org_id,
                notebook_id: record.notebook_id,
                permissions: record.permissions,
                rate_limit_rpm: record.rate_limit_rpm,
            }))
    }

    pub async fn list_api_keys(
        &self,
        notebook_id: &str,
    ) -> Result<Vec<common::ApiKeyRow>, common::AppError> {
        self.admin
            .list_api_keys(self.auth, self.storage, notebook_id)
            .await
    }

    pub async fn create_api_key(
        &self,
        notebook_id: &str,
        req: common::CreateApiKeyRequest,
    ) -> Result<common::CreateApiKeyResponse, common::AppError> {
        self.admin
            .create_api_key(self.auth, self.storage, notebook_id, req)
            .await
    }

    pub async fn list_org_api_keys(&self) -> Result<Vec<common::ApiKeyRow>, common::AppError> {
        self.admin
            .list_org_api_keys(self.auth, self.storage)
            .await
    }

    pub async fn create_org_api_key(
        &self,
        req: common::CreateApiKeyRequest,
    ) -> Result<common::CreateApiKeyResponse, common::AppError> {
        self.admin
            .create_org_api_key(self.auth, self.storage, req)
            .await
    }

    pub async fn revoke_org_api_key(
        &self,
        key_id: &str,
    ) -> Result<common::StatusOnlyResponse, common::AppError> {
        self.admin
            .revoke_org_api_key(self.auth, self.storage, key_id)
            .await
    }

    pub async fn revoke_api_key(
        &self,
        notebook_id: &str,
        key_id: &str,
    ) -> Result<common::StatusOnlyResponse, common::AppError> {
        self.admin
            .revoke_api_key(self.auth, self.storage, notebook_id, key_id)
            .await
    }

    pub async fn list_notifications(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<common::NotificationRow>, common::AppError> {
        self.admin
            .list_notifications(self.auth, self.storage, limit, offset)
            .await
    }

    pub async fn mark_notification_read(
        &self,
        notification_id: &str,
    ) -> Result<common::StatusOnlyResponse, common::AppError> {
        self.admin
            .mark_notification_read(self.auth, self.storage, notification_id)
            .await
    }
}

