use app_core::{
    MemoryApiKeyRecord, StorageContext, current_org_id, current_user_id,
    deactivate_memory_api_key, parse_uuid_or_app_error, register_memory_api_key,
    validate_memory_api_key,
};
use contracts::auth_runtime::{AuthContext, OrgId, SubjectKind};
use common::{
    ApiKeyRow, AppError, CreateApiKeyRequest, CreateApiKeyResponse, NotificationRow,
    StatusOnlyResponse, new_id, now_rfc3339,
};
use uuid::Uuid;

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
        if let Some(store) = storage.admin_store() {
            let notebook_uuid =
                parse_uuid_or_app_error(notebook_id, "notebook_not_found", "notebook not found")?;
            return store.list_api_keys(auth, Some(notebook_uuid)).await;
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
        let notebook_uuid =
            parse_uuid_or_app_error(notebook_id, "notebook_not_found", "notebook not found")?;
        let permissions =
            contracts::normalize_api_key_permissions(&req.permissions, Some(notebook_uuid));
        let rate_limit_rpm = req.rate_limit_rpm.unwrap_or(60);
        let expires_at = req
            .expires_at
            .as_deref()
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&chrono::Utc));

        if let Some(store) = storage.admin_store() {
            return store
                .create_api_key(
                    auth,
                    Some(notebook_uuid),
                    req.name.trim(),
                    &permissions,
                    i32::try_from(rate_limit_rpm).unwrap_or(60),
                    expires_at,
                )
                .await;
        }

        let plaintext_key = format!("sk_{}", new_id().replace('-', ""));
        let row = ApiKeyRow {
            id: new_id(),
            org_id: current_org_id(auth),
            notebook_id: notebook_id.to_string(),
            key_prefix: plaintext_key.chars().take(12).collect(),
            name: req.name,
            permissions: permissions.clone(),
            rate_limit_rpm,
            expires_at: req.expires_at,
            last_used_at: None,
            is_active: true,
            created_by: current_user_id(auth),
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
        };
        {
            let mut keys = storage.api_keys().write().await;
            keys.entry(notebook_id.to_string())
                .or_default()
                .push(row.clone());
        }
        let notebook_uuid = Uuid::parse_str(notebook_id).ok();
        register_memory_api_key(
            storage.api_key_hashes(),
            &plaintext_key,
            MemoryApiKeyRecord {
                id: Uuid::parse_str(&row.id).unwrap_or_else(|_| Uuid::new_v4()),
                org_id: OrgId::from(
                    Uuid::parse_str(&row.org_id).unwrap_or_else(|_| Uuid::new_v4()),
                ),
                notebook_id: notebook_uuid,
                permissions,
                rate_limit_rpm,
                is_active: true,
                expires_at,
            },
        )
        .await;
        Ok(CreateApiKeyResponse {
            api_key: row,
            plaintext_key,
        })
    }

    pub async fn list_org_api_keys(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
    ) -> Result<Vec<ApiKeyRow>, AppError> {
        ensure_org_api_key_admin(auth)?;
        if let Some(store) = storage.admin_store() {
            return store.list_api_keys(auth, None).await;
        }

        const ORG_KEY_BUCKET: &str = "__org__";
        let keys = storage.api_keys().read().await;
        Ok(keys.get(ORG_KEY_BUCKET).cloned().unwrap_or_default())
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
        let permissions = contracts::normalize_api_key_permissions(&req.permissions, None);
        let rate_limit_rpm = req.rate_limit_rpm.unwrap_or(60);
        let expires_at = req
            .expires_at
            .as_deref()
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&chrono::Utc));

        if let Some(store) = storage.admin_store() {
            return store
                .create_api_key(
                    auth,
                    None,
                    req.name.trim(),
                    &permissions,
                    i32::try_from(rate_limit_rpm).unwrap_or(60),
                    expires_at,
                )
                .await;
        }

        const ORG_KEY_BUCKET: &str = "__org__";
        let plaintext_key = format!("sk_{}", new_id().replace('-', ""));
        let row = ApiKeyRow {
            id: new_id(),
            org_id: current_org_id(auth),
            notebook_id: String::new(),
            key_prefix: plaintext_key.chars().take(12).collect(),
            name: req.name,
            permissions: permissions.clone(),
            rate_limit_rpm,
            expires_at: req.expires_at,
            last_used_at: None,
            is_active: true,
            created_by: current_user_id(auth),
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
        };
        {
            let mut keys = storage.api_keys().write().await;
            keys.entry(ORG_KEY_BUCKET.to_string())
                .or_default()
                .push(row.clone());
        }
        register_memory_api_key(
            storage.api_key_hashes(),
            &plaintext_key,
            MemoryApiKeyRecord {
                id: Uuid::parse_str(&row.id).unwrap_or_else(|_| Uuid::new_v4()),
                org_id: OrgId::from(
                    Uuid::parse_str(&row.org_id).unwrap_or_else(|_| Uuid::new_v4()),
                ),
                notebook_id: None,
                permissions,
                rate_limit_rpm,
                is_active: true,
                expires_at,
            },
        )
        .await;
        Ok(CreateApiKeyResponse {
            api_key: row,
            plaintext_key,
        })
    }

    pub async fn revoke_org_api_key(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        key_id: &str,
    ) -> Result<StatusOnlyResponse, AppError> {
        ensure_org_api_key_admin(auth)?;
        if let Some(store) = storage.admin_store() {
            let key_uuid =
                parse_uuid_or_app_error(key_id, "api_key_not_found", "api key not found")?;
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
            return Ok(StatusOnlyResponse {
                status: "revoked".to_string(),
            });
        }

        const ORG_KEY_BUCKET: &str = "__org__";
        let mut keys_map = storage.api_keys().write().await;
        let keys = keys_map.entry(ORG_KEY_BUCKET.to_string()).or_default();
        let before = keys.len();
        keys.retain(|item| item.id != key_id);
        if before == keys.len() {
            return Err(AppError::not_found(
                "api_key_not_found",
                "api key not found",
            ));
        }
        deactivate_memory_api_key(storage.api_key_hashes(), key_id).await;
        Ok(StatusOnlyResponse {
            status: "revoked".to_string(),
        })
    }

    pub async fn revoke_api_key(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        notebook_id: &str,
        key_id: &str,
    ) -> Result<StatusOnlyResponse, AppError> {
        if let Some(store) = storage.admin_store() {
            let notebook_uuid =
                parse_uuid_or_app_error(notebook_id, "notebook_not_found", "notebook not found")?;
            let key_uuid =
                parse_uuid_or_app_error(key_id, "api_key_not_found", "api key not found")?;
            let revoked = store
                .revoke_api_key(auth, Some(notebook_uuid), key_uuid)
                .await?;
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
        deactivate_memory_api_key(storage.api_key_hashes(), key_id).await;
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
        if let Some(store) = storage.admin_store() {
            let user_id = auth
                .actor_id()
                .map(|value| value.into_uuid())
                .ok_or_else(|| AppError::unauthorized("notification access requires a user"))?;
            return store.list_notifications(auth, user_id, limit, offset).await;
        }

        let state = storage.inner().read().await;
        if state.notifications.is_empty() {
            return Ok(vec![NotificationRow {
                id: "notif-m1-skeleton".to_string(),
                org_id: current_org_id(auth),
                user_id: current_user_id(auth),
                event_type: "system.degrade".to_string(),
                title: "M1/M2 skeleton running".to_string(),
                body: "Rust API is serving placeholder notebook/document/chat flows with explicit degrade trace.".to_string(),
                data: std::collections::BTreeMap::new(),
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
        if let Some(store) = storage.admin_store() {
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

    pub async fn validate_api_key(
        &self,
        storage: &StorageContext,
        plaintext_key: &str,
    ) -> Result<Option<MemoryApiKeyRecord>, AppError> {
        if storage.admin_store().is_some() {
            return Ok(None);
        }
        Ok(validate_memory_api_key(storage.api_key_hashes(), plaintext_key).await)
    }
}
