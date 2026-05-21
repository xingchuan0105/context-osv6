//! Atomic tool executor for the UnifiedAgent.
//!
//! Dispatches calculator, code_interpreter, weather_query, and web_search tool calls
//! via the `SkillRegistry` so they can be used from any agent mode without
//! hard-coding the dispatch table.
//!
//! v5: All dispatch paths now run through `PolicyEnforcer` (standard rules) when
//! an `auth` context is provided.  The legacy no-auth paths use a permissive
//! enforcer so that existing tests and call-sites continue to work.

use common::{ToolCall, ToolResult, ToolStatus};

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
) -> Vec<ToolResult> {
    let futures = calls
        .into_iter()
        .map(|call| async move {
            dispatch_atomic_tool_with_enforcement(&call, search_provider, auth).await
        })
        .collect::<Vec<_>>();
    futures::future::join_all(futures).await
}

/// v5 path: dispatch a single tool call with PolicyEnforcement.
pub async fn dispatch_atomic_tool_with_enforcement(
    call: &ToolCall,
    search_provider: Option<&dyn avrag_search::SearchProvider>,
    auth: Option<&avrag_auth::AuthContext>,
) -> ToolResult {
    // 1. Policy check via CapabilityRegistry
    let registry = crate::agents::capability::CapabilityRegistry::standard_cached();
    if let Some(meta) = registry.tool(&call.tool) {
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
            _ => {} // LogOnly / MaskOutput — allow through for now
        }
    }

    // 2. Execute via SkillRegistry with retry
    let skill_registry = crate::agents::skills::registry::builtin_registry_cached();
    let ctx = crate::agents::skills::ExecutionContext::new(search_provider);

    let retry_policy = registry
        .tool(&call.tool)
        .map(|m| m.retry_policy.clone())
        .unwrap_or_default();

    execute_with_retry(
        || async {
            skill_registry.execute(&call.tool, &call.args, &ctx).await
        },
        &retry_policy,
    )
    .await
}

