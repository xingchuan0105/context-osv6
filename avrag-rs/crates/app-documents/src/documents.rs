use app_billing::BillingContext;
use app_core::{
    AnalyticsServiceCtx, ObjectStoreHeadError, StorageContext, StoredDocument,
    parse_uuid_or_app_error,
};
use avrag_auth::AuthContext;
use common::{
    AppError, CreateDocumentRequest, Document, DocumentContentResponse, ParsedPreviewResponse,
    StatusOnlyResponse, UpdateDocumentRequest, new_id, now_rfc3339,
};
use contracts::documents::{CreateDocumentUploadResponse, DocumentStatus};
use ingestion::{AuditAction, IngestDocumentPayload, build_ingest_task, task_audit};
use tokio::time::{Duration, sleep};
use tracing::info;
use uuid::Uuid;

use crate::analytics_helpers::{
    record_product_event_if_available, record_storage_cost_event_if_available,
};
use crate::document_context::DocumentContext;
use crate::helpers::{
    build_parsed_preview, build_summary, document_is_deleting_or_deleted,
    document_upload_status_is_mutable_for_app, handle_document_deletion_outcome,
    handle_upload_invalid_outcome, handle_upload_queue_outcome, sanitize_filename,
    upload_status_conflict_error,
};

impl DocumentContext {
    pub async fn list_documents(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        notebook_id: Option<&str>,
        document_id: Option<&str>,
    ) -> Vec<Document> {
        if let Some(store) = storage.document_store() {
            let notebook_uuid = notebook_id.and_then(|value| Uuid::parse_str(value).ok());
            let document_uuid = document_id.and_then(|value| Uuid::parse_str(value).ok());
            return store
                .list_documents(auth, notebook_uuid, document_uuid)
                .await
                .unwrap_or_default();
        }
        let state = storage.inner().read().await;
        state
            .documents
            .values()
            .filter(|stored| {
                stored.document.org_id == StorageContext::current_org_id(auth)
                    && notebook_id
                        .map(|id| stored.document.notebook_id == id)
                        .unwrap_or(true)
                    && document_id
                        .map(|id| stored.document.id == id)
                        .unwrap_or(true)
                    && !document_is_deleting_or_deleted(&stored.document.status)
            })
            .map(|stored| stored.document.clone())
            .collect()
    }

