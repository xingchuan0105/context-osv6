use crate::lib_impl::*;
use avrag_storage_pg::DocumentScopeState;
use common::{AppError, DocumentStatus};
use uuid::Uuid;

impl AppState {
    pub async fn execute_rag_execute_plan(
        &self,
        req: common::ExecutePlanRequest,
    ) -> Result<common::ExecutePlanResponse, AppError> {
        req.validate()
            .map_err(|error| AppError::validation("invalid_execute_plan", error.to_string()))?;
        self.validate_execute_plan_doc_scope(&req).await?;

        if let Some(rag_runtime) = &self.rag_runtime {
            return rag_runtime
                .execute_plan(&req, &self.auth)
                .await
                .map_err(map_anyhow_error);
        }

        Err(AppError::validation(
            "rag_runtime_not_configured",
            "RAG execute-plan requires rag_runtime to be configured.",
        ))
    }

    pub async fn execute_runtime_tools(
        &self,
        req: common::RuntimeExecuteRequest,
    ) -> Result<common::RuntimeExecuteResponse, AppError> {
        if req.calls.is_empty() {
            return Err(AppError::validation(
                "invalid_calls",
                "calls must not be empty",
            ));
        }

        if let Some(rag_runtime) = &self.rag_runtime {
            let results = rag_runtime.execute_tools(&self.auth, req.calls).await;
            return Ok(common::RuntimeExecuteResponse { results });
        }

        Err(AppError::validation(
            "rag_runtime_not_configured",
            "RAG runtime execute requires rag_runtime to be configured.",
        ))
    }

    async fn validate_execute_plan_doc_scope(
        &self,
        req: &common::ExecutePlanRequest,
    ) -> Result<(), AppError> {
        if req.doc_scope.is_empty() {
            return Err(AppError::validation(
                "invalid_doc_scope",
                "doc_scope must not be empty",
            ));
        }

        let doc_ids = req
            .doc_scope
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

        if let Some(pg) = &self.pg {
            let unique_doc_ids = unique_doc_ids.iter().copied().collect::<Vec<_>>();
            let states = pg
                .get_document_scope_states(&self.auth, &unique_doc_ids)
                .await
                .map_err(map_pg_error)?;
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
            return Ok(());
        }

        let state = self.inner.read().await;
        for doc_id in &req.doc_scope {
            let Some(stored) = state.documents.get(doc_id) else {
                return Err(AppError::validation(
                    "invalid_doc_scope",
                    format!("document {doc_id} does not exist"),
                ));
            };
            if stored.document.org_id != self.current_org_id() {
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

        Ok(())
    }
}