/// Execute an async operation with exponential-backoff retry.
///
/// - Non-idempotent tools are never retried.
/// - Only `ToolStatus::Error` and `Timeout` trigger retry.
/// - `NotFound` / `NotImplemented` are treated as terminal.
async fn execute_with_retry<F, Fut>(op: F, policy: &crate::agents::capability::RetryPolicy) -> ToolResult
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

        backoff = ((backoff as f64 * policy.backoff_multiplier) as u64)
            .min(policy.max_backoff_ms);
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
        common::ToolStatus::Ok => "ok",
        common::ToolStatus::Error => "error",
        common::ToolStatus::NotFound => "not_found",
        common::ToolStatus::NotImplemented => "not_implemented",
        common::ToolStatus::Timeout => "timeout",
    };

    telemetry::prometheus::observe_agent_tool_call(&call.tool, status_str, elapsed_ms);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::ToolStatus;

    fn tool_call(tool: &str, args: serde_json::Value) -> ToolCall {
        ToolCall {
            tool: tool.to_string(),
            version: "1.0".to_string(),
            args,
        }
    }

    // -----------------------------------------------------------------------
    // Calculator
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_calculator_basic() {
        let call = tool_call("calculator", serde_json::json!({"expression": "1 + 2 * 3"}));
        let result = dispatch_atomic_tool(&call, None).await;
        assert_eq!(result.status, ToolStatus::Ok);
        let data = result.data.unwrap();
        assert_eq!(data["result"].as_f64().unwrap(), 7.0);
    }

    #[tokio::test]
    async fn test_calculator_missing_expression() {
        let call = tool_call("calculator", serde_json::json!({}));
        let result = dispatch_atomic_tool(&call, None).await;
        assert_eq!(result.status, ToolStatus::Error);
        let data = result.data.unwrap();
        assert!(data["error"].as_str().unwrap().contains("missing expression"));
    }

    #[tokio::test]
    async fn test_calculator_trigonometry() {
        let call = tool_call("calculator", serde_json::json!({"expression": "sin(pi/2)"}));
        let result = dispatch_atomic_tool(&call, None).await;
        assert_eq!(result.status, ToolStatus::Ok);
        let data = result.data.unwrap();
        assert!(data["result"].as_f64().unwrap() > 0.99);
    }

    #[tokio::test]
    async fn test_calculator_division_by_zero() {
        let call = tool_call("calculator", serde_json::json!({"expression": "1/0"}));
        let result = dispatch_atomic_tool(&call, None).await;
        assert_eq!(result.status, ToolStatus::Error);
    }

    #[tokio::test]
    async fn test_calculator_invalid_expression() {
        let call = tool_call("calculator", serde_json::json!({"expression": "1 + * 2"}));
        let result = dispatch_atomic_tool(&call, None).await;
        assert_eq!(result.status, ToolStatus::Error);
    }

    // -----------------------------------------------------------------------
    // Code Interpreter
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_code_interpreter_simple() {
        let call = tool_call(
            "code_interpreter",
            serde_json::json!({"code": "print(1 + 2)"}),
        );
        let result = dispatch_atomic_tool(&call, None).await;
        assert_eq!(result.status, ToolStatus::Ok);
        let data = result.data.unwrap();
        assert!(data["stdout"].as_str().unwrap().contains("3"));
        assert!(data["success"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_code_interpreter_missing_code() {
        let call = tool_call("code_interpreter", serde_json::json!({}));
        let result = dispatch_atomic_tool(&call, None).await;
        assert_eq!(result.status, ToolStatus::Error);
        let data = result.data.unwrap();
        assert!(data["error"].as_str().unwrap().contains("missing code"));
    }

    #[tokio::test]
    async fn test_code_interpreter_stderr() {
        let call = tool_call(
            "code_interpreter",
            serde_json::json!({"code": "raise ValueError('error')"}),
        );
        let result = dispatch_atomic_tool(&call, None).await;
        assert_eq!(result.status, ToolStatus::Ok);
        let data = result.data.unwrap();
        assert!(data["stderr"].as_str().unwrap().contains("ValueError"));
        assert!(data["success"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_code_interpreter_exception() {
        let call = tool_call(
            "code_interpreter",
            serde_json::json!({"code": "1/0"}),
        );
        let result = dispatch_atomic_tool(&call, None).await;
        assert_eq!(result.status, ToolStatus::Ok);
        let data = result.data.unwrap();
        assert!(data["stderr"].as_str().unwrap().contains("ZeroDivisionError"));
        assert!(data["success"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_code_interpreter_result_field() {
        let call = tool_call(
            "code_interpreter",
            serde_json::json!({"code": "x = 42"}),
        );
        let result = dispatch_atomic_tool(&call, None).await;
        assert_eq!(result.status, ToolStatus::Ok);
        let data = result.data.unwrap();
        assert!(data["result"].is_null() || data["result"] == serde_json::Value::Null);
    }

    // -----------------------------------------------------------------------
    // Weather Query
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_weather_query_missing_location() {
        let call = tool_call("weather_query", serde_json::json!({}));
        let result = dispatch_atomic_tool(&call, None).await;
        assert_eq!(result.status, ToolStatus::Error);
        let data = result.data.unwrap();
        assert!(data["error"].as_str().unwrap().contains("missing location"));
    }

    // -----------------------------------------------------------------------
    // Web Search
    // -----------------------------------------------------------------------

    struct FakeSearchProvider;

    #[async_trait::async_trait]
    impl avrag_search::SearchProvider for FakeSearchProvider {
        async fn execute_search(
            &self,
            query: &str,
            _vertical: Option<&str>,
        ) -> anyhow::Result<avrag_search::SearchResponse> {
            Ok(avrag_search::SearchResponse {
                query_type: "test".to_string(),
                sub_queries: vec![query.to_string()],
                results: vec![avrag_search::SearchResult {
                    title: format!("Result for {query}"),
                    url: format!("https://example.com/search?q={query}"),
                    snippet: "test snippet".to_string(),
                    citation_index: Some(1),
                }],
                synthesized_answer: "test answer".to_string(),
                llm_usage: None,
            })
        }
    }

    #[tokio::test]
    async fn test_web_search_basic() {
        let call = tool_call("web_search", serde_json::json!({"query": "rust lang"}));
        let provider = FakeSearchProvider;
        let result = dispatch_atomic_tool(&call, Some(&provider)).await;
        assert_eq!(result.status, ToolStatus::Ok);
        let data = result.data.unwrap();
        assert_eq!(data["query_type"], "test");
        assert_eq!(data["sub_queries"].as_array().unwrap().len(), 1);
        let results = data["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["title"], "Result for rust lang");
    }

    #[tokio::test]
    async fn test_web_search_no_provider() {
        let call = tool_call("web_search", serde_json::json!({"query": "rust"}));
        let result = dispatch_atomic_tool(&call, None).await;
        assert_eq!(result.status, ToolStatus::Error);
        let data = result.data.unwrap();
        assert!(data["error"].as_str().unwrap().contains("not available"));
    }

    #[tokio::test]
    async fn test_web_search_missing_query() {
        let call = tool_call("web_search", serde_json::json!({}));
        let provider = FakeSearchProvider;
        let result = dispatch_atomic_tool(&call, Some(&provider)).await;
        assert_eq!(result.status, ToolStatus::Error);
        let data = result.data.unwrap();
        assert!(data["error"].as_str().unwrap().contains("missing query"));
    }

    #[tokio::test]
    async fn test_web_search_with_vertical() {
        let call = tool_call(
            "web_search",
            serde_json::json!({"query": "news", "vertical": "news"}),
        );
        let provider = FakeSearchProvider;
        let result = dispatch_atomic_tool(&call, Some(&provider)).await;
        assert_eq!(result.status, ToolStatus::Ok);
    }

    // -----------------------------------------------------------------------
    // Batch dispatch
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_dispatch_multiple_tools_parallel() {
        let calls = vec![
            tool_call("calculator", serde_json::json!({"expression": "1+1"})),
            tool_call("calculator", serde_json::json!({"expression": "2*3"})),
        ];
        let results = dispatch_atomic_tools(calls).await;
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].data.as_ref().unwrap()["result"].as_f64().unwrap(), 2.0);
        assert_eq!(results[1].data.as_ref().unwrap()["result"].as_f64().unwrap(), 6.0);
    }

    #[tokio::test]
    async fn test_dispatch_atomic_tools_with_provider() {
        let calls = vec![
            tool_call("calculator", serde_json::json!({"expression": "3+3"})),
            tool_call("web_search", serde_json::json!({"query": "test"})),
        ];
        let provider = FakeSearchProvider;
        let results = dispatch_atomic_tools_with_provider(calls, Some(&provider)).await;
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].status, ToolStatus::Ok);
        assert_eq!(results[0].tool, "calculator");
        assert_eq!(results[1].status, ToolStatus::Ok);
        assert_eq!(results[1].tool, "web_search");
    }

    // -----------------------------------------------------------------------
    // Unsupported tool
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_unsupported_tool() {
        let call = tool_call("unknown_tool", serde_json::json!({}));
        let result = dispatch_atomic_tool(&call, None).await;
        assert_eq!(result.status, ToolStatus::NotImplemented);
    }

    // -----------------------------------------------------------------------
    // PolicyEnforcement integration
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_enforcement_blocks_web_search_without_external_network_perm() {
        let call = tool_call("web_search", serde_json::json!({"query": "test"}));
        let auth = avrag_auth::AuthContext::new(
            avrag_auth::OrgId::new(uuid::Uuid::nil()),
            avrag_auth::SubjectKind::User,
        );
        let result = dispatch_atomic_tool_with_enforcement(&call, None, Some(&auth)).await;
        assert_eq!(result.status, ToolStatus::Error);
        let data = result.data.unwrap();
        assert!(data["error"].as_str().unwrap().contains("external network"));
    }

    #[tokio::test]
    async fn test_enforcement_allows_web_search_with_external_network_perm() {
        let call = tool_call("web_search", serde_json::json!({"query": "test"}));
        let auth = avrag_auth::AuthContext::new(
            avrag_auth::OrgId::new(uuid::Uuid::nil()),
            avrag_auth::SubjectKind::User,
        )
        .grant("external_network");
        let provider = FakeSearchProvider;
        let result = dispatch_atomic_tool_with_enforcement(&call, Some(&provider), Some(&auth)).await;
        assert_eq!(result.status, ToolStatus::Ok);
    }

    #[tokio::test]
    async fn test_enforcement_blocks_code_interpreter_without_code_execution_perm() {
        let call = tool_call("code_interpreter", serde_json::json!({"code": "1+1"}));
        let auth = avrag_auth::AuthContext::new(
            avrag_auth::OrgId::new(uuid::Uuid::nil()),
            avrag_auth::SubjectKind::User,
        );
        let result = dispatch_atomic_tool_with_enforcement(&call, None, Some(&auth)).await;
        assert_eq!(result.status, ToolStatus::Error);
        let data = result.data.unwrap();
        assert!(data["error"].as_str().unwrap().contains("code execution"));
    }

    #[tokio::test]
    async fn test_legacy_path_is_permissive_no_auth() {
        let call = tool_call("web_search", serde_json::json!({"query": "test"}));
        let provider = FakeSearchProvider;
        // Legacy dispatch_atomic_tool (no auth) should use permissive enforcer
        let result = dispatch_atomic_tool(&call, Some(&provider)).await;
        assert_eq!(result.status, ToolStatus::Ok);
    }

    // -----------------------------------------------------------------------
    // Retry policy
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_retry_succeeds_on_second_attempt() {
        use crate::agents::capability::RetryPolicy;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let counter = std::sync::Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let policy = RetryPolicy {
            max_retries: 3,
            backoff_ms: 1,
            backoff_multiplier: 1.0,
            max_backoff_ms: 10,
            idempotent: true,
            idempotency_key_header: None,
        };

        let result = execute_with_retry(
            move || {
                let c = c.clone();
                async move {
                    let n = c.fetch_add(1, Ordering::SeqCst);
                    if n == 0 {
                        ToolResult {
                            tool: "x".to_string(),
                            version: "1.0".to_string(),
                            status: ToolStatus::Error,
                            data: Some(serde_json::json!({"error": "transient"})),
                            trace: None,
                        }
                    } else {
                        ToolResult {
                            tool: "x".to_string(),
                            version: "1.0".to_string(),
                            status: ToolStatus::Ok,
                            data: Some(serde_json::json!({"ok": true})),
                            trace: None,
                        }
                    }
                }
            },
            &policy,
        )
        .await;

        assert_eq!(result.status, ToolStatus::Ok);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_non_idempotent_skips_retry() {
        use crate::agents::capability::RetryPolicy;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let counter = std::sync::Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let policy = RetryPolicy {
            max_retries: 3,
            backoff_ms: 1,
            backoff_multiplier: 1.0,
            max_backoff_ms: 10,
            idempotent: false, // non-idempotent
            idempotency_key_header: None,
        };

        let result = execute_with_retry(
            move || {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    ToolResult {
                        tool: "x".to_string(),
                        version: "1.0".to_string(),
                        status: ToolStatus::Error,
                        data: Some(serde_json::json!({"error": "boom"})),
                        trace: None,
                    }
                }
            },
            &policy,
        )
        .await;

        assert_eq!(result.status, ToolStatus::Error);
        assert_eq!(counter.load(Ordering::SeqCst), 1); // no retry
    }

    #[tokio::test]
    async fn test_not_found_is_terminal_no_retry() {
        use crate::agents::capability::RetryPolicy;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let counter = std::sync::Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let policy = RetryPolicy {
            max_retries: 3,
            backoff_ms: 1,
            backoff_multiplier: 1.0,
            max_backoff_ms: 10,
            idempotent: true,
            idempotency_key_header: None,
        };

        let result = execute_with_retry(
            move || {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    ToolResult {
                        tool: "x".to_string(),
                        version: "1.0".to_string(),
                        status: ToolStatus::NotFound,
                        data: None,
                        trace: None,
                    }
                }
            },
            &policy,
        )
        .await;

        assert_eq!(result.status, ToolStatus::NotFound);
        assert_eq!(counter.load(Ordering::SeqCst), 1); // no retry for NotFound
    }
}