    pub async fn create_document_upload(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        _billing: &BillingContext,
        analytics: &AnalyticsServiceCtx,
        notebook_id: &str,
        req: CreateDocumentRequest,
    ) -> Result<CreateDocumentUploadResponse, AppError> {
        if req.filename.trim().is_empty() {
            return Err(AppError::validation(
                "filename_required",
                "filename is required",
            ));
        }
        ingestion::parser::ParseRouter::ensure_supported_file_type(
            req.filename.trim(),
            &req.mime_type,
        )
        .map_err(|error| AppError::validation(error.code(), error.to_string()))?;

        if req.file_size > storage.max_upload_file_size_bytes() {
            return Err(AppError::validation(
                "file_too_large",
                format!(
                    "file size {} exceeds maximum allowed size of {} bytes",
                    req.file_size,
                    storage.max_upload_file_size_bytes()
                ),
            ));
        }

        if let Some(store) = storage.document_store() {
            let quota = storage.billing_quota().ok_or_else(|| {
                AppError::internal("billing quota port is required for document uploads")
            })?;
            quota
                .ensure_storage_bytes_quota(auth, req.file_size as i64)
                .await?;
            let notebook_id =
                parse_uuid_or_app_error(notebook_id, "notebook_not_found", "notebook not found")?;
            if store.get_notebook(auth, notebook_id).await?.is_none() {
                return Err(AppError::not_found(
                    "notebook_not_found",
                    "notebook not found",
                ));
            }
            let document = store
                .create_document(
                    auth,
                    notebook_id,
                    req.filename.trim(),
                    req.file_size,
                    &req.mime_type,
                )
                .await?;
            let seed = store
                .get_document_task_seed(
                    auth,
                    parse_uuid_or_app_error(
                        &document.id,
                        "document_not_found",
                        "document not found",
                    )?,
                )
                .await?
                .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
            record_product_event_if_available(
                auth,
                analytics,
                analytics::ProductEventName::DocumentUploadStarted,
                analytics::Surface::Workspace,
                analytics::ResultTag::Success,
                None,
                Some(notebook_id),
                serde_json::json!({
                    "document_id": document.id.clone(),
                    "filename": req.filename.trim(),
                    "file_size": req.file_size,
                    "mime_type": req.mime_type,
                    "source": "file_upload",
                }),
            )
            .await;
            record_storage_cost_event_if_available(
                auth,
                analytics,
                analytics::CostEventName::UploadBytesMetered,
                "upload",
                Some(notebook_id),
                req.file_size as i64,
                "file_upload",
                serde_json::json!({
                    "document_id": document.id.clone(),
                    "filename": req.filename.trim(),
                    "mime_type": req.mime_type,
                }),
            )
            .await;
            return Ok(CreateDocumentUploadResponse {
                document_id: document.id.clone(),
                upload_url: storage.signed_upload_url(&document.id, &seed.object_path, None)?,
                status: "pending".to_string(),
            });
        }

        let notebook = self
            .get_notebook(auth, storage, notebook_id)
            .await
            .ok_or_else(|| AppError::not_found("notebook_not_found", "notebook not found"))?;

        let now = now_rfc3339();
        let document_id = new_id();
        let mime_type = req.mime_type.clone();
        let document = Document {
            id: document_id.clone(),
            org_id: StorageContext::current_org_id(auth),
            notebook_id: notebook.id.clone(),
            owner_id: StorageContext::current_user_id(auth),
            file_name: req.filename.trim().to_string(),
            mime_type,
            file_size: req.file_size,
            status: DocumentStatus::Pending,
            chunk_count: 0,
            created_at: now.clone(),
            updated_at: now,
        };

        let stored = StoredDocument {
            document,
            content: String::new(),
            summary: None,
            parsed_items: Vec::new(),
        };

        {
            let mut state = storage.inner().write().await;
            state.documents.insert(document_id.clone(), stored);
        }
        record_product_event_if_available(
            auth,
            analytics,
            analytics::ProductEventName::DocumentUploadStarted,
            analytics::Surface::Workspace,
            analytics::ResultTag::Success,
            None,
            Uuid::parse_str(notebook_id).ok(),
            serde_json::json!({
                "document_id": document_id,
                "filename": req.filename.trim(),
                "file_size": req.file_size,
                "mime_type": req.mime_type,
                "source": "file_upload",
            }),
        )
        .await;
        record_storage_cost_event_if_available(
            auth,
            analytics,
            analytics::CostEventName::UploadBytesMetered,
            "upload",
            Uuid::parse_str(notebook_id).ok(),
            req.file_size as i64,
            "file_upload",
            serde_json::json!({
                "document_id": document_id.clone(),
                "filename": req.filename.trim(),
                "mime_type": req.mime_type,
            }),
        )
        .await;

        Ok(CreateDocumentUploadResponse {
            document_id: document_id.clone(),
            upload_url: storage.signed_upload_url(
                &document_id,
                &format!(
                    "{}/{}/{}/{}",
                    StorageContext::current_org_id(auth),
                    notebook_id,
                    document_id,
                    sanitize_filename(req.filename.trim())
                ),
                None,
            )?,
            status: "pending".to_string(),
        })
    }

