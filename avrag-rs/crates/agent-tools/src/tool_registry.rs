//! Single tool dispatch surface backed by [`crate::catalog::ToolCatalog`].
//!
//! All ReActLoop tool execution goes through [`dispatch_tool`].

use std::sync::Arc;

use app_core::ChatPersistencePort;
use contracts::{ToolCall, ToolResult, ToolStatus};

use crate::catalog::{ToolCatalog, ToolExecKind};
use crate::rag_bridge::dispatch_rag_tool;

/// LLM-facing reject hints from `avrag-rs/prompts/loop/*.md` (prompts-in-md).
macro_rules! loop_prompt {
    ($file:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../prompts/loop/",
            $file
        ))
    };
}

fn subst_prompt(template: &str, pairs: &[(&str, &str)]) -> String {
    let mut out = template.trim().to_string();
    for (k, v) in pairs {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

/// Tool ids handled by the RAG runtime (catalog + legacy helper).
pub fn is_rag_tool(tool: &str) -> bool {
    ToolCatalog::standard_cached().is_rag(tool)
}

/// Runtime dependencies for one tool call.
pub struct ToolDispatchContext<'a> {
    pub auth: Option<&'a contracts::auth_runtime::AuthContext>,
    pub session_id: Option<uuid::Uuid>,
    pub doc_scope: &'a [String],
    pub search_provider: Option<&'a dyn avrag_search::SearchProvider>,
    pub rag_runtime: Option<&'a avrag_rag_core::RagRuntime>,
    pub chat_persistence: Option<&'a dyn ChatPersistencePort>,
    /// When true, run CapabilityRegistry policy enforcement (production loop).
    pub enforce_policy: bool,
    pub client_ip: Option<&'a str>,
    pub client_local_time: Option<&'a str>,
    pub client_timezone: Option<&'a str>,
}

/// Codegen Python SDK method names — never native tool schema ids.
/// Reject bare tool_calls so the model retries with a `<code language="python">` block.
pub const CODEGEN_SDK_METHOD_NAMES: &[&str] = &[
    // SaC canonical (2026-07-30)
    "dense",
    "lexical",
    "grep",
    "web",
    "fetch",
    "doc_profile",
    "doc_summary",
    "history",
    "user_profile",
    "save",
    "load",
    // Temporary / legacy aliases still rejected if invented as native tools.
    "dense_search",
    "lexical_search",
    "graph_search",
    "chunk_fetch",
    "doc_chunks",
    "doc_scan",
    "read_lines",
];

/// Former native retrieval tools now SaC-only (A1). Catalog may still hold
/// them for host-internal fallback via `dispatch_rag_fallback` / SearchProvider,
/// but **LLM function-calling must not execute them**.
pub const SAC_SUPERSEDED_NATIVE_TOOLS: &[&str] = &[
    "dense_retrieval",
    "lexical_retrieval",
    "graph_retrieval",
    "index_lookup",
    "doc_summary",
    "doc_metadata",
    "doc_profile",
    "doc_scan",
    "doc_grep",
    "doc_read_lines",
    "web_search",
    "web_fetch",
];

/// True when `tool` is a codegen SDK method name, not a ToolCatalog entry.
pub fn is_codegen_sdk_method_as_native_tool(tool_name: &str) -> bool {
    CODEGEN_SDK_METHOD_NAMES.iter().any(|n| *n == tool_name)
}

/// True when a catalog tool id is retrieval/web and must only be used via SaC SDK.
pub fn is_sac_superseded_native_tool(tool_name: &str) -> bool {
    SAC_SUPERSEDED_NATIVE_TOOLS.iter().any(|n| *n == tool_name)
}

/// Synthetic error result when the model invents native tool_calls for SDK methods.
pub fn reject_codegen_method_as_native_tool(tool_name: &str) -> ToolResult {
    let hint = subst_prompt(
        loop_prompt!("codegen-method-as-native-rejection.tmpl.md"),
        &[("tool", tool_name)],
    );
    ToolResult {
        tool: tool_name.to_string(),
        version: "1.0".to_string(),
        status: ToolStatus::Error,
        data: Some(serde_json::json!({
            "error": "not_a_native_tool",
            "tool": tool_name,
            "hint": hint,
        })),
        trace: None,
    }
}

/// Reject LLM native tool_calls for tools that moved into the SaC sandbox SDK (A1).
pub fn reject_sac_superseded_native_tool(tool_name: &str) -> ToolResult {
    let sac_hint = match tool_name {
        "dense_retrieval" => "await client.dense(query)",
        "lexical_retrieval" => "await client.lexical(query)",
        "web_search" => "await client.web(query)",
        "web_fetch" => "await client.fetch(url)",
        "doc_summary" => "await client.doc_summary(...)",
        "doc_profile" => "await client.doc_profile(...)",
        "doc_grep" | "doc_scan" => "await client.grep(pattern, ...)",
        "doc_read_lines" => "await client.grep(..., context=N)  # read_lines removed",
        "graph_retrieval" => "await client.lexical(query)  # graph is bound to lexical",
        _ => "await client.<method>(...)  # see codegen/search skill",
    };
    let hint = subst_prompt(
        loop_prompt!("sac-superseded-rejection.tmpl.md"),
        &[("tool", tool_name), ("sac_hint", sac_hint)],
    );
    ToolResult {
        tool: tool_name.to_string(),
        version: "1.0".to_string(),
        status: ToolStatus::Error,
        data: Some(serde_json::json!({
            "error": "sac_sdk_only",
            "tool": tool_name,
            "hint": hint,
        })),
        trace: None,
    }
}

/// Canonical tool execute entry used by ReActLoop and all call sites.
pub async fn dispatch_tool(call: &ToolCall, ctx: &ToolDispatchContext<'_>) -> ToolResult {
    // B4: hard gate at the single execute entry (was iteration-local).
    if is_codegen_sdk_method_as_native_tool(&call.tool) {
        tracing::warn!(
            tool = %call.tool,
            "rejecting codegen SDK method issued as native tool_call"
        );
        return reject_codegen_method_as_native_tool(&call.tool);
    }
    // A1: retrieval/web only via SaC sandbox — not LLM function calling.
    if is_sac_superseded_native_tool(&call.tool) {
        tracing::warn!(
            tool = %call.tool,
            "rejecting SaC-superseded native tool_call (use sandbox client)"
        );
        return reject_sac_superseded_native_tool(&call.tool);
    }

    let catalog = ToolCatalog::standard_cached();
    let Some(registered) = catalog.get(&call.tool) else {
        return ToolResult {
            tool: call.tool.clone(),
            version: call.version.clone(),
            status: ToolStatus::NotImplemented,
            data: Some(serde_json::json!({ "error": format!("unknown tool: {}", call.tool) })),
            trace: None,
        };
    };

    match registered.exec {
        ToolExecKind::Rag => dispatch_rag(call, ctx).await,
        ToolExecKind::Skill => dispatch_skill(call, ctx, &registered.meta).await,
    }
}

async fn dispatch_rag(call: &ToolCall, ctx: &ToolDispatchContext<'_>) -> ToolResult {
    let (Some(runtime), Some(auth)) = (ctx.rag_runtime, ctx.auth) else {
        return ToolResult {
            tool: call.tool.clone(),
            version: call.version.clone(),
            status: ToolStatus::NotImplemented,
            data: Some(serde_json::json!({
                "error": if ctx.rag_runtime.is_none() {
                    "rag runtime not configured"
                } else {
                    "auth context required for rag tools"
                }
            })),
            trace: None,
        };
    };
    dispatch_rag_tool(runtime, auth, call, ctx.doc_scope).await
}

/// Resolve tool metadata from the unified catalog.
pub fn tool_meta(tool: &str) -> Option<crate::capability::ToolMetadata> {
    ToolCatalog::standard_cached().tool_meta(tool).cloned()
}

async fn dispatch_skill(
    call: &ToolCall,
    ctx: &ToolDispatchContext<'_>,
    meta: &crate::capability::ToolMetadata,
) -> ToolResult {
    if ctx.enforce_policy {
        let enforcer =
            crate::capability::PolicyEnforcer::new(crate::capability::standard_rules());
        match enforcer.evaluate(meta, ctx.auth) {
            crate::capability::EnforcementAction::Allow => {}
            crate::capability::EnforcementAction::Deny { reason } => {
                return ToolResult {
                    tool: call.tool.clone(),
                    version: call.version.clone(),
                    status: ToolStatus::Error,
                    data: Some(serde_json::json!({ "error": reason })),
                    trace: None,
                };
            }
            crate::capability::EnforcementAction::RequireApproval { reason } => {
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
            _ => {}
        }
    }

    let skill_registry = ToolCatalog::standard_cached().skill_registry();
    let exec_ctx = crate::skills::ExecutionContext::with_memory(
        ctx.search_provider,
        ctx.auth,
        ctx.session_id,
        ctx.chat_persistence,
    )
    .with_client_context(
        ctx.client_ip.map(str::to_string),
        ctx.client_local_time.map(str::to_string),
        ctx.client_timezone.map(str::to_string),
    );

    execute_with_retry(
        || async { skill_registry.execute(&call.tool, &call.args, &exec_ctx).await },
        &meta.retry_policy,
    )
    .await
}

/// Execute an async operation with exponential-backoff retry.
pub async fn execute_with_retry<F, Fut>(
    op: F,
    policy: &crate::capability::RetryPolicy,
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

/// Convenience for call sites that only have Arc-wrapped deps (ReActLoop).
pub struct OwnedToolDeps {
    pub search_executor: Option<Arc<dyn avrag_search::SearchProvider>>,
    pub rag_runtime: Option<Arc<avrag_rag_core::RagRuntime>>,
    pub chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    pub client_ip: Option<String>,
    pub client_local_time: Option<String>,
    pub client_timezone: Option<String>,
}

impl OwnedToolDeps {
    pub async fn dispatch(
        &self,
        call: &ToolCall,
        auth: &contracts::auth_runtime::AuthContext,
        doc_scope: &[String],
        session_id: Option<&str>,
    ) -> ToolResult {
        let session_uuid = session_id.and_then(|id| uuid::Uuid::parse_str(id).ok());
        let ctx = ToolDispatchContext {
            auth: Some(auth),
            session_id: session_uuid,
            doc_scope,
            search_provider: self.search_executor.as_deref(),
            rag_runtime: self.rag_runtime.as_deref(),
            chat_persistence: self.chat_persistence.as_deref(),
            enforce_policy: true,
            client_ip: self.client_ip.as_deref(),
            client_local_time: self.client_local_time.as_deref(),
            client_timezone: self.client_timezone.as_deref(),
        };
        dispatch_tool(call, &ctx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn call(tool: &str, args: serde_json::Value) -> ToolCall {
        ToolCall {
            tool: tool.into(),
            version: "1.0".into(),
            args,
        }
    }

    fn ctx_permissive<'a>(
        search: Option<&'a dyn avrag_search::SearchProvider>,
    ) -> ToolDispatchContext<'a> {
        ToolDispatchContext {
            auth: None,
            session_id: None,
            doc_scope: &[],
            search_provider: search,
            rag_runtime: None,
            chat_persistence: None,
            enforce_policy: false,
            client_ip: None,
            client_local_time: None,
            client_timezone: None,
        }
    }

    fn ctx_enforced<'a>(
        auth: Option<&'a contracts::auth_runtime::AuthContext>,
        search: Option<&'a dyn avrag_search::SearchProvider>,
    ) -> ToolDispatchContext<'a> {
        ToolDispatchContext {
            auth,
            session_id: None,
            doc_scope: &[],
            search_provider: search,
            rag_runtime: None,
            chat_persistence: None,
            enforce_policy: true,
            client_ip: None,
            client_local_time: None,
            client_timezone: None,
        }
    }

    struct FakeSearchProvider;

    #[async_trait::async_trait]
    impl avrag_search::SearchProvider for FakeSearchProvider {
        async fn execute_search(
            &self,
            query: &str,
            _vertical: Option<&str>,
        ) -> anyhow::Result<avrag_search::SearchResponse> {
            Ok(avrag_search::SearchResponse {
                query_type: "test".into(),
                sub_queries: vec![query.into()],
                results: vec![avrag_search::SearchResult {
                    title: format!("Result for {query}"),
                    url: format!("https://example.com/search?q={query}"),
                    snippet: "test snippet".into(),
                    citation_index: Some(1),
                }],
                synthesized_answer: "test answer".into(),
                llm_usage: None,
            })
        }
    }

    #[test]
    fn rag_tool_classification() {
        assert!(is_rag_tool("dense_retrieval"));
        assert!(is_rag_tool("doc_scan"));
        assert!(!is_rag_tool("calculator"));
        assert!(!is_rag_tool("web_search"));
    }

    #[test]
    fn codegen_sdk_method_names_rejected_as_native() {
        assert!(is_codegen_sdk_method_as_native_tool("dense"));
        assert!(is_codegen_sdk_method_as_native_tool("lexical"));
        assert!(is_codegen_sdk_method_as_native_tool("web"));
        assert!(is_codegen_sdk_method_as_native_tool("fetch"));
        assert!(is_codegen_sdk_method_as_native_tool("dense_search")); // legacy alias
        assert!(is_codegen_sdk_method_as_native_tool("grep"));
        assert!(is_codegen_sdk_method_as_native_tool("read_lines")); // still reject if invented
        assert!(!is_codegen_sdk_method_as_native_tool("dense_retrieval"));
        assert!(is_sac_superseded_native_tool("dense_retrieval"));
        assert!(is_sac_superseded_native_tool("web_search"));
        let r = reject_codegen_method_as_native_tool("dense");
        assert_eq!(r.status, ToolStatus::Error);
        let err = r.data.as_ref().and_then(|d| d.get("error")).and_then(|e| e.as_str());
        assert_eq!(err, Some("not_a_native_tool"));
        let r2 = reject_sac_superseded_native_tool("web_search");
        assert_eq!(r2.status, ToolStatus::Error);
        assert_eq!(
            r2.data.as_ref().and_then(|d| d.get("error")).and_then(|e| e.as_str()),
            Some("sac_sdk_only")
        );
        let hint = r2
            .data
            .as_ref()
            .and_then(|d| d.get("hint"))
            .and_then(|h| h.as_str())
            .unwrap_or("");
        assert!(hint.contains("web_search"), "hint: {hint}");
        assert!(hint.contains("client.web"), "hint: {hint}");
        // Parameterized: every SaC-superseded name is recognized.
        for name in SAC_SUPERSEDED_NATIVE_TOOLS {
            assert!(
                is_sac_superseded_native_tool(name),
                "missing SaC supersede for {name}"
            );
            let rej = reject_sac_superseded_native_tool(name);
            assert_eq!(rej.status, ToolStatus::Error);
            assert_eq!(
                rej.data
                    .as_ref()
                    .and_then(|d| d.get("error"))
                    .and_then(|e| e.as_str()),
                Some("sac_sdk_only")
            );
        }
    }

    #[tokio::test]
    async fn dispatch_tool_rejects_codegen_sdk_before_catalog() {
        let result = dispatch_tool(
            &call("dense_search", serde_json::json!({})),
            &ctx_permissive(None),
        )
        .await;
        assert_eq!(result.status, ToolStatus::Error);
        assert_eq!(
            result
                .data
                .as_ref()
                .and_then(|d| d.get("error"))
                .and_then(|e| e.as_str()),
            Some("not_a_native_tool")
        );
    }

    #[test]
    fn tool_meta_from_catalog() {
        let meta = tool_meta("calculator").expect("calculator meta");
        assert_eq!(meta.id, "calculator");
        let rag = tool_meta("dense_retrieval").expect("dense meta");
        assert_eq!(rag.owner, "rag-runtime");
    }

    #[tokio::test]
    async fn rag_native_is_rejected_as_sac_only() {
        let result = dispatch_tool(&call("dense_retrieval", serde_json::json!({})), &ctx_permissive(None))
            .await;
        assert_eq!(result.status, ToolStatus::Error);
        assert_eq!(
            result
                .data
                .as_ref()
                .and_then(|d| d.get("error"))
                .and_then(|e| e.as_str()),
            Some("sac_sdk_only")
        );
    }

    #[tokio::test]
    async fn unknown_tool_is_not_implemented() {
        let result =
            dispatch_tool(&call("no_such_tool", serde_json::json!({})), &ctx_permissive(None)).await;
        assert_eq!(result.status, ToolStatus::NotImplemented);
    }

    #[tokio::test]
    async fn calculator_via_dispatch_tool() {
        let result = dispatch_tool(
            &call("calculator", serde_json::json!({"expression": "1 + 2 * 3"})),
            &ctx_permissive(None),
        )
        .await;
        assert_eq!(result.status, ToolStatus::Ok);
        assert_eq!(result.data.unwrap()["result"].as_f64().unwrap(), 7.0);
    }

    #[tokio::test]
    async fn web_search_native_is_rejected_as_sac_only() {
        // A1: web_search is no longer an LLM-facing native tool (use client.web).
        let auth = contracts::auth_runtime::AuthContext::new(
            contracts::auth_runtime::UserId::new(uuid::Uuid::nil()),
            contracts::auth_runtime::SubjectKind::User,
        )
        .grant("external_network");
        let provider = FakeSearchProvider;
        let result = dispatch_tool(
            &call("web_search", serde_json::json!({"query": "test"})),
            &ctx_enforced(Some(&auth), Some(&provider)),
        )
        .await;
        assert_eq!(result.status, ToolStatus::Error);
        assert_eq!(
            result
                .data
                .as_ref()
                .and_then(|d| d.get("error"))
                .and_then(|e| e.as_str()),
            Some("sac_sdk_only")
        );
    }

    #[tokio::test]
    async fn permissive_path_also_rejects_web_search_native() {
        let provider = FakeSearchProvider;
        let result = dispatch_tool(
            &call("web_search", serde_json::json!({"query": "test"})),
            &ctx_permissive(Some(&provider)),
        )
        .await;
        assert_eq!(result.status, ToolStatus::Error);
        assert_eq!(
            result
                .data
                .as_ref()
                .and_then(|d| d.get("error"))
                .and_then(|e| e.as_str()),
            Some("sac_sdk_only")
        );
    }

    #[tokio::test]
    async fn retry_succeeds_on_second_attempt() {
        let counter = std::sync::Arc::new(AtomicUsize::new(0));
        let c = counter.clone();
        let policy = crate::capability::RetryPolicy {
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
                            tool: "x".into(),
                            version: "1.0".into(),
                            status: ToolStatus::Error,
                            data: Some(serde_json::json!({"error": "transient"})),
                            trace: None,
                        }
                    } else {
                        ToolResult {
                            tool: "x".into(),
                            version: "1.0".into(),
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
}
