use std::sync::Arc;

use async_trait::async_trait;
use app_core::{domain_rows::UserProfileRow, map_pg_error, AdminStorePort};
use avrag_auth::AuthContext;
use avrag_storage_pg::PgAppRepository;
use chrono::{DateTime, Utc};
use common::{ApiKeyRow, AppError, CreateApiKeyResponse, NotificationRow};
use uuid::Uuid;

pub struct PgAdminStoreAdapter {
    repo: Arc<PgAppRepository>,
}

impl PgAdminStoreAdapter {
    pub fn new(repo: Arc<PgAppRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl AdminStorePort for PgAdminStoreAdapter {
    async fn get_user_profile(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
    ) -> Result<Option<UserProfileRow>, AppError> {
        self.repo
            .get_user_profile(auth, user_id)
            .await
            .map_err(map_pg_error)
    }

    async fn upsert_user_profile(
        &self,
        auth: &AuthContext,
        profile: &UserProfileRow,
    ) -> Result<(), AppError> {
        self.repo
            .upsert_user_profile(auth, profile)
            .await
            .map_err(map_pg_error)
    }

    async fn list_api_keys(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
    ) -> Result<Vec<ApiKeyRow>, AppError> {
        self.repo
            .list_api_keys(auth, notebook_id)
            .await
            .map_err(map_pg_error)
    }

    async fn create_api_key(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
        name: &str,
        permissions: &[String],
        rate_limit_rpm: i32,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<CreateApiKeyResponse, AppError> {
        let (api_key, plaintext_key) = self
            .repo
            .create_api_key(
                auth,
                notebook_id,
                name,
                permissions,
                rate_limit_rpm.max(0) as u32,
                expires_at,
            )
            .await
            .map_err(map_pg_error)?;
        Ok(CreateApiKeyResponse {
            api_key,
            plaintext_key,
        })
    }

    async fn revoke_api_key(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
        key_id: Uuid,
    ) -> Result<bool, AppError> {
        self.repo
            .revoke_api_key(auth, notebook_id, key_id)
            .await
            .map_err(map_pg_error)
    }

    async fn list_notifications(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<NotificationRow>, AppError> {
        self.repo
            .list_notifications(auth, user_id, limit, offset)
            .await
            .map_err(map_pg_error)
    }

    async fn mark_notification_read(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
        notification_id: Uuid,
    ) -> Result<bool, AppError> {
        self.repo
            .mark_notification_read(auth, user_id, notification_id)
            .await
            .map_err(map_pg_error)
    }
}
