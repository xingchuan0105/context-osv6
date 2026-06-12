use app_billing::BillingContext;
use app_core::{parse_uuid_or_app_error, AnalyticsServiceCtx, StorageContext, StoredDocument};
use avrag_auth::AuthContext;
use common::{AddUrlSourceRequest, AppError, Document, SourceRow, new_id, now_rfc3339};
use contracts::documents::{CreateDocumentUploadResponse, DocumentStatus};
use uuid::Uuid;

use crate::analytics_helpers::{
    record_product_event_if_available, record_storage_cost_event_if_available,
};
use crate::document_context::DocumentContext;
use crate::helpers::{
    build_parsed_preview, build_summary, document_is_deleting_or_deleted, status_label,
};
use crate::url_fetch::{fetch_url_import, write_raw_object};

impl DocumentContext {
    pub async fn add_url_source(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        _billing: &BillingContext,
        analytics: &AnalyticsServiceCtx,
        notebook_id: &str,
        req: AddUrlSourceRequest,
    ) -> Result<CreateDocumentUploadResponse, AppError> {
        let url = req.url.trim();
        if url.is_empty() {
            return Err(AppError::validation("url_required", "url is required"));
        }
        let fetched = fetch_url_import(url).await?;

        if let Some(store) = storage.document_store() {
            let notebook_id =
                parse_uuid_or_app_error(notebook_id, "notebook_not_found", "notebook not found")?;
            let quota = storage.billing_quota().ok_or_else(|| {
                AppError::internal("billing quota port is required for url imports")
            })?;
            quota
                .ensure_storage_bytes_quota(auth, fetched.raw_bytes.len() as i64)
                .await?;
            if !quota.notebook_exists(auth, notebook_id).await? {
                return Err(AppError::not_found(
                    "notebook_not_found",
                    "notebook not found",
                ));
            }
            let document = store
                .create_document(
                    auth,
                    notebook_id,
                    &fetched.filename,
                    fetched.raw_bytes.len() as u64,
                    &fetched.mime_type,
                )
                .await?;
            let document_id =
                parse_uuid_or_app_error(&document.id, "document_not_found", "document not found")?;
            let seed = store
                .get_document_task_seed(auth, document_id)
                .await?
                .ok_or_else(|| AppError::not_found("document_not_found", "document not found"))?;
            write_raw_object(
                storage.object_root_path(),
                &seed.object_path,
                &fetched.raw_bytes,
            )
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
            store
                .set_document_status(auth, document_id, DocumentStatus::Queued)
                .await?;
            self.enqueue_ingest_task(auth, storage, seed).await?;
            record_product_event_if_available(
                auth,
                analytics,
                analytics::ProductEventName::UrlSourceAdded,
                analytics::Surface::Workspace,
                analytics::ResultTag::Success,
                None,
                Some(notebook_id),
                serde_json::json!({
                    "document_id": document.id.clone(),
                    "url": url,
                    "filename": document.file_name.clone(),
                    "mime_type": document.mime_type.clone(),
                    "status": "queued",
                }),
            )
            .await;
            record_storage_cost_event_if_available(
                auth,
                analytics,
                analytics::CostEventName::UploadBytesMetered,
                "upload",
                Some(notebook_id),
                fetched.raw_bytes.len() as i64,
                "url_import",
                serde_json::json!({
                    "document_id": document.id.clone(),
                    "url": url,
                    "mime_type": document.mime_type.clone(),
                }),
            )
            .await;
            return Ok(CreateDocumentUploadResponse {
                document_id: document.id,
                upload_url: String::new(),
                status: "queued".to_string(),
            });
        }

        let now = now_rfc3339();
        let document_id = new_id();
        let parsed_items = build_parsed_preview(&fetched.extracted_content);
        let stored = StoredDocument {
            document: Document {
                id: document_id.clone(),
                org_id: StorageContext::current_org_id(auth),
                notebook_id: notebook_id.to_string(),
                owner_id: StorageContext::current_user_id(auth),
                file_name: fetched.filename,
                mime_type: fetched.mime_type,
                file_size: fetched.raw_bytes.len() as u64,
                status: DocumentStatus::Completed,
                chunk_count: parsed_items.len(),
                created_at: now.clone(),
                updated_at: now,
            },
            content: fetched.extracted_content.clone(),
            summary: Some(build_summary(&fetched.extracted_content)),
            parsed_items,
        };
        {
            let mut state = storage.inner().write().await;
            state.documents.insert(document_id.clone(), stored);
        }
        record_product_event_if_available(
            auth,
            analytics,
            analytics::ProductEventName::UrlSourceAdded,
            analytics::Surface::Workspace,
            analytics::ResultTag::Success,
            None,
            Uuid::parse_str(notebook_id).ok(),
            serde_json::json!({
                "document_id": document_id,
                "url": url,
                "status": "completed",
            }),
        )
        .await;
        record_storage_cost_event_if_available(
            auth,
            analytics,
            analytics::CostEventName::UploadBytesMetered,
            "upload",
            Uuid::parse_str(notebook_id).ok(),
            fetched.raw_bytes.len() as i64,
            "url_import",
            serde_json::json!({
                "document_id": document_id.clone(),
                "url": url,
            }),
        )
        .await;
        Ok(CreateDocumentUploadResponse {
            document_id,
            upload_url: String::new(),
            status: "completed".to_string(),
        })
    }

    pub async fn list_sources(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        notebook_id: Option<&str>,
    ) -> Vec<SourceRow> {
        if let Some(store) = storage.document_store() {
            let notebook_uuid = notebook_id.and_then(|value| Uuid::parse_str(value).ok());
            return store
                .list_sources(auth, notebook_uuid)
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
                    && !document_is_deleting_or_deleted(&stored.document.status)
            })
            .filter_map(|stored| {
                let notebook = state.notebooks.get(&stored.document.notebook_id)?;
                if notebook.org_id != StorageContext::current_org_id(auth) {
                    return None;
                }
                Some(SourceRow {
                    id: stored.document.id.clone(),
                    notebook_id: notebook.id.clone(),
                    notebook_name: notebook.name.clone(),
                    title: stored.document.file_name.clone(),
                    file_name: stored.document.file_name.clone(),
                    status: status_label(&stored.document.status).to_string(),
                })
            })
            .collect()
    }
}
