//! Atomic tool dispatch — thin shim over [`agent_tools::tool_registry`].
//!
//! Kept for existing tests and call-sites; new code should use `tool_registry::dispatch_tool`.

use contracts::{ToolCall, ToolResult};

use agent_tools::tool_registry::{ToolDispatchContext, dispatch_tool};

/// Dispatch multiple atomic tool calls in parallel.
///
/// Backwards-compatible path — no policy enforcement.
pub async fn dispatch_atomic_tools(calls: Vec<ToolCall>) -> Vec<ToolResult> {
    dispatch_atomic_tools_with_provider(calls, None).await
}

/// Dispatch multiple atomic tool calls in parallel, with an optional web-search provider.
///
/// Backwards-compatible path — no policy enforcement.
pub async fn dispatch_atomic_tools_with_provider(
    calls: Vec<ToolCall>,
    search_provider: Option<&dyn avrag_search::SearchProvider>,
) -> Vec<ToolResult> {
    let futures = calls
        .into_iter()
        .map(|call| async move { dispatch_atomic_tool(&call, search_provider).await })
        .collect::<Vec<_>>();
    futures::future::join_all(futures).await
}

/// Dispatch with PolicyEnforcement (production path for non-RAG tools).
pub async fn dispatch_atomic_tools_with_enforcement(
    calls: Vec<ToolCall>,
    search_provider: Option<&dyn avrag_search::SearchProvider>,
    auth: Option<&contracts::auth_runtime::AuthContext>,
    session_id: Option<uuid::Uuid>,
    chat_persistence: Option<&dyn app_core::ChatPersistencePort>,
) -> Vec<ToolResult> {
    let futures = calls
        .into_iter()
        .map(|call| async move {
            dispatch_atomic_tool_with_enforcement(
                &call,
                search_provider,
                auth,
                session_id,
                chat_persistence,
            )
            .await
        })
        .collect::<Vec<_>>();
    futures::future::join_all(futures).await
}

/// Single tool call with policy enforcement — delegates to tool_registry.
pub async fn dispatch_atomic_tool_with_enforcement(
    call: &ToolCall,
    search_provider: Option<&dyn avrag_search::SearchProvider>,
    auth: Option<&contracts::auth_runtime::AuthContext>,
    session_id: Option<uuid::Uuid>,
    chat_persistence: Option<&dyn app_core::ChatPersistencePort>,
) -> ToolResult {
    let ctx = ToolDispatchContext {
        auth,
        session_id,
        doc_scope: &[],
        search_provider,
        rag_runtime: None,
        chat_persistence,
        enforce_policy: true,
    };
    dispatch_tool(call, &ctx).await
}

/// Backwards-compatible single-tool dispatch (no enforcement).
pub async fn dispatch_atomic_tool(
    call: &ToolCall,
    search_provider: Option<&dyn avrag_search::SearchProvider>,
) -> ToolResult {
    let start = std::time::Instant::now();
    let ctx = ToolDispatchContext {
        auth: None,
        session_id: None,
        doc_scope: &[],
        search_provider,
        rag_runtime: None,
        chat_persistence: None,
        enforce_policy: false,
    };
    let result = dispatch_tool(call, &ctx).await;
    let elapsed_ms = start.elapsed().as_millis() as f64;

    let status_str = match result.status {
        contracts::ToolStatus::Ok => "ok",
        contracts::ToolStatus::Error => "error",
        contracts::ToolStatus::NotFound => "not_found",
        contracts::ToolStatus::NotImplemented => "not_implemented",
        contracts::ToolStatus::Timeout => "timeout",
    };

    telemetry::prometheus::observe_agent_tool_call(&call.tool, status_str, elapsed_ms);
    result
}

