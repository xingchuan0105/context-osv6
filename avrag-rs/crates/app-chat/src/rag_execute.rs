use common::AppError;

use crate::context::{map_anyhow_error, ChatContext};

impl ChatContext {
    pub async fn execute_rag_execute_plan(
        &self,
        req: contracts::ExecutePlanRequest,
    ) -> Result<contracts::ExecutePlanResponse, AppError> {
        req.validate()
            .map_err(|error| AppError::validation("invalid_execute_plan", error.to_string()))?;
        self.validate_execute_plan_doc_scope(&req).await?;

        if let Some(rag_runtime) = self.orchestrator.rag_runtime() {
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
        req: contracts::RuntimeExecuteRequest,
    ) -> Result<contracts::RuntimeExecuteResponse, AppError> {
        if req.calls.is_empty() {
            return Err(AppError::validation(
                "invalid_calls",
                "calls must not be empty",
            ));
        }

        if let Some(rag_runtime) = self.orchestrator.rag_runtime() {
            let results = rag_runtime.execute_tools(&self.auth, req.calls).await;
            return Ok(contracts::RuntimeExecuteResponse { results });
        }

        Err(AppError::validation(
            "rag_runtime_not_configured",
            "RAG runtime execute requires rag_runtime to be configured.",
        ))
    }

    async fn validate_execute_plan_doc_scope(
        &self,
        req: &contracts::ExecutePlanRequest,
    ) -> Result<(), AppError> {
        if req.doc_scope.is_empty() {
            return Err(AppError::validation(
                "invalid_doc_scope",
                "doc_scope must not be empty",
            ));
        }

        self.validate_rag_doc_scope(&req.doc_scope).await
    }
}
