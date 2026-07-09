use app_core::{
    AnalyticsServiceCtx, DocumentStorePort, StorageContext, parse_uuid_or_app_error,
};
use common::{
    AppError, CreateNotebookRequest, StatusOnlyResponse, UpdateNotebookRequest,
};
use contracts::auth_runtime::AuthContext;
use contracts::notebooks::Notebook;
use std::sync::Arc;
use uuid::Uuid;

use crate::analytics_helpers::record_product_event_if_available;
use crate::document_context::DocumentContext;

fn require_document_store(
    storage: &StorageContext,
) -> Result<Arc<dyn DocumentStorePort>, AppError> {
    storage.document_store().ok_or_else(|| {
        AppError::internal("document store port is required (wire MemoryDocumentStore or Pg adapter at bootstrap)")
    })
}

impl DocumentContext {
    pub async fn list_notebooks(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
    ) -> Vec<Notebook> {
        let Ok(store) = require_document_store(storage) else {
            return Vec::new();
        };
        store.list_notebooks(auth).await.unwrap_or_default()
    }

    pub async fn get_notebook(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        notebook_id: &str,
    ) -> Option<Notebook> {
        let store = require_document_store(storage).ok()?;
        let notebook_id = Uuid::parse_str(notebook_id).ok()?;
        let notebook = store.get_notebook(auth, notebook_id).await.ok().flatten()?;
        Some(notebook)
    }

    pub async fn create_notebook(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        analytics: &AnalyticsServiceCtx,
        req: CreateNotebookRequest,
    ) -> Result<Notebook, AppError> {
        if req.name.trim().is_empty() {
            return Err(AppError::validation(
                "name_required",
                "notebook name is required",
            ));
        }

        let store = require_document_store(storage)?;
        let notebook = store
            .create_notebook(auth, req.name.trim(), req.description.trim())
            .await?;
        record_product_event_if_available(
            auth,
            analytics,
            analytics::ProductEventName::NotebookCreated,
            analytics::Surface::Workspace,
            analytics::ResultTag::Success,
            None,
            Uuid::parse_str(&notebook.id).ok(),
            serde_json::json!({
                "name": notebook.name.clone(),
                "runtime_mode": storage.runtime_mode(),
            }),
        )
        .await;
        Ok(notebook)
    }

    pub async fn update_notebook(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        notebook_id: &str,
        req: UpdateNotebookRequest,
    ) -> Result<Notebook, AppError> {
        let store = require_document_store(storage)?;
        let notebook_id =
            parse_uuid_or_app_error(notebook_id, "notebook_not_found", "notebook not found")?;
        store
            .update_notebook(
                auth,
                notebook_id,
                Some(req.name.trim()),
                Some(req.description.trim()),
            )
            .await?
            .ok_or_else(|| AppError::not_found("notebook_not_found", "notebook not found"))
    }

    pub async fn delete_notebook(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        notebook_id: &str,
    ) -> Result<StatusOnlyResponse, AppError> {
        let store = require_document_store(storage)?;
        let notebook_id =
            parse_uuid_or_app_error(notebook_id, "notebook_not_found", "notebook not found")?;
        let deleted = store.delete_notebook(auth, notebook_id).await?;
        if !deleted {
            return Err(AppError::not_found(
                "notebook_not_found",
                "notebook not found",
            ));
        }
        Ok(StatusOnlyResponse {
            status: "deleted".to_string(),
        })
    }
}