    pub async fn put_uploaded_document(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        document_id: &str,
        body: Vec<u8>,
    ) -> Result<StatusOnlyResponse, AppError> {
        if let Some(store) = storage.document_store() {
            let document_id =
                parse_uuid_or_app_error(document_id, "document_not_found", "document not found")?;
            let seed = store
                .get_document_task_seed(auth, document_id)
                .await?
                .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
            if !document_upload_status_is_mutable_for_app(&seed.status) {
                return Err(upload_status_conflict_error(&seed.status));
            }
            storage
                .object_store()
                .put(&seed.object_path, &body)
                .await
                .map_err(|error| AppError::internal(error.to_string()))?;
            return Ok(StatusOnlyResponse {
                status: "uploaded".to_string(),
            });
        }

        let mut state = storage.inner().write().await;
        let stored = state
            .documents
            .get_mut(document_id)
            .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
        if stored.document.org_id != StorageContext::current_org_id(auth) {
            return Err(AppError::not_found(
                "document_not_found",
                "document not found",
            ));
        }
        if !document_upload_status_is_mutable_for_app(&stored.document.status) {
            return Err(upload_status_conflict_error(&stored.document.status));
        }

        let content = String::from_utf8(body).unwrap_or_else(|_| {
            "Binary upload received. Parsed preview is not available.".to_string()
        });
        let parsed_items = build_parsed_preview(&content);
        stored.document.file_size = content.len() as u64;
        stored.document.chunk_count = parsed_items.len();
        stored.document.updated_at = now_rfc3339();
        stored.content = content.clone();
        stored.summary = Some(build_summary(&content));
        stored.parsed_items = parsed_items;
        Ok(StatusOnlyResponse {
            status: "uploaded".to_string(),
        })
    }

