use common::AppError;

use crate::context::ChatContext;

impl ChatContext {
    /// ADR 0006 §5: product execute-plan path is retired. Prefer AgentLoop + ToolCall
    /// via `/api/v1/chat`. HTTP surface returns 410; this method mirrors that.
    pub async fn execute_rag_execute_plan(
        &self,
        _req: contracts::ExecutePlanRequest,
    ) -> Result<contracts::ExecutePlanResponse, AppError> {
        tracing::warn!(
            target: "adr0006_execute_plan",
            "deprecated /rag/execute-plan product path invoked"
        );
        telemetry::prometheus::record_dependency_failure("execute_plan_deprecated");
        Err(AppError::gone(
            "execute_plan_gone",
            "POST /api/v1/rag/execute-plan has been removed (ADR 0006). \
             Use chat AgentLoop + ToolCall retrieval instead.",
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
}
