//! Product App — Desktop (W2: desktop relay token CRUD for cloud login).
//!
//! Session side (`/api/v1/desktop/tokens*`) mints/lists/revokes long-lived
//! relay tokens for the signed-in account owner. The relay side
//! (`/v1/relay/*` guard) resolves presented tokens via the same store.

use app_core::{
    DesktopTokenIdentity, DesktopTokenStorePort, DesktopTokenView, MintedDesktopTokenResponse,
};
use common::{ApiResponse, AppError};
use contracts::auth_runtime::AuthContext;
use std::sync::Arc;
use uuid::Uuid;

pub struct DesktopApp<'a> {
    pub(crate) auth: &'a AuthContext,
    pub(crate) store: Arc<dyn DesktopTokenStorePort>,
}

impl<'a> DesktopApp<'a> {
    fn auth_required<T>() -> ApiResponse<T> {
        ApiResponse::err("authenticated_user_required", "authenticated user required")
    }

    fn to_api<T>(result: Result<T, AppError>) -> ApiResponse<T> {
        match result {
            Ok(value) => ApiResponse::ok(value),
            Err(error) => ApiResponse::err(error.code(), error.message()),
        }
    }

    /// Mint a new relay token for the account owner. Plaintext returned once.
    pub async fn mint_token(&self, name: &str) -> ApiResponse<MintedDesktopTokenResponse> {
        let owner = self.owner_user_id();
        let Some(owner) = owner else {
            return Self::auth_required();
        };
        Self::to_api(app_core::mint_desktop_token(&self.store, owner, name).await)
    }

    /// List the account owner's tokens (redacted; includes revoked rows).
    pub async fn list_tokens(&self) -> ApiResponse<Vec<DesktopTokenView>> {
        let Some(owner) = self.owner_user_id() else {
            return Self::auth_required();
        };
        Self::to_api(self.store.list(owner).await)
    }

    /// Soft-revoke a token by id (owner-scoped; revoked tokens fail relay auth).
    pub async fn revoke_token(&self, id: Uuid) -> ApiResponse<DesktopTokenView> {
        let Some(owner) = self.owner_user_id() else {
            return Self::auth_required();
        };
        Self::to_api(self.store.revoke(owner, id).await)
    }

    /// Relay guard path: resolve an active identity from a plaintext bearer token.
    ///
    /// Fail-closed contract for callers: `Ok(None)` = unknown/revoked → 401;
    /// `Err` = store failure → 5xx (never silently allow).
    pub async fn resolve_token(
        &self,
        plaintext: &str,
    ) -> Result<Option<DesktopTokenIdentity>, AppError> {
        if !plaintext.starts_with(app_core::DESKTOP_TOKEN_PREFIX) {
            return Ok(None);
        }
        self.store
            .resolve_by_hash(&app_core::hash_desktop_token(plaintext))
            .await
    }

    /// Payer / tenant root: account owner (B2C personal, T8 — no org).
    fn owner_user_id(&self) -> Option<Uuid> {
        let owner = self.auth.user_id().into_uuid();
        if owner.is_nil() { None } else { Some(owner) }
    }
}
