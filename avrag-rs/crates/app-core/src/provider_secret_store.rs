//! Provider secret persistence boundary — encrypting SQL adapters live in bootstrap.

use async_trait::async_trait;
use common::AppError;
use uuid::Uuid;

use crate::provider_secret_domain::{
    ProviderSecretPurpose, ProviderSecretView, ResolvedProviderSecret, UpsertProviderSecretInput,
};

#[async_trait]
pub trait ProviderSecretStorePort: Send + Sync {
    /// Create or replace the active secret for (owner, workspace, purpose).
    /// Returns the public view (fingerprint only). Never returns plaintext.
    async fn upsert(
        &self,
        input: &UpsertProviderSecretInput,
    ) -> Result<ProviderSecretView, AppError>;

    /// List secrets for the owner (active by default). Fingerprints only.
    async fn list(
        &self,
        owner_user_id: Uuid,
        include_revoked: bool,
    ) -> Result<Vec<ProviderSecretView>, AppError>;

    /// Soft-revoke by id (owner-scoped). Revoked secrets cannot resolve.
    async fn revoke(&self, owner_user_id: Uuid, id: Uuid) -> Result<ProviderSecretView, AppError>;

    /// Decrypt the active secret for outbound use.
    ///
    /// Preference: workspace-scoped secret when `workspace_id` is Some and present;
    /// else account-default (`workspace_id` NULL). Revoked rows are never returned.
    async fn resolve(
        &self,
        owner_user_id: Uuid,
        workspace_id: Option<Uuid>,
        purpose: ProviderSecretPurpose,
    ) -> Result<Option<ResolvedProviderSecret>, AppError>;

    /// True when the owner has any active secret for the given purpose (any scope).
    async fn has_active(
        &self,
        owner_user_id: Uuid,
        purpose: ProviderSecretPurpose,
    ) -> Result<bool, AppError>;
}
