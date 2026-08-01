use async_trait::async_trait;
use common::AppError;
use contracts::auth_runtime::AuthContext;
use uuid::Uuid;

/// Storage quota gate — implementations call billing + persistence as needed.
#[async_trait]
pub trait BillingQuotaPort: Send + Sync {
    async fn ensure_storage_bytes_quota(
        &self,
        auth: &AuthContext,
        bytes: i64,
    ) -> Result<(), AppError>;

    async fn notebook_exists(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<bool, AppError>;
}
