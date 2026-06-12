use async_trait::async_trait;
use avrag_auth::AuthContext;
use chrono::{DateTime, Utc};
use common::{
    ApiKeyRow, AppError, CreateApiKeyResponse, NotificationRow, StatusOnlyResponse,
};
use uuid::Uuid;

use crate::domain_rows::UserProfileRow;

#[async_trait]
pub trait AdminStorePort: Send + Sync {
    async fn get_user_profile(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
    ) -> Result<Option<UserProfileRow>, AppError>;

    async fn upsert_user_profile(
        &self,
        auth: &AuthContext,
        profile: &UserProfileRow,
    ) -> Result<(), AppError>;

    async fn list_api_keys(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
    ) -> Result<Vec<ApiKeyRow>, AppError>;

    async fn create_api_key(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
        name: &str,
        permissions: &[String],
        rate_limit_rpm: i32,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<CreateApiKeyResponse, AppError>;

    async fn revoke_api_key(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
        key_id: Uuid,
    ) -> Result<bool, AppError>;

    async fn list_notifications(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<NotificationRow>, AppError>;

    async fn mark_notification_read(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
        notification_id: Uuid,
    ) -> Result<bool, AppError>;
}
