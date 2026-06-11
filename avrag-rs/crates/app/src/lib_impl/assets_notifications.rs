use common::{
    ApiKeyRow, AppError, CitationLookupResponse, CreateApiKeyRequest, CreateApiKeyResponse,
    NotificationRow, ShareTokenResponse, StatusOnlyResponse, new_id, now_rfc3339,
};
use std::collections::BTreeMap;

use crate::lib_impl::*;

impl AppState {
    pub async fn lookup_citation(
        &self,
        session_id: &str,
        message_id: i64,
        citation_id: i64,
    ) -> Result<CitationLookupResponse, AppError> {
        if let Some(pg) = self.storage.pg() {
            let session_uuid =
                parse_uuid_or_app_error(session_id, "session_not_found", "session not found")?;
            let message = pg
                .get_message(&self.auth, session_uuid, message_id)
                .await
                .map_err(map_pg_error)?
                .ok_or_else(|| AppError::not_found("message_not_found", "message not found"))?;
            let citation = message
                .citations
                .iter()
                .find(|citation| citation.citation_id == citation_id)
                .cloned()
                .ok_or_else(|| AppError::not_found("citation_not_found", "citation not found"))?;
            if let Some(chunk_id) = citation.chunk_id.as_deref() {
                let chunk_uuid =
                    parse_uuid_or_app_error(chunk_id, "chunk_not_found", "chunk not found")?;

                if let Some(multimodal) = pg
                    .get_multimodal_chunk_by_id(&self.auth, chunk_uuid)
                    .await
                    .map_err(map_pg_error)?
                {
                    let asset = if let Some(asset_id) = multimodal.asset_id {
                        pg.get_document_asset_by_id(&self.auth, asset_id)
                            .await
                            .map_err(map_pg_error)?
                    } else {
                        None
                    };
                    let caption = multimodal
                        .caption
                        .clone()
                        .or_else(|| asset.as_ref().and_then(|item| item.caption.clone()));
                    let page = multimodal.page.map(|value| value as usize).or_else(|| {
                        asset
                            .as_ref()
                            .and_then(|item| item.page.map(|value| value as usize))
                    });
                    let image_url = if let Some(asset) = asset.as_ref() {
                        self.resolve_citation_asset_url(asset).await
                    } else {
                        None
                    };

                    return Ok(CitationLookupResponse {
                        doc_name: Some(citation.doc_name),
                        content: multimodal
                            .context_text
                            .clone()
                            .or_else(|| Some(multimodal.normalized_text.clone())),
                        doc_id: Some(citation.doc_id),
                        chunk_id: Some(chunk_uuid.to_string()),
                        page,
                        chunk_type: Some("image_with_context".to_string()),
                        asset_id: multimodal.asset_id.map(|value| value.to_string()),
                        caption,
                        image_url,
                        parser_backend: Some(multimodal.parser_backend.clone()),
                        source_locator: multimodal
                            .metadata
                            .get("source_locator")
                            .cloned()
                            .filter(|value| !value.is_null()),
                    });
                }

                if let Some(chunk) = pg
                    .get_chunk_by_id(&self.auth, chunk_uuid)
                    .await
                    .map_err(map_pg_error)?
                {
                    return Ok(CitationLookupResponse {
                        doc_name: Some(citation.doc_name),
                        content: Some(chunk.content),
                        doc_id: Some(citation.doc_id),
                        chunk_id: Some(chunk_uuid.to_string()),
                        page: chunk.page.map(|value| value as usize),
                        chunk_type: chunk
                            .metadata
                            .get("block_type")
                            .and_then(serde_json::Value::as_str)
                            .map(ToOwned::to_owned)
                            .or(Some("text".to_string())),
                        asset_id: None,
                        caption: None,
                        image_url: None,
                        parser_backend: chunk
                            .metadata
                            .get("parser_backend")
                            .and_then(serde_json::Value::as_str)
                            .map(ToOwned::to_owned),
                        source_locator: chunk
                            .metadata
                            .get("source_locator")
                            .cloned()
                            .filter(|value| !value.is_null()),
                    });
                }
            }
            let content = self.get_document_content(&citation.doc_id).await?;
            return Ok(CitationLookupResponse {
                doc_name: Some(citation.doc_name),
                content: Some(content.content),
                doc_id: Some(citation.doc_id),
                chunk_id: citation.chunk_id,
                page: citation.page,
                chunk_type: citation.chunk_type,
                asset_id: citation.asset_id,
                caption: citation.caption,
                image_url: citation.image_url,
                parser_backend: citation.parser_backend,
                source_locator: citation.source_locator,
            });
        }

        let state = self.storage.inner().read().await;
        let messages = state
            .messages
            .get(session_id)
            .ok_or_else(|| AppError::not_found("session_not_found", "session not found"))?;
        let session = state
            .sessions
            .get(session_id)
            .ok_or_else(|| AppError::not_found("session_not_found", "session not found"))?;
        if !self.memory_session_visible(&state, session) {
            return Err(AppError::not_found(
                "session_not_found",
                "session not found",
            ));
        }
        let message = messages
            .iter()
            .find(|message| message.id == message_id)
            .ok_or_else(|| AppError::not_found("message_not_found", "message not found"))?;
        let citation = message
            .citations
            .iter()
            .find(|citation| citation.citation_id == citation_id)
            .ok_or_else(|| AppError::not_found("citation_not_found", "citation not found"))?;
        let doc = state
            .documents
            .get(&citation.doc_id)
            .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
        Ok(CitationLookupResponse {
            doc_name: Some(citation.doc_name.clone()),
            content: Some(doc.content.clone()),
            doc_id: Some(citation.doc_id.clone()),
            chunk_id: citation.chunk_id.clone(),
            page: citation.page,
            chunk_type: citation.chunk_type.clone(),
            asset_id: citation.asset_id.clone(),
            caption: citation.caption.clone(),
            image_url: citation.image_url.clone(),
            parser_backend: citation.parser_backend.clone(),
            source_locator: citation.source_locator.clone(),
        })
    }

