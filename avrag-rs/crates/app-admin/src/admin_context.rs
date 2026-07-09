use app_core::{
    AdminStorePort, MemoryApiKeyRecord, StorageContext, parse_uuid_or_app_error,
    validate_memory_api_key,
};
use common::{
    ApiKeyRow, AppError, CreateApiKeyRequest, CreateApiKeyResponse, NotificationRow,
    StatusOnlyResponse,
};
use contracts::auth_runtime::{AuthContext, SubjectKind};
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct AdminContext;

fn ensure_org_api_key_admin(auth: &AuthContext) -> Result<(), AppError> {
    if !matches!(auth.subject_kind(), SubjectKind::User) {
        return Err(AppError::forbidden(
            "admin_required",
            "organization admin permission required",
        ));
    }
    auth.ensure_permission(contracts::agent_permissions::PERM_ADMIN)
        .map_err(|_| {
            AppError::forbidden("admin_required", "organization admin permission required")
        })
}

fn require_admin_store(storage: &StorageContext) -> Result<Arc<dyn AdminStorePort>, AppError> {
    storage.admin_store().ok_or_else(|| {
        AppError::internal(
            "admin store port is required (wire MemoryAdminStore or Pg adapter at bootstrap)",
        )
    })
}

impl AdminContext {
    pub fn new() -> Self {
        Self
    }

    pub async fn list_api_keys(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        workspace_id: &str,
    ) -> Result<Vec<ApiKeyRow>, AppError> {
        let store = require_admin_store(storage)?;
        let notebook_uuid =
            parse_uuid_or_app_error(workspace_id, "workspace_not_found", "workspace not found")?;
        store.list_api_keys(auth, Some(notebook_uuid)).await
    }

    pub async fn create_api_key(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        workspace_id: &str,
        req: CreateApiKeyRequest,
    ) -> Result<CreateApiKeyResponse, AppError> {
        if req.name.trim().is_empty() {
            return Err(AppError::validation("name_required", "name is required"));
        }
        let store = require_admin_store(storage)?;
        let notebook_uuid =
            parse_uuid_or_app_error(workspace_id, "workspace_not_found", "workspace not found")?;
        let permissions =
            contracts::normalize_api_key_permissions(&req.permissions, Some(notebook_uuid));
        let rate_limit_rpm = req.rate_limit_rpm.unwrap_or(60);
        let expires_at = req
            .expires_at
            .as_deref()
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&chrono::Utc));
        store
            .create_api_key(
                auth,
                Some(notebook_uuid),
                req.name.trim(),
                &permissions,
                i32::try_from(rate_limit_rpm).unwrap_or(60),
                expires_at,
            )
            .await
    }

    pub async fn list_org_api_keys(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
    ) -> Result<Vec<ApiKeyRow>, AppError> {
        ensure_org_api_key_admin(auth)?;
        let store = require_admin_store(storage)?;
        store.list_api_keys(auth, None).await
    }

    pub async fn create_org_api_key(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        req: CreateApiKeyRequest,
    ) -> Result<CreateApiKeyResponse, AppError> {
        ensure_org_api_key_admin(auth)?;
        if req.name.trim().is_empty() {
            return Err(AppError::validation("name_required", "name is required"));
        }
        let store = require_admin_store(storage)?;
        let permissions = contracts::normalize_api_key_permissions(&req.permissions, None);
        let rate_limit_rpm = req.rate_limit_rpm.unwrap_or(60);
        let expires_at = req
            .expires_at
            .as_deref()
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&chrono::Utc));
        store
            .create_api_key(
                auth,
                None,
                req.name.trim(),
                &permissions,
                i32::try_from(rate_limit_rpm).unwrap_or(60),
                expires_at,
            )
            .await
    }

    pub async fn revoke_org_api_key(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        key_id: &str,
    ) -> Result<StatusOnlyResponse, AppError> {
        ensure_org_api_key_admin(auth)?;
        let store = require_admin_store(storage)?;
        let key_uuid = parse_uuid_or_app_error(key_id, "api_key_not_found", "api key not found")?;
        let is_org_key = store
            .list_api_keys(auth, None)
            .await?
            .into_iter()
            .any(|key| key.id == key_id);
        if !is_org_key {
            return Err(AppError::not_found(
                "api_key_not_found",
                "api key not found",
            ));
        }
        let revoked = store.revoke_api_key(auth, None, key_uuid).await?;
        if !revoked {
            return Err(AppError::not_found(
                "api_key_not_found",
                "api key not found",
            ));
        }
        Ok(StatusOnlyResponse {
            status: "revoked".to_string(),
        })
    }

    pub async fn revoke_api_key(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        workspace_id: &str,
        key_id: &str,
    ) -> Result<StatusOnlyResponse, AppError> {
        let store = require_admin_store(storage)?;
        let notebook_uuid =
            parse_uuid_or_app_error(workspace_id, "workspace_not_found", "workspace not found")?;
        let key_uuid = parse_uuid_or_app_error(key_id, "api_key_not_found", "api key not found")?;
        let revoked = store
            .revoke_api_key(auth, Some(notebook_uuid), key_uuid)
            .await?;
        if !revoked {
            return Err(AppError::not_found(
                "api_key_not_found",
                "api key not found",
            ));
        }
        Ok(StatusOnlyResponse {
            status: "revoked".to_string(),
        })
    }

    pub async fn list_notifications(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<NotificationRow>, AppError> {
        let store = require_admin_store(storage)?;
        let user_id = auth
            .actor_id()
            .map(|value| value.into_uuid())
            .ok_or_else(|| AppError::unauthorized("notification access requires a user"))?;
        store.list_notifications(auth, user_id, limit, offset).await
    }

    pub async fn mark_notification_read(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        notification_id: &str,
    ) -> Result<StatusOnlyResponse, AppError> {
        let store = require_admin_store(storage)?;
        let user_id = auth
            .actor_id()
            .map(|value| value.into_uuid())
            .ok_or_else(|| AppError::unauthorized("notification access requires a user"))?;
        let notification_uuid = parse_uuid_or_app_error(
            notification_id,
            "notification_not_found",
            "notification not found",
        )?;
        let updated = store
            .mark_notification_read(auth, user_id, notification_uuid)
            .await?;
        if !updated {
            return Err(AppError::not_found(
                "notification_not_found",
                "notification not found",
            ));
        }
        Ok(StatusOnlyResponse {
            status: "ok".to_string(),
        })
    }

    /// Validate a plaintext API key against the in-memory hash index.
    /// Used by memory-mode auth; PG-mode API keys are validated via auth_store.
    pub async fn validate_api_key(
        &self,
        storage: &StorageContext,
        plaintext_key: &str,
    ) -> Result<Option<MemoryApiKeyRecord>, AppError> {
        Ok(validate_memory_api_key(storage.api_key_hashes(), plaintext_key).await)
    }
}
