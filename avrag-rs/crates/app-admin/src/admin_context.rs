use std::collections::BTreeMap;

use app_core::{map_pg_error, parse_uuid_or_app_error, StorageContext};
use avrag_auth::AuthContext;
use common::{
    new_id, now_rfc3339, ApiKeyRow, AppError, CreateApiKeyRequest, CreateApiKeyResponse,
    NotificationRow, StatusOnlyResponse,
};

#[derive(Clone, Default)]
pub struct AdminContext;

impl AdminContext {
    pub fn new() -> Self {
        Self
    }

    pub async fn list_api_keys(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        notebook_id: &str,
    ) -> Result<Vec<ApiKeyRow>, AppError> {
        if let Some(pg) = storage.pg() {
            let notebook_uuid =
                parse_uuid_or_app_error(notebook_id, "notebook_not_found", "notebook not found")?;
            return pg
                .list_api_keys(auth, Some(notebook_uuid))
                .await
                .map_err(map_pg_error);
        }

        let keys = storage.api_keys().read().await;
        Ok(keys.get(notebook_id).cloned().unwrap_or_default())
    }

    pub async fn create_api_key(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        notebook_id: &str,
        req: CreateApiKeyRequest,
    ) -> Result<CreateApiKeyResponse, AppError> {
        if req.name.trim().is_empty() {
            return Err(AppError::validation("name_required", "name is required"));
        }
        let permissions = if req.permissions.is_empty() {
            vec!["query".to_string()]
        } else {
            req.permissions.clone()
        };
        let rate_limit_rpm = req.rate_limit_rpm.unwrap_or(60);
        let expires_at = req
            .expires_at
            .as_deref()
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&chrono::Utc));

        if let Some(pg) = storage.pg() {
            let notebook_uuid =
                parse_uuid_or_app_error(notebook_id, "notebook_not_found", "notebook not found")?;
            let (api_key, plaintext_key) = pg
                .create_api_key(
                    auth,
                    Some(notebook_uuid),
                    req.name.trim(),
                    &permissions,
                    rate_limit_rpm,
                    expires_at,
                )
                .await
                .map_err(map_pg_error)?;
            return Ok(CreateApiKeyResponse {
                api_key,
                plaintext_key,
            });
        }

        let row = ApiKeyRow {
            id: new_id(),
            org_id: StorageContext::current_org_id(auth),
            notebook_id: notebook_id.to_string(),
            key_prefix: "ctx_new".to_string(),
            name: req.name,
            permissions,
            rate_limit_rpm,
            expires_at: req.expires_at,
            last_used_at: None,
            is_active: true,
            created_by: StorageContext::current_user_id(auth),
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
        };
        {
            let mut keys = storage.api_keys().write().await;
            keys.entry(notebook_id.to_string())
                .or_default()
                .push(row.clone());
        }
        Ok(CreateApiKeyResponse {
            api_key: row,
            plaintext_key: format!("sk_{}", new_id().replace('-', "")),
        })
    }

    pub async fn revoke_api_key(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        notebook_id: &str,
        key_id: &str,
    ) -> Result<StatusOnlyResponse, AppError> {
        if let Some(pg) = storage.pg() {
            let notebook_uuid =
                parse_uuid_or_app_error(notebook_id, "notebook_not_found", "notebook not found")?;
            let key_uuid =
                parse_uuid_or_app_error(key_id, "api_key_not_found", "api key not found")?;
            let revoked = pg
                .revoke_api_key(auth, Some(notebook_uuid), key_uuid)
                .await
                .map_err(map_pg_error)?;
            if !revoked {
                return Err(AppError::not_found(
                    "api_key_not_found",
                    "api key not found",
                ));
            }
            return Ok(StatusOnlyResponse {
                status: "revoked".to_string(),
            });
        }

        let mut keys_map = storage.api_keys().write().await;
        let keys = keys_map.entry(notebook_id.to_string()).or_default();
        let before = keys.len();
        keys.retain(|item| item.id != key_id);
        if before == keys.len() {
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
        if let Some(pg) = storage.pg() {
            let user_id = auth
                .actor_id()
                .map(|value| value.into_uuid())
                .ok_or_else(|| AppError::unauthorized("notification access requires a user"))?;
            return pg
                .list_notifications(auth, user_id, limit, offset)
                .await
                .map_err(map_pg_error);
        }

        let state = storage.inner().read().await;
        if state.notifications.is_empty() {
            return Ok(vec![NotificationRow {
                id: "notif-m1-skeleton".to_string(),
                org_id: StorageContext::current_org_id(auth),
                user_id: StorageContext::current_user_id(auth),
                event_type: "system.degrade".to_string(),
                title: "M1/M2 skeleton running".to_string(),
                body: "Rust API is serving placeholder notebook/document/chat flows with explicit degrade trace.".to_string(),
                data: BTreeMap::new(),
                read_at: None,
                created_at: now_rfc3339(),
                updated_at: now_rfc3339(),
            }]);
        }
        Ok(state.notifications.clone())
    }

    pub async fn mark_notification_read(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        notification_id: &str,
    ) -> Result<StatusOnlyResponse, AppError> {
        if let Some(pg) = storage.pg() {
            let user_id = auth
                .actor_id()
                .map(|value| value.into_uuid())
                .ok_or_else(|| AppError::unauthorized("notification access requires a user"))?;
            let notification_uuid = parse_uuid_or_app_error(
                notification_id,
                "notification_not_found",
                "notification not found",
            )?;
            let updated = pg
                .mark_notification_read(auth, user_id, notification_uuid)
                .await
                .map_err(map_pg_error)?;
            if !updated {
                return Err(AppError::not_found(
                    "notification_not_found",
                    "notification not found",
                ));
            }
            return Ok(StatusOnlyResponse {
                status: "ok".to_string(),
            });
        }

        let mut state = storage.inner().write().await;
        if let Some(item) = state
            .notifications
            .iter_mut()
            .find(|item| item.id == notification_id)
        {
            if item.read_at.is_none() {
                item.read_at = Some(now_rfc3339());
                item.updated_at = now_rfc3339();
            }
            return Ok(StatusOnlyResponse {
                status: "ok".to_string(),
            });
        }
        Err(AppError::not_found(
            "notification_not_found",
            "notification not found",
        ))
    }
}