    pub async fn get_citation_asset(&self, asset_id: &str) -> Result<(Vec<u8>, String), AppError> {
        let pg_opt = self.storage.pg();
        let pg = pg_opt
            .as_ref()
            .ok_or_else(|| AppError::internal("postgres backend is not configured"))?;
        let asset_uuid = parse_uuid_or_app_error(asset_id, "asset_not_found", "asset not found")?;
        let asset = pg
            .get_document_asset_by_id(&self.auth, asset_uuid)
            .await
            .map_err(map_pg_error)?
            .ok_or_else(|| AppError::not_found("asset_not_found", "asset not found"))?;
        let storage_path = asset
            .storage_path
            .clone()
            .ok_or_else(|| AppError::not_found("asset_not_found", "asset not found"))?;
        if is_remote_asset_reference(&storage_path) {
            return Err(AppError::validation(
                "asset_is_remote",
                "remote asset should be fetched via image_url",
            ));
        }

        let bytes = self
            .storage.object_store()
            .get(&storage_path)
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        let mime_type = asset
            .mime_type
            .clone()
            .or_else(|| infer_mime_type_from_path(&storage_path))
            .unwrap_or_else(|| "application/octet-stream".to_string());

        Ok((bytes, mime_type))
    }

    pub async fn list_api_keys(&self, notebook_id: &str) -> Result<Vec<ApiKeyRow>, AppError> {
        if let Some(pg) = self.storage.pg() {
            let notebook_uuid =
                parse_uuid_or_app_error(notebook_id, "notebook_not_found", "notebook not found")?;
            return pg
                .list_api_keys(&self.auth, Some(notebook_uuid))
                .await
                .map_err(map_pg_error);
        }

        let keys = self.storage.api_keys().read().await;
        Ok(keys.get(notebook_id).cloned().unwrap_or_default())
    }

    pub async fn create_api_key(
        &self,
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

        if let Some(pg) = self.storage.pg() {
            let notebook_uuid =
                parse_uuid_or_app_error(notebook_id, "notebook_not_found", "notebook not found")?;
            let (api_key, plaintext_key) = pg
                .create_api_key(
                    &self.auth,
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
            org_id: self.current_org_id(),
            notebook_id: notebook_id.to_string(),
            key_prefix: "ctx_new".to_string(),
            name: req.name,
            permissions,
            rate_limit_rpm,
            expires_at: req.expires_at,
            last_used_at: None,
            is_active: true,
            created_by: self.current_user_id(),
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
        };
        {
            let mut keys = self.storage.api_keys().write().await;
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
        notebook_id: &str,
        key_id: &str,
    ) -> Result<StatusOnlyResponse, AppError> {
        if let Some(pg) = self.storage.pg() {
            let notebook_uuid =
                parse_uuid_or_app_error(notebook_id, "notebook_not_found", "notebook not found")?;
            let key_uuid =
                parse_uuid_or_app_error(key_id, "api_key_not_found", "api key not found")?;
            let revoked = pg
                .revoke_api_key(&self.auth, Some(notebook_uuid), key_uuid)
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

        let mut keys_map = self.storage.api_keys().write().await;
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
        limit: usize,
        offset: usize,
    ) -> Result<Vec<NotificationRow>, AppError> {
        if let Some(pg) = self.storage.pg() {
            let user_id = self
                .auth
                .actor_id()
                .map(|value| value.into_uuid())
                .ok_or_else(|| AppError::unauthorized("notification access requires a user"))?;
            return pg
                .list_notifications(&self.auth, user_id, limit, offset)
                .await
                .map_err(map_pg_error);
        }

        let state = self.storage.inner().read().await;
        if state.notifications.is_empty() {
            return Ok(vec![NotificationRow {
                id: "notif-m1-skeleton".to_string(),
                org_id: self.current_org_id(),
                user_id: self.current_user_id(),
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
        notification_id: &str,
    ) -> Result<StatusOnlyResponse, AppError> {
        if let Some(pg) = self.storage.pg() {
            let user_id = self
                .auth
                .actor_id()
                .map(|value| value.into_uuid())
                .ok_or_else(|| AppError::unauthorized("notification access requires a user"))?;
            let notification_uuid = parse_uuid_or_app_error(
                notification_id,
                "notification_not_found",
                "notification not found",
            )?;
            let updated = pg
                .mark_notification_read(&self.auth, user_id, notification_uuid)
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

        let mut state = self.storage.inner().write().await;
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
