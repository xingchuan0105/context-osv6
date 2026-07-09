use app_billing::BillingContext;
use app_core::{AnalyticsServiceCtx, StorageContext, parse_uuid_or_app_error};
use contracts::auth_runtime::AuthContext;
use common::{AddUrlSourceRequest, AppError, SourceRow};
use contracts::documents::{CreateDocumentUploadResponse, DocumentStatus};
use uuid::Uuid;

use crate::analytics_helpers::{
    record_product_event_if_available, record_storage_cost_event_if_available,
};
use crate::document_context::DocumentContext;
use crate::url_fetch::{fetch_url_import, write_raw_object};

impl DocumentContext {
    pub async fn add_url_source(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        _billing: &BillingContext,
        analytics: &AnalyticsServiceCtx,
        workspace_id: &str,
        req: AddUrlSourceRequest,
    ) -> Result<CreateDocumentUploadResponse, AppError> {
        let url = req.url.trim();
        if url.is_empty() {
            return Err(AppError::validation("url_required", "url is required"));
        }
        let fetched = fetch_url_import(url).await?;

        let store = storage.document_store().ok_or_else(|| {
            AppError::internal(
                "document store port is required (wire MemoryDocumentStore or Pg adapter at bootstrap)",
            )
        })?;
        let workspace_id =
            parse_uuid_or_app_error(workspace_id, "notebook_not_found", "notebook not found")?;
        let quota = storage.billing_quota().ok_or_else(|| {
            AppError::internal("billing quota port is required for url imports")
        })?;
        quota
            .ensure_storage_bytes_quota(auth, fetched.raw_bytes.len() as i64)
            .await?;
        if !quota.notebook_exists(auth, workspace_id).await? {
            return Err(AppError::not_found(
                "notebook_not_found",
                "notebook not found",
            ));
        }
        let document = store
            .create_document(
                auth,
                workspace_id,
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
            Some(workspace_id),
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
            Some(workspace_id),
            fetched.raw_bytes.len() as i64,
            "url_import",
            serde_json::json!({
                "document_id": document.id.clone(),
                "url": url,
                "mime_type": document.mime_type.clone(),
            }),
        )
        .await;
        Ok(CreateDocumentUploadResponse {
            document_id: document.id,
            upload_url: String::new(),
            status: "queued".to_string(),
        })
    }

    pub async fn list_sources(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        workspace_id: Option<&str>,
    ) -> Vec<SourceRow> {
        let Some(store) = storage.document_store() else {
            return Vec::new();
        };
        let notebook_uuid = workspace_id.and_then(|value| Uuid::parse_str(value).ok());
        store
            .list_sources(auth, notebook_uuid)
            .await
            .unwrap_or_default()
    }
}
