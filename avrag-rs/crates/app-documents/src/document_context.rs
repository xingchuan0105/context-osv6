use app_core::{DocumentScopeValidator, StorageContext, domain_rows::DocumentScopeState};
use async_trait::async_trait;
use avrag_auth::AuthContext;
use common::AppError;
use contracts::documents::DocumentStatus;
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct DocumentContext;

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
            .collect::<std::collections::HashSet<_>>();

        if let Some(store) = storage.document_store() {
            let unique_doc_ids = unique_doc_ids.iter().copied().collect::<Vec<_>>();
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
        } else {
            let state = storage.inner().read().await;
            let org_id = StorageContext::current_org_id(auth);
            for doc_id in doc_scope {
                let Some(stored) = state.documents.get(doc_id) else {
                    return Err(AppError::validation(
                        "invalid_doc_scope",
                        format!("document {doc_id} does not exist"),
                    ));
                };
                if stored.document.org_id != org_id {
                    return Err(AppError::validation(
                        "invalid_doc_scope",
                        format!("document {doc_id} is not accessible"),
                    ));
                }
                if !matches!(stored.document.status, DocumentStatus::Completed) {
                    return Err(AppError::validation(
                        "invalid_doc_scope",
                        format!("document {doc_id} is not ready for RAG execution"),
                    ));
                }
            }
        }

        if let Some(notebook_id) = auth.notebook_id() {
            self.validate_document_scope(auth, storage, &notebook_id.to_string(), doc_scope)
                .await?;
        }
        Ok(())
    }

    pub async fn resolve_citation_asset_url(
        &self,
        storage: &StorageContext,
        asset: &app_core::DocumentAssetRow,
    ) -> Option<String> {
        storage.resolve_citation_asset_url(asset).await
    }
}

#[async_trait]
impl DocumentScopeValidator for DocumentContext {
    async fn validate_document_scope(
        &self,
        auth: &AuthContext,
        storage: &StorageContext,
        notebook_id: &str,
        document_ids: &[String],
    ) -> Result<(), AppError> {
        if document_ids.is_empty() {
            return Ok(());
        }
        let notebook_uuid = Uuid::parse_str(notebook_id)
            .map_err(|_| AppError::not_found("notebook_not_found", "notebook not found"))?;

        if let Some(store) = storage.document_store() {
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
                let seed_notebook_uuid = Uuid::parse_str(&seed.notebook_id)
                    .map_err(|_| AppError::internal("document notebook id is invalid"))?;
                if seed_notebook_uuid != notebook_uuid {
                    return Err(AppError::validation(
                        "invalid_document_scope",
                        format!("document {document_id} is not in notebook {notebook_id}"),
                    ));
                }
            }
            return Ok(());
        }

        let state = storage.inner().read().await;
        let org_id = StorageContext::current_org_id(auth);
        for document_id in document_ids {
            let Some(stored) = state.documents.get(document_id) else {
                return Err(AppError::validation(
                    "invalid_document_scope",
                    format!("document {document_id} does not exist"),
                ));
            };
            if stored.document.org_id != org_id {
                return Err(AppError::validation(
                    "invalid_document_scope",
                    format!("document {document_id} is not accessible"),
                ));
            }
            if stored.document.notebook_id != notebook_id {
                return Err(AppError::validation(
                    "invalid_document_scope",
                    format!("document {document_id} is not in notebook {notebook_id}"),
                ));
            }
        }
        Ok(())
    }
}

pub type PgDocumentScopeValidator = DocumentContext;
pub type DocumentService = DocumentContext;
