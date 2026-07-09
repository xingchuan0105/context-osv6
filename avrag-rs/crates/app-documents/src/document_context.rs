use app_core::{
    DocumentScopeValidator, DocumentStorePort, ObjectStoreConfig, StorageContext,
    domain_rows::DocumentScopeState,
};
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

    pub async fn validate_rag_doc_scope(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        doc_scope: &[String],
    ) -> Result<(), AppError> {
        let doc_ids = doc_scope
            .iter()
            .map(|id| {
                Uuid::parse_str(id).map_err(|_| {
                    AppError::validation(
                        "invalid_doc_scope",
                        format!("doc_scope contains an invalid document id: {id}"),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let unique_doc_ids = doc_ids
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        let store = require_document_store(storage)?;
        let states = store
            .get_document_scope_states(auth, &unique_doc_ids)
            .await?;
        if states.len() != unique_doc_ids.len() {
            return Err(AppError::validation(
                "invalid_doc_scope",
                "doc_scope contains a document that does not exist or is not accessible",
            ));
        }
        if let Some(DocumentScopeState {
            document_id,
            status,
        }) = states
            .into_iter()
            .find(|state| !matches!(state.status, DocumentStatus::Completed))
        {
            return Err(AppError::validation(
                "invalid_doc_scope",
                format!("document {document_id} is not ready for RAG execution: {status:?}"),
            ));
        }

        if let Some(workspace_id) = auth.workspace_id() {
            self.validate_document_scope(auth, storage, &workspace_id.to_string(), doc_scope)
                .await?;
        }
        Ok(())
    }

    pub async fn resolve_citation_asset_url(
        &self,
        objects: &ObjectStoreConfig,
        asset: &app_core::DocumentAssetRow,
    ) -> Option<String> {
        objects.resolve_citation_asset_url(asset).await
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
            let Some(seed) = store.get_document_task_seed(auth, document_uuid).await? else {
                return Err(AppError::validation(
                    "invalid_document_scope",
                    format!("document {document_id} does not exist or is not accessible"),
                ));
            };
            let seed_notebook_uuid = Uuid::parse_str(&seed.workspace_id)
                .map_err(|_| AppError::internal("document notebook id is invalid"))?;
            if seed_notebook_uuid != notebook_uuid {
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
