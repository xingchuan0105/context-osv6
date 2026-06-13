//! Core atomic tool dispatch — policy enforcement, retry, and skill registry execution.

use contracts::{ToolCall, ToolResult, ToolStatus};

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

/// v5 path: dispatch with PolicyEnforcement.
///
/// When `auth` is `Some`, the standard `PolicyEnforcer` rules are applied
/// (risk level, permissions, external deps).  Denied calls return a
/// `ToolResult` with `status = Error` instead of panicking.
pub async fn dispatch_atomic_tools_with_enforcement(
    calls: Vec<ToolCall>,
    search_provider: Option<&dyn avrag_search::SearchProvider>,
    auth: Option<&avrag_auth::AuthContext>,
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

/// v5 path: dispatch a single tool call with PolicyEnforcement.
pub async fn dispatch_atomic_tool_with_enforcement(
    call: &ToolCall,
    search_provider: Option<&dyn avrag_search::SearchProvider>,
    auth: Option<&avrag_auth::AuthContext>,
    session_id: Option<uuid::Uuid>,
    chat_persistence: Option<&dyn app_core::ChatPersistencePort>,
) -> ToolResult {
    // 1. Policy check via non-prompt runtime metadata.
    let registry = crate::agents::capability::CapabilityRegistry::standard_cached();
    let runtime_meta = runtime_tool_metadata(&call.tool);
    if let Some(meta) = registry.tool(&call.tool).or_else(|| runtime_meta.as_ref()) {
        let enforcer = crate::agents::capability::PolicyEnforcer::new(
            crate::agents::capability::standard_rules(),
        );
        match enforcer.evaluate(meta, auth) {
            crate::agents::capability::EnforcementAction::Allow => {}
            crate::agents::capability::EnforcementAction::Deny { reason } => {
                return ToolResult {
                    tool: call.tool.clone(),
                    version: call.version.clone(),
                    status: ToolStatus::Error,
                    data: Some(serde_json::json!({ "error": reason })),
                    trace: None,
                };
            }
            crate::agents::capability::EnforcementAction::RequireApproval { reason } => {
                return ToolResult {
                    tool: call.tool.clone(),
                    version: call.version.clone(),
                    status: ToolStatus::Error,
                    data: Some(serde_json::json!({
                        "error": reason,
                        "requires_approval": true,
                    })),
                    trace: None,
                };
            }
            _ => {} // LogOnly / MaskOutput — allow through for now
        }
    }

    // 2. Execute via SkillRegistry with retry
    let skill_registry = crate::agents::skills::registry::builtin_registry_cached();
    let ctx = crate::agents::skills::ExecutionContext::with_memory(
        search_provider,
        auth,
        session_id,
        chat_persistence,
    );

    let retry_policy = registry
        .tool(&call.tool)
        .or_else(|| runtime_meta.as_ref())
        .map(|m| m.retry_policy.clone())
        .unwrap_or_default();

    execute_with_retry(
        || async { skill_registry.execute(&call.tool, &call.args, &ctx).await },
        &retry_policy,
    )
    .await
}

fn runtime_tool_metadata(id: &str) -> Option<crate::agents::capability::ToolMetadata> {
    use crate::agents::capability::{
        ActivationPhase, Permission, RetryPolicy, RiskLevel, ToolMetadata,
    };

    let (description, risk_level, permissions) = match id {
        "web_search" => (
            "Search the public web",
            RiskLevel::High,
            vec![Permission::ExternalNetwork],
        ),
        "web_fetch" => (
            "Fetch a public web page",
            RiskLevel::High,
            vec![Permission::ExternalNetwork],
        ),
        "code_interpreter" => (
            "Execute code in a sandbox",
            RiskLevel::High,
            vec![Permission::CodeExecution],
        ),
        "calculator" => (
            "Evaluate a mathematical expression",
            RiskLevel::Low,
            Vec::new(),
        ),
        "weather_query" => ("Query weather data", RiskLevel::Low, Vec::new()),
        _ => return None,
    };

    Some(ToolMetadata {
        id: id.to_string(),
        version: "1.0.0".to_string(),
        owner: "runtime".to_string(),
        description: description.to_string(),
        input_schema: serde_json::Value::Null,
        output_schema: serde_json::Value::Null,
        risk_level,
        permissions,
        external_deps: Vec::new(),
        deprecation: None,
        retry_policy: RetryPolicy::default(),
        activation_phase: ActivationPhase::PlanAndEvaluate,
        applicable_strategies: Vec::new(),
    })
}

/// Execute an async operation with exponential-backoff retry.
///
/// - Non-idempotent tools are never retried.
/// - Only `ToolStatus::Error` and `Timeout` trigger retry.
/// - `NotFound` / `NotImplemented` are treated as terminal.
pub(crate) async fn execute_with_retry<F, Fut>(
    op: F,
    policy: &crate::agents::capability::RetryPolicy,
) -> ToolResult
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = ToolResult>,
{
    let mut result = op().await;
    if result.status == ToolStatus::Ok || !policy.idempotent {
        return result;
    }

    let mut backoff = policy.backoff_ms;
    for _attempt in 0..policy.max_retries {
        if !matches!(result.status, ToolStatus::Error | ToolStatus::Timeout) {
            return result;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(backoff)).await;
        result = op().await;
        if result.status == ToolStatus::Ok {
            return result;
        }

        backoff = ((backoff as f64 * policy.backoff_multiplier) as u64).min(policy.max_backoff_ms);
    }

    result
}

/// Backwards-compatible single-tool dispatch (no enforcement).
///
/// Tests and legacy call-sites use this path.  Policy enforcement is
/// applied only when calling `dispatch_atomic_tool_with_enforcement`.
pub async fn dispatch_atomic_tool(
    call: &ToolCall,
    search_provider: Option<&dyn avrag_search::SearchProvider>,
) -> ToolResult {
    let start = std::time::Instant::now();
    let registry = crate::agents::skills::registry::builtin_registry_cached();
    let ctx = crate::agents::skills::ExecutionContext::new(search_provider);
    let result = registry.execute(&call.tool, &call.args, &ctx).await;
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
