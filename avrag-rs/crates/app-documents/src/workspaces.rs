use app_core::{
    AnalyticsServiceCtx, DocumentStorePort, StorageContext, parse_uuid_or_app_error,
};
use common::{
    AppError, CreateWorkspaceRequest, StatusOnlyResponse, UpdateWorkspaceRequest,
};
use contracts::auth_runtime::AuthContext;
use contracts::workspaces::Workspace;
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
    pub async fn list_workspaces(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
    ) -> Vec<Workspace> {
        let Ok(store) = require_document_store(storage) else {
            return Vec::new();
        };
        store.list_workspaces(auth).await.unwrap_or_default()
    }

    pub async fn get_workspace(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        workspace_id: &str,
    ) -> Option<Workspace> {
        let store = require_document_store(storage).ok()?;
        let workspace_id = Uuid::parse_str(workspace_id).ok()?;
        let notebook = store.get_workspace(auth, workspace_id).await.ok().flatten()?;
        Some(notebook)
    }

    pub async fn create_workspace(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        analytics: &AnalyticsServiceCtx,
        req: CreateWorkspaceRequest,
    ) -> Result<Workspace, AppError> {
        if req.name.trim().is_empty() {
            return Err(AppError::validation(
                "name_required",
                "notebook name is required",
            ));
        }

        let store = require_document_store(storage)?;
        let notebook = store
            .create_workspace(auth, req.name.trim(), req.description.trim())
            .await?;
        record_product_event_if_available(
            auth,
            analytics,
            analytics::ProductEventName::WorkspaceCreated,
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

    pub async fn update_workspace(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        workspace_id: &str,
        req: UpdateWorkspaceRequest,
    ) -> Result<Workspace, AppError> {
        let store = require_document_store(storage)?;
        let workspace_id =
            parse_uuid_or_app_error(workspace_id, "workspace_not_found", "workspace not found")?;
        store
            .update_workspace(
                auth,
                workspace_id,
                Some(req.name.trim()),
                Some(req.description.trim()),
            )
            .await?
            .ok_or_else(|| AppError::not_found("workspace_not_found", "workspace not found"))
    }

    pub async fn delete_workspace(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        workspace_id: &str,
    ) -> Result<StatusOnlyResponse, AppError> {
        let store = require_document_store(storage)?;
        let workspace_id =
            parse_uuid_or_app_error(workspace_id, "workspace_not_found", "workspace not found")?;
        let deleted = store.delete_workspace(auth, workspace_id).await?;
        if !deleted {
            return Err(AppError::not_found(
                "workspace_not_found",
                "workspace not found",
            ));
        }
        Ok(StatusOnlyResponse {
            status: "deleted".to_string(),
        })
    }
}
