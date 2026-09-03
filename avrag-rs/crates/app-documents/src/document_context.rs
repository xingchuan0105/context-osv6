use app_core::{DocumentScopeValidator, DocumentStorePort, ObjectStoreConfig, StorageContext};
use async_trait::async_trait;
use common::AppError;
use contracts::auth_runtime::AuthContext;
use contracts::documents::DocumentStatus;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct DocumentContext;

fn require_document_store(
    storage: &StorageContext,
) -> Result<Arc<dyn DocumentStorePort>, AppError> {
    storage.document_store().ok_or_else(|| {
        AppError::internal("document store port is required (wire MemoryDocumentStore or Pg adapter at bootstrap)")
    })
}

impl DocumentContext {
    pub fn new() -> Self {
        Self
    }

    pub async fn resolve_citation_asset_url(
        &self,
        objects: &ObjectStoreConfig,
        asset: &app_core::DocumentAssetRow,
    ) -> Option<String> {
        objects.resolve_citation_asset_url(asset).await
    }

    /// Workspace-bound completed artifacts with binding + parse version facts
    /// (chat-first W2d snapshot; scope truth is the binding tables).
    pub async fn completed_workspace_binding_versions(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        workspace_id: &str,
    ) -> Result<Vec<app_core::WorkspaceBindingVersion>, AppError> {
        let workspace_uuid = Uuid::parse_str(workspace_id)
            .map_err(|_| AppError::not_found("workspace_not_found", "workspace not found"))?;
        let store = require_document_store(storage)?;
        store
            .completed_workspace_binding_versions(auth, workspace_uuid)
            .await
    }

    /// Completed document ids in a workspace, used as the RAG enforcement scope.
    ///
    /// Fail-closed: a missing/errored document store surfaces as an error rather than
    /// an empty scope (an empty scope means "no enforcement / org-wide" upstream).
    pub async fn completed_workspace_doc_ids(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        workspace_id: &str,
    ) -> Result<Vec<String>, AppError> {
        let workspace_uuid = Uuid::parse_str(workspace_id)
            .map_err(|_| AppError::not_found("workspace_not_found", "workspace not found"))?;
        let store = require_document_store(storage)?;
        let documents = store
            .list_documents(auth, Some(workspace_uuid), None)
            .await?;
        Ok(documents
            .into_iter()
            .filter(|document| matches!(document.status, DocumentStatus::Completed))
            .map(|document| document.id)
            .collect())
    }
}

#[async_trait]
impl DocumentScopeValidator for DocumentContext {
    async fn validate_document_scope(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        workspace_id: &str,
        document_ids: &[String],
    ) -> Result<(), AppError> {
        if document_ids.is_empty() {
            return Ok(());
        }
        let notebook_uuid = Uuid::parse_str(workspace_id)
            .map_err(|_| AppError::not_found("workspace_not_found", "workspace not found"))?;

        let store = require_document_store(storage)?;
        for document_id in document_ids {
            let document_uuid = Uuid::parse_str(document_id).map_err(|_| {
                AppError::validation(
                    "invalid_document_scope",
                    format!("document scope contains an invalid document id: {document_id}"),
                )
            })?;
            // Scope truth is the typed workspace binding, not a column on the artifact.
            let bound = store
                .list_documents(auth, Some(notebook_uuid), Some(document_uuid))
                .await?;
            if bound.is_empty() {
                return Err(AppError::validation(
                    "invalid_document_scope",
                    format!("document {document_id} is not in notebook {workspace_id}"),
                ));
            }
        }
        Ok(())
    }
}

pub type PgDocumentScopeValidator = DocumentContext;
pub type DocumentService = DocumentContext;
