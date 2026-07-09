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
            let results = rag_runtime.execute_tools(&self.auth, req.calls).await;
            return Ok(contracts::RuntimeExecuteResponse { results });
        }

        Err(AppError::validation(
            "rag_runtime_not_configured",
            "RAG runtime execute requires rag_runtime to be configured.",
        ))
    }
}
