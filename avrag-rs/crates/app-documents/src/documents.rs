use app_billing::BillingContext;
use app_core::{
    AnalyticsServiceCtx, ObjectStoreHeadError, StorageContext, current_user_id,
    parse_uuid_or_app_error,
};
use contracts::auth_runtime::AuthContext;
use common::{
    AppError, CreateDocumentRequest, Document, DocumentContentResponse, ParsedPreviewResponse,
    StatusOnlyResponse, UpdateDocumentRequest, now_rfc3339,
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
    handle_upload_invalid_outcome, handle_upload_queue_outcome, upload_status_conflict_error,
};

impl DocumentContext {
    pub async fn list_documents(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        workspace_id: Option<&str>,
        document_id: Option<&str>,
    ) -> Vec<Document> {
        let Some(store) = storage.document_store() else {
            return Vec::new();
        };
        let notebook_uuid = workspace_id.and_then(|value| Uuid::parse_str(value).ok());
        let document_uuid = document_id.and_then(|value| Uuid::parse_str(value).ok());
        store
            .list_documents(auth, notebook_uuid, document_uuid)
            .await
            .unwrap_or_default()
    }

    pub async fn create_document_upload(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        _billing: &BillingContext,
        analytics: &AnalyticsServiceCtx,
        workspace_id: &str,
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

        let store = storage.document_store().ok_or_else(|| {
            AppError::internal("document store is required for document uploads")
        })?;
        let quota = storage.billing_quota().ok_or_else(|| {
            AppError::internal("billing quota port is required for document uploads")
        })?;
        quota
            .ensure_storage_bytes_quota(auth, req.file_size as i64)
            .await?;
        let workspace_id =
            parse_uuid_or_app_error(workspace_id, "workspace_not_found", "workspace not found")?;
        if store.get_workspace(auth, workspace_id).await?.is_none() {
            return Err(AppError::not_found(
                "workspace_not_found",
                "workspace not found",
            ));
        }
        let document = store
            .create_document(
                auth,
                workspace_id,
                req.filename.trim(),
                req.file_size,
                &req.mime_type,
            )
            .await?;
        let seed = store
            .get_document_task_seed(
                auth,
                parse_uuid_or_app_error(&document.id, "document_not_found", "document not found")?,
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
            Some(workspace_id),
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
            Some(workspace_id),
            req.file_size as i64,
            "file_upload",
            serde_json::json!({
                "document_id": document.id.clone(),
                "filename": req.filename.trim(),
                "mime_type": req.mime_type,
            }),
        )
        .await;
        Ok(CreateDocumentUploadResponse {
            document_id: document.id.clone(),
            upload_url: storage.signed_upload_url(&document.id, &seed.object_path, None)?,
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
        let store = storage.document_store().ok_or_else(|| {
            AppError::internal("document store is required for document uploads")
        })?;
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
        if storage.uses_memory_adapters() {
            let content =
                String::from_utf8(body).unwrap_or_else(|_| {
                    "Binary upload received. Parsed preview is not available.".to_string()
                });
            let parsed_items = build_parsed_preview(&content);
            let mut state = storage.inner().write().await;
            if let Some(stored) = state.documents.get_mut(&seed.document_id) {
                stored.document.file_size = content.len() as u64;
                stored.document.chunk_count = parsed_items.len();
                stored.document.updated_at = now_rfc3339();
                stored.content = content.clone();
                stored.summary = Some(build_summary(&content));
                stored.parsed_items = parsed_items;
            }
        }
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
        let store = storage.document_store().ok_or_else(|| {
            AppError::internal("document store is required for document uploads")
        })?;
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
            Err(ObjectStoreHeadError::NotFound { .. } | ObjectStoreHeadError::NotFile { .. }) => {
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
            seed.owner_user_id.clone(),
            seed.workspace_id.clone(),
            seed.document_id.clone(),
            Some(current_user_id(auth)),
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
        let workspace_id = Uuid::parse_str(&seed.workspace_id).ok();
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
            workspace_id,
            metadata,
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
        let store = storage.document_store().ok_or_else(|| {
            AppError::internal("document store is required for document status transitions")
        })?;
        let document_id =
            parse_uuid_or_app_error(document_id, "document_not_found", "document not found")?;
        let updated = store.set_document_status(auth, document_id, status).await?;
        if !updated {
            return Err(AppError::not_found(
                "document_not_found",
                "document not found",
            ));
        }
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
        let store = storage.document_store().ok_or_else(|| {
            AppError::internal("document store is required for document updates")
        })?;
        let document_id =
            parse_uuid_or_app_error(document_id, "document_not_found", "document not found")?;
        let workspace_id = req
            .workspace_id
            .as_deref()
            .map(|value| {
                parse_uuid_or_app_error(value, "workspace_not_found", "workspace not found")
            })
            .transpose()?;
        let updated = store
            .update_document(
                auth,
                document_id,
                req.filename.as_deref(),
                workspace_id,
                req.status.clone(),
            )
            .await?;
        if !updated {
            return Err(AppError::not_found(
                "document_not_found",
                "document not found",
            ));
        }
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
        let store = storage.document_store().ok_or_else(|| {
            AppError::internal("document store is required for document deletion")
        })?;
        let document_id =
            parse_uuid_or_app_error(document_id, "document_not_found", "document not found")?;
        let outcome = store.delete_document(auth, document_id).await?;
        handle_document_deletion_outcome(outcome)
    }

    pub async fn reindex_document(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        analytics: &AnalyticsServiceCtx,
        document_id: &str,
    ) -> Result<StatusOnlyResponse, AppError> {
        let store = storage.document_store().ok_or_else(|| {
            AppError::internal("document store is required for document reindexing")
        })?;
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
        let workspace_id = Uuid::parse_str(&seed.workspace_id).ok();
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
            workspace_id,
            metadata,
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
        let store = storage.document_store().ok_or_else(|| {
            AppError::internal("document store is required for document content retrieval")
        })?;
        let document_id =
            parse_uuid_or_app_error(document_id, "document_not_found", "document not found")?;
        store
            .get_document_content(auth, document_id)
            .await?
            .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))
    }

    pub async fn get_parsed_preview(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        document_id: &str,
        cursor: usize,
        limit: usize,
    ) -> Result<ParsedPreviewResponse, AppError> {
        let store = storage.document_store().ok_or_else(|| {
            AppError::internal("document store is required for parsed preview retrieval")
        })?;
        let document_id =
            parse_uuid_or_app_error(document_id, "document_not_found", "document not found")?;
        let cursor_str = cursor.to_string();
        let cursor_ref = if cursor == 0 {
            None
        } else {
            Some(cursor_str.as_str())
        };
        store
            .get_parsed_preview(auth, document_id, cursor_ref, limit)
            .await
    }
}
