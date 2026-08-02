use common::AppError;

use crate::context::ChatContext;

impl ChatContext {
    /// ToolCall runtime execute (not the retired execute-plan HTTP surface).
    pub async fn execute_runtime_tools(
        &self,
        req: contracts::RuntimeExecuteRequest,
    ) -> Result<contracts::RuntimeExecuteResponse, AppError> {
        if req.calls.is_empty() {
            return Err(AppError::validation(
                "invalid_calls",
                "calls must not be empty",
            ));
        }

        if let Some(rag_runtime) = self.orchestrator.rag_runtime() {
            let scope = self.workspace_doc_scope().await?;
            let results = futures::future::join_all(req.calls.into_iter().map(|call| {
                avrag_rag_core::runtime::scoped_rag_dispatch::dispatch_scoped(
                    rag_runtime,
                    &self.auth,
                    call,
                    &scope,
                )
            }))
            .await;
            return Ok(contracts::RuntimeExecuteResponse { results });
        }

        Err(AppError::validation(
            "rag_runtime_not_configured",
            "RAG runtime execute requires rag_runtime to be configured.",
        ))
    }

    /// Derive the enforcement scope from the authenticated workspace: all
    /// completed document ids in `auth.workspace_id()`.
    ///
    /// Empty only when auth carries no workspace scope (scope is then unenforced
    /// upstream). A missing/errored document store fails closed instead of
    /// degrading to an unenforced empty scope.
    async fn workspace_doc_scope(&self) -> Result<Vec<String>, AppError> {
        let Some(workspace_id) = self.auth.workspace_id() else {
            return Ok(Vec::new());
        };
        self.documents
            .completed_workspace_doc_ids(&self.auth, &self.storage, &workspace_id.to_string())
            .await
    }
}