    pub async fn put_uploaded_document_stream<S, E>(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        document_id: &str,
        stream: S,
    ) -> Result<StatusOnlyResponse, AppError>
    where
        S: futures::Stream<Item = std::result::Result<bytes::Bytes, E>>
            + Send
            + Sync
            + Unpin
            + 'static,
        E: std::error::Error + Send + Sync + 'static,
    {
        if let Some(store) = storage.document_store() {
            let document_id =
                parse_uuid_or_app_error(document_id, "document_not_found", "document not found")?;
            let seed = store
                .get_document_task_seed(auth, document_id)
                .await?
                .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
            if !document_upload_status_is_mutable_for_app(&seed.status) {
                return Err(upload_status_conflict_error(&seed.status));
            }
            use futures::StreamExt;
            storage
                .object_store()
                .put_stream(
                    &seed.object_path,
                    Box::pin(stream.map(|result| {
                        result.map_err(|error| AppError::internal(error.to_string()))
                    })),
                )
                .await?;
            return Ok(StatusOnlyResponse {
                status: "uploaded".to_string(),
            });
        }

        use futures::StreamExt;
        let mut body = Vec::new();
        let mut stream = stream;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| AppError::internal(e.to_string()))?;
            body.extend_from_slice(&chunk);
        }
        self.put_uploaded_document(auth, storage, document_id, body)
            .await
    }

    pub async fn complete_document_upload(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        analytics: &AnalyticsServiceCtx,
        document_id: &str,
    ) -> Result<StatusOnlyResponse, AppError> {
        if let Some(store) = storage.document_store() {
            let document_uuid =
                parse_uuid_or_app_error(document_id, "document_not_found", "document not found")?;
            let seed = store
                .get_document_task_seed(auth, document_uuid)
                .await?
                .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;

            if !document_upload_status_is_mutable_for_app(&seed.status) {
                return Err(upload_status_conflict_error(&seed.status));
            }

            let object_metadata = match storage.object_store().head(&seed.object_path).await {
                Ok(metadata) => metadata,
                Err(
                    ObjectStoreHeadError::NotFound { .. } | ObjectStoreHeadError::NotFile { .. },
                ) => {
                    let detail = format!("object missing or invalid for {}", seed.object_path);
                    handle_upload_invalid_outcome(
                        store
                            .set_document_upload_invalid(auth, document_uuid, &detail)
                            .await?,
                    )?;
                    return Err(AppError::validation(
                        "upload_validation_failed",
                        "uploaded object is missing or invalid",
                    ));
                }
                Err(ObjectStoreHeadError::Backend(error)) => {
                    return Err(AppError::internal(format!(
                        "uploaded object metadata check failed: {error}"
                    )));
                }
            };

            if object_metadata.size_bytes != seed.file_size {
                let detail = format!(
                    "object size mismatch for {}: expected {} bytes, got {} bytes",
                    seed.object_path, seed.file_size, object_metadata.size_bytes
                );
                handle_upload_invalid_outcome(
                    store
                        .set_document_upload_invalid(auth, document_uuid, &detail)
                        .await?,
                )?;
                return Err(AppError::validation(
                    "upload_validation_failed",
                    format!(
                        "uploaded object size mismatch: expected {} bytes, got {} bytes",
                        seed.file_size, object_metadata.size_bytes
                    ),
                ));
            }

            let task = build_ingest_task(
                seed.org_id.clone(),
                seed.notebook_id.clone(),
                seed.document_id.clone(),
                Some(StorageContext::current_user_id(auth)),
                IngestDocumentPayload {
                    source_uri: format!("object://{}", seed.object_path),
                    object_path: seed.object_path.clone(),
                    mime_type: seed.mime_type.clone(),
                    filename: seed.filename.clone(),
                    file_size: seed.file_size,
                },
            );
            let queue_outcome = store
                .queue_validated_document_upload(
                    auth,
                    document_uuid,
                    object_metadata.size_bytes,
                    object_metadata.sha256_hex.as_deref(),
                    &task,
                )
                .await?;
            let task_inserted = handle_upload_queue_outcome(queue_outcome)?;
            let notebook_id = Uuid::parse_str(&seed.notebook_id).ok();
            let metadata = serde_json::json!({
                "document_id": seed.document_id.clone(),
                "filename": seed.filename.clone(),
                "file_size": seed.file_size,
                "actual_file_size": object_metadata.size_bytes,
                "upload_sha256": object_metadata.sha256_hex.clone(),
                "mime_type": seed.mime_type.clone(),
                "status": "queued",
            });
            if task_inserted {
                store
                    .append_audit_record(&task_audit(
                        &task,
                        AuditAction::TaskEnqueued,
                        serde_json::json!({
                            "kind": task.kind,
                            "document_id": task.document_id,
                            "object_path": seed.object_path,
                        }),
                    ))
                    .await?;
            }
            record_product_event_if_available(
                auth,
                analytics,
                analytics::ProductEventName::DocumentUploadCompleted,
                analytics::Surface::Workspace,
                analytics::ResultTag::Success,
                None,
                notebook_id,
                metadata,
            )
            .await;
            return Ok(StatusOnlyResponse {
                status: "queued".to_string(),
            });
        }

        let (notebook_id, file_name, file_size, mime_type) = {
            let mut state = storage.inner().write().await;
            let stored = state
                .documents
                .get_mut(document_id)
                .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
            if !document_upload_status_is_mutable_for_app(&stored.document.status) {
                return Err(upload_status_conflict_error(&stored.document.status));
            }
            stored.document.status = DocumentStatus::Queued;
            stored.document.updated_at = now_rfc3339();
            (
                stored.document.notebook_id.clone(),
                stored.document.file_name.clone(),
                stored.document.file_size,
                stored.document.mime_type.clone(),
            )
        };
        record_product_event_if_available(
            auth,
            analytics,
            analytics::ProductEventName::DocumentUploadCompleted,
            analytics::Surface::Workspace,
            analytics::ResultTag::Success,
            None,
            Uuid::parse_str(&notebook_id).ok(),
            serde_json::json!({
                "document_id": document_id,
                "filename": file_name,
                "file_size": file_size,
                "mime_type": mime_type,
                "status": "queued",
            }),
        )
        .await;
        Ok(StatusOnlyResponse {
            status: "queued".to_string(),
        })
    }

    pub async fn transition_document_status(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        document_id: &str,
        status: DocumentStatus,
    ) -> Result<(), AppError> {
        if document_is_deleting_or_deleted(&status) {
            return Err(AppError::validation(
                "unsupported_document_status_transition",
                "deleting and deleted are reserved for the document deletion workflow",
            ));
        }
        if let Some(store) = storage.document_store() {
            let document_id =
                parse_uuid_or_app_error(document_id, "document_not_found", "document not found")?;
            let updated = store.set_document_status(auth, document_id, status).await?;
            if !updated {
                return Err(AppError::not_found(
                    "document_not_found",
                    "document not found",
                ));
            }
            return Ok(());
        }

        let mut state = storage.inner().write().await;
        let stored = state
            .documents
            .get_mut(document_id)
            .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
        if stored.document.org_id != StorageContext::current_org_id(auth) {
            return Err(AppError::not_found(
                "document_not_found",
                "document not found",
            ));
        }
        if document_is_deleting_or_deleted(&stored.document.status) {
            return Err(AppError::not_found(
                "document_not_found",
                "document not found",
            ));
        }
        stored.document.status = status;
        stored.document.updated_at = now_rfc3339();
        Ok(())
    }

    pub async fn simulate_ingestion(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        document_id: String,
    ) {
        info!(document_id, "starting simulated ingestion");
        let _ = self
            .transition_document_status(auth, storage, &document_id, DocumentStatus::Processing)
            .await;
        sleep(Duration::from_secs(1)).await;
        let _ = self
            .transition_document_status(auth, storage, &document_id, DocumentStatus::Completed)
            .await;
        info!(document_id, "completed simulated ingestion");
    }

    pub async fn update_document(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        document_id: &str,
        req: UpdateDocumentRequest,
    ) -> Result<StatusOnlyResponse, AppError> {
        if req
            .status
            .as_ref()
            .is_some_and(document_is_deleting_or_deleted)
        {
            return Err(AppError::validation(
                "unsupported_document_status_update",
                "deleting and deleted are reserved for the document deletion workflow",
            ));
        }
        if let Some(store) = storage.document_store() {
            let document_id =
                parse_uuid_or_app_error(document_id, "document_not_found", "document not found")?;
            let notebook_id = req
                .notebook_id
                .as_deref()
                .map(|value| {
                    parse_uuid_or_app_error(value, "notebook_not_found", "notebook not found")
                })
                .transpose()?;
            let updated = store
                .update_document(
                    auth,
                    document_id,
                    req.filename.as_deref(),
                    notebook_id,
                    req.status.clone(),
                )
                .await?;
            if !updated {
                return Err(AppError::not_found(
                    "document_not_found",
                    "document not found",
                ));
            }
            return Ok(StatusOnlyResponse {
                status: "updated".to_string(),
            });
        }

        let mut state = storage.inner().write().await;
        let stored = state
            .documents
            .get_mut(document_id)
            .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
        if stored.document.org_id != StorageContext::current_org_id(auth) {
            return Err(AppError::not_found(
                "document_not_found",
                "document not found",
            ));
        }
        if document_is_deleting_or_deleted(&stored.document.status) {
            return Err(AppError::not_found(
                "document_not_found",
                "document not found",
            ));
        }

        if let Some(filename) = req.filename {
            stored.document.file_name = filename;
        }
        if let Some(notebook_id) = req.notebook_id {
            stored.document.notebook_id = notebook_id;
        }
        if let Some(status) = req.status {
            stored.document.status = status;
        }
        stored.document.updated_at = now_rfc3339();
        Ok(StatusOnlyResponse {
            status: "updated".to_string(),
        })
    }

    pub async fn delete_document(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        document_id: &str,
    ) -> Result<StatusOnlyResponse, AppError> {
        if let Some(store) = storage.document_store() {
            let document_id =
                parse_uuid_or_app_error(document_id, "document_not_found", "document not found")?;
            let outcome = store.delete_document(auth, document_id).await?;
            return handle_document_deletion_outcome(outcome);
        }

        let mut state = storage.inner().write().await;
        let stored = state
            .documents
            .get_mut(document_id)
            .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
        if stored.document.org_id != StorageContext::current_org_id(auth) {
            return Err(AppError::not_found(
                "document_not_found",
                "document not found",
            ));
        }
        if matches!(stored.document.status, DocumentStatus::Deleted) {
            return Ok(StatusOnlyResponse {
                status: "deleted".to_string(),
            });
        }
        stored.document.status = DocumentStatus::Deleting;
        stored.document.updated_at = now_rfc3339();
        Ok(StatusOnlyResponse {
            status: "deleting".to_string(),
        })
    }

    pub async fn reindex_document(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        analytics: &AnalyticsServiceCtx,
        document_id: &str,
    ) -> Result<StatusOnlyResponse, AppError> {
        if let Some(store) = storage.document_store() {
            let document_id =
                parse_uuid_or_app_error(document_id, "document_not_found", "document not found")?;
            let seed = store
                .get_document_task_seed(auth, document_id)
                .await?
                .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
            if document_is_deleting_or_deleted(&seed.status) {
                return Err(AppError::not_found(
                    "document_not_found",
                    "document not found",
                ));
            }
            let updated = store
                .set_document_status(auth, document_id, DocumentStatus::Queued)
                .await?;
            if !updated {
                return Err(AppError::not_found(
                    "document_not_found",
                    "document not found",
                ));
            }
            let notebook_id = Uuid::parse_str(&seed.notebook_id).ok();
            let metadata = serde_json::json!({
                "document_id": seed.document_id.clone(),
                "filename": seed.filename.clone(),
                "reason": "manual",
            });
            self.enqueue_reindex_task(auth, storage, seed).await?;
            record_product_event_if_available(
                auth,
                analytics,
                analytics::ProductEventName::DocumentReindexed,
                analytics::Surface::Workspace,
                analytics::ResultTag::Success,
                None,
                notebook_id,
                metadata,
            )
            .await;
            return Ok(StatusOnlyResponse {
                status: "queued".to_string(),
            });
        }

        self.transition_document_status(auth, storage, document_id, DocumentStatus::Queued)
            .await?;
        record_product_event_if_available(
            auth,
            analytics,
            analytics::ProductEventName::DocumentReindexed,
            analytics::Surface::Workspace,
            analytics::ResultTag::Success,
            None,
            None,
            serde_json::json!({
                "document_id": document_id.to_string(),
                "reason": "manual",
            }),
        )
        .await;
        Ok(StatusOnlyResponse {
            status: "queued".to_string(),
        })
    }

    pub async fn get_document_content(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        document_id: &str,
    ) -> Result<DocumentContentResponse, AppError> {
        if let Some(store) = storage.document_store() {
            let document_id =
                parse_uuid_or_app_error(document_id, "document_not_found", "document not found")?;
            return store
                .get_document_content(auth, document_id)
                .await?
                .ok_or_else(|| AppError::not_found("document_not_found", "document not found"));
        }

        let state = storage.inner().read().await;
        let stored = state
            .documents
            .get(document_id)
            .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
        if stored.document.org_id != StorageContext::current_org_id(auth) {
            return Err(AppError::not_found(
                "document_not_found",
                "document not found",
            ));
        }
        if document_is_deleting_or_deleted(&stored.document.status) {
            return Err(AppError::not_found(
                "document_not_found",
                "document not found",
            ));
        }
        Ok(DocumentContentResponse {
            content: stored.content.clone(),
            summary: stored.summary.clone(),
        })
    }

    pub async fn get_parsed_preview(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        document_id: &str,
        cursor: usize,
        limit: usize,
    ) -> Result<ParsedPreviewResponse, AppError> {
        if let Some(store) = storage.document_store() {
            let document_id =
                parse_uuid_or_app_error(document_id, "document_not_found", "document not found")?;
            let cursor_str = cursor.to_string();
            let cursor_ref = if cursor == 0 {
                None
            } else {
                Some(cursor_str.as_str())
            };
            return store
                .get_parsed_preview(auth, document_id, cursor_ref, limit)
                .await;
        }

        let state = storage.inner().read().await;
        let stored = state
            .documents
            .get(document_id)
            .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
        if stored.document.org_id != StorageContext::current_org_id(auth) {
            return Err(AppError::not_found(
                "document_not_found",
                "document not found",
            ));
        }
        if document_is_deleting_or_deleted(&stored.document.status) {
            return Err(AppError::not_found(
                "document_not_found",
                "document not found",
            ));
        }
        let items = stored
            .parsed_items
            .iter()
            .skip(cursor)
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        let next_cursor = cursor + items.len();
        Ok(ParsedPreviewResponse {
            items,
            has_more: next_cursor < stored.parsed_items.len(),
            next_cursor,
            summary: stored.summary.clone(),
        })
    }
}
