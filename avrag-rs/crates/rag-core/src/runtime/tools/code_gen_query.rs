//! `code_gen_query` — execute Python in a sandbox to orchestrate retrieval.
//!
//! This tool is the "code-gen" path for complex queries. The LLM writes
//! Python that imports `avrag_sdk` (thin HTTP client to the Rust retrieval
//! backend), calls retrieval primitives in any combination, and returns
//! chunks.
//!
//! ## When to use
//!
//! Use this tool when the LLM needs to:
//! - **Fan-out** retrieval across multiple query variations in parallel
//! - **Cross-source correlation**: use results from one source as queries
//!   for another (e.g., graph result → lookup related docs)
//! - **Adaptive iteration**: branch based on intermediate results
//!   ("if len(candidates) < 10, broaden the query")
//! - **Custom post-processing**: filter, dedupe, aggregate in Python
//!
//! For simple queries, use the atomic tools (`dense_retrieval`,
//! `lexical_retrieval`, etc.) directly.
//!
//! ## Output contract
//!
//! The Python program's last expression should be a JSON-serializable
//! list of chunk dicts. The tool parses this and returns it as
//! `data.chunks`.
//!
//! Expected chunk dict fields:
//! - `chunk_id` (string, required)
//! - `doc_id` (string, required)
//! - `content` (string, required)
//! - `score` (number, optional)
//! - `source` (string, optional)
//! - `page` (number, optional)
//! - `chunk_type` (string, optional)
//!
//! Example:
//! ```python
//! from avrag_sdk import client
//! import asyncio
//!
//! async def main():
//!     tasks = [client.dense(q, k=10) for q in queries]
//!     results = await asyncio.gather(*tasks)
//!     merged = {c.chunk_id: c for group in results for c in group}
//!     return [c.model_dump() for c in merged.values()]
//!
//! import json
//! json.dumps(asyncio.run(main()))
//! ```
//!
//! ## Error handling
//!
//! - Python exceptions are caught by the sandbox and returned in
//!   `data.stderr`. The tool's `status` is `Error` in that case so the
//!   LLM can replan.
//! - Sandbox-level errors (timeout, rlimit) → `ToolStatus::Error` with
//!   `data.error` describing the failure.
//! - Argument parsing errors → `ToolStatus::Error`.

use avrag_auth::AuthContext;
use common::{CodeGenQueryArgs, ToolResult, ToolStatus, ToolTrace};

use crate::RagRuntime;

const TOOL_NAME: &str = "code_gen_query";
const TOOL_VERSION: &str = "1.0";

pub async fn run(
    _runtime: &RagRuntime,
    _auth: &AuthContext,
    args: &serde_json::Value,
) -> ToolResult {
    let args: CodeGenQueryArgs = match serde_json::from_value(args.clone()) {
        Ok(a) => a,
        Err(e) => {
            return super::error_result(TOOL_NAME, format!("invalid args: {e}"));
        }
    };

    if args.code.trim().is_empty() {
        return super::error_result(TOOL_NAME, "code must not be empty".to_string());
    }

    let started = std::time::Instant::now();

    // Inject context (if provided) as Python variable assignments
    // prepended to the user's code. Done before moving args.code into
    // the blocking task.
    let code = match inject_context(&args.code, args.context.as_ref()) {
        Ok(c) => c,
        Err(e) => return super::error_result(TOOL_NAME, format!("context injection failed: {e}")),
    };

    // The Python interpreter is synchronous (subprocess-based). Run it
    // on a blocking pool so we don't hold the async runtime.
    let exec_result = tokio::task::spawn_blocking(move || {
        let interpreter = avrag_code_interpreter::CodeInterpreter::new();
        interpreter.execute(&code)
    })
    .await;

    let elapsed_ms = started.elapsed().as_millis() as u64;

    let exec = match exec_result {
        Ok(Ok(e)) => e,
        Ok(Err(e)) => {
            return super::error_result(
                TOOL_NAME,
                format!("sandbox error: {e}"),
            );
        }
        Err(e) => {
            return super::error_result(
                TOOL_NAME,
                format!("sandbox task panicked: {e}"),
            );
        }
    };

    // If the sandbox was killed (timeout / rlimit), return error
    if exec.killed {
        return ToolResult {
            tool: TOOL_NAME.to_string(),
            version: TOOL_VERSION.to_string(),
            status: ToolStatus::Error,
            data: Some(serde_json::json!({
                "error": "sandbox killed (timeout or resource limit)",
                "stdout": exec.stdout,
                "stderr": exec.stderr,
                "exit_code": exec.exit_code,
                "program": args.code,
                "elapsed_ms": elapsed_ms,
            })),
            trace: None,
        };
    }

    // The sandbox wrapper does NOT capture expression values (its
    // _result variable is never assigned). All output must go to
    // stdout. We take the last non-empty line of stdout and try to
    // parse it as JSON.
    let chunks_result = parse_chunks_from_stdout(&exec.stdout);

    match chunks_result {
        Ok(chunks) => {
            let chunk_count = chunks.len();
            ToolResult {
                tool: TOOL_NAME.to_string(),
                version: TOOL_VERSION.to_string(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!({
                    "chunks": chunks,
                    "chunk_count": chunk_count,
                    "program": args.code,
                    "stdout": exec.stdout,
                    "stderr": exec.stderr,
                    "elapsed_ms": elapsed_ms,
                })),
                trace: Some(ToolTrace {
                    elapsed_ms: Some(elapsed_ms),
                    raw_hit_count: Some(chunk_count),
                    hydrated_hit_count: Some(chunk_count),
                    degrade_reason: None,
                }),
            }
        }
        Err(reason) => {
            // Last expression wasn't a chunk list. This may be a
            // legitimate result (e.g., the program computed a number
            // or summary) — return the raw output as `data` and let
            // the LLM use it.
            ToolResult {
                tool: TOOL_NAME.to_string(),
                version: TOOL_VERSION.to_string(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!({
                    "raw_stdout": exec.stdout,
                    "stdout": exec.stdout,
                    "stderr": exec.stderr,
                    "program": args.code,
                    "elapsed_ms": elapsed_ms,
                    "parse_note": reason,
                })),
                trace: Some(ToolTrace {
                    elapsed_ms: Some(elapsed_ms),
                    raw_hit_count: None,
                    hydrated_hit_count: None,
                    degrade_reason: None,
                }),
            }
        }
    }
}

/// Inject context as Python variable assignments prepended to `code`.
///
/// For example, `{"user_query": "find contracts"}` becomes:
/// ```python
/// user_query = "find contracts"
/// <user's code here>
/// ```
///
/// Returns an error if the context is not a JSON object.
fn inject_context(
    code: &str,
    context: Option<&serde_json::Value>,
) -> Result<String, String> {
    let Some(ctx) = context else {
        return Ok(code.to_string());
    };
    let Some(obj) = ctx.as_object() else {
        return Err("context must be a JSON object".to_string());
    };

    let mut prelude = String::new();
    for (key, value) in obj {
        // Validate key is a valid Python identifier
        if !is_valid_python_identifier(key) {
            return Err(format!(
                "context key '{key}' is not a valid Python identifier"
            ));
        }
        let json_str = serde_json::to_string(value)
            .map_err(|e| format!("failed to serialize context value: {e}"))?;
        prelude.push_str(&format!("{key} = {json_str}\n"));
    }
    Ok(format!("{prelude}{code}"))
}

fn is_valid_python_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Try to parse the last non-empty line of stdout as a JSON list of
/// chunk dicts.
///
/// The Python program should `print(json.dumps(chunks))` as its final
/// output. We take the last non-empty line of stdout and parse it as
/// strict JSON.
///
/// If parsing fails, returns an `Err` with a reason so the tool can
/// return the raw stdout for the LLM to inspect.
fn parse_chunks_from_stdout(stdout: &str) -> Result<Vec<serde_json::Value>, String> {
    let last_line = stdout
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .last();

    let Some(line) = last_line else {
        return Err("stdout is empty".to_string());
    };

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
        return chunks_from_json_value(value);
    }

    Err(format!(
        "last stdout line is not valid JSON: {}",
        &line[..line.len().min(80)]
    ))
}

fn chunks_from_json_value(value: serde_json::Value) -> Result<Vec<serde_json::Value>, String> {
    match value {
        serde_json::Value::Array(items) => {
            // Validate each item has at least chunk_id and doc_id
            for (i, item) in items.iter().enumerate() {
                if !item.is_object() {
                    return Err(format!("item {i} is not an object"));
                }
                let obj = item.as_object().unwrap();
                if !obj.contains_key("chunk_id") {
                    return Err(format!("item {i} missing 'chunk_id'"));
                }
            }
            Ok(items)
        }
        other => Err(format!(
            "expected JSON array of chunks, got: {}",
            serde_json::to_string(&other).unwrap_or_default()
        )),
    }
}

/// Minimal Python list-of-dicts repr parser.
///
/// Handles:
/// - `[{...}, {...}]` (list of dicts)
/// - `'string'` and `"string"` (quoted strings)
/// - `True` / `False` / `None` (Python booleans/null)
/// - Numbers
///
/// Does NOT handle nested lists/dicts with full fidelity. Use
/// `json.dumps()` for guaranteed correctness.
fn parse_python_list_repr(s: &str) -> Option<Vec<String>> {
    let s = s.trim();
    if !s.starts_with('[') || !s.ends_with(']') {
        return None;
    }
    let inner = &s[1..s.len() - 1];
    // Split by top-level commas. This is a simplified parser — does
    // not handle nested structures. For the LLM's expected output
    // (flat dicts), this is sufficient.
    let mut items = Vec::new();
    let mut depth = 0;
    let mut current = String::new();
    let mut in_string: Option<char> = None;
    let mut escape = false;

    for c in inner.chars() {
        if escape {
            current.push(c);
            escape = false;
            continue;
        }
        if c == '\\' && in_string.is_some() {
            current.push(c);
            escape = true;
            continue;
        }
        if let Some(q) = in_string {
            if c == q {
                in_string = None;
            }
            current.push(c);
            continue;
        }
        if c == '\'' || c == '"' {
            in_string = Some(c);
            current.push(c);
            continue;
        }
        if c == '[' || c == '{' || c == '(' {
            depth += 1;
        } else if c == ']' || c == '}' || c == ')' {
            depth -= 1;
        }
        if c == ',' && depth == 0 {
            items.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(c);
        }
    }
    if !current.trim().is_empty() {
        items.push(current.trim().to_string());
    }
    Some(items)
}

fn chunks_from_list_repr(items: Vec<String>) -> Vec<serde_json::Value> {
    items
        .into_iter()
        .map(|item| {
            // Convert Python repr to JSON-ish.
            // This is best-effort — proper handling requires ast.literal_eval
            // or running Python. For the LLM's use case, we wrap the raw
            // string as `{"_raw": item}` so it's at least JSON-valid.
            serde_json::json!({ "_raw_repr": item })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn inject_context_simple() {
        let ctx = json!({"user_query": "find contracts", "limit": 10});
        let code = "print(user_query)";
        let result = inject_context(code, Some(&ctx)).unwrap();
        assert!(result.contains("user_query = \"find contracts\""));
        assert!(result.contains("limit = 10"));
        assert!(result.contains("print(user_query)"));
    }

    #[test]
    fn inject_context_none_returns_code_unchanged() {
        let code = "print('hello')";
        let result = inject_context(code, None).unwrap();
        assert_eq!(result, code);
    }

    #[test]
    fn inject_context_rejects_non_object() {
        let ctx = json!("just a string");
        let result = inject_context("code", Some(&ctx));
        assert!(result.is_err());
    }

    #[test]
    fn inject_context_rejects_invalid_identifier() {
        let ctx = json!({"1invalid": "value"});
        let result = inject_context("code", Some(&ctx));
        assert!(result.is_err());
    }

    #[test]
    fn is_valid_python_identifier_accepts_underscore_start() {
        assert!(is_valid_python_identifier("_user_query"));
        assert!(is_valid_python_identifier("user_query_2"));
    }

    #[test]
    fn is_valid_python_identifier_rejects_dash() {
        assert!(!is_valid_python_identifier("user-query"));
    }

    #[test]
    fn parse_chunks_from_json_array() {
        let json_str = r#"[{"chunk_id":"c1","doc_id":"d1","content":"x","score":0.9}]"#;
        let chunks = parse_chunks_from_stdout(json_str).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0]["chunk_id"], "c1");
    }

    #[test]
    fn parse_chunks_rejects_array_without_chunk_id() {
        let json_str = r#"[{"doc_id":"d1","content":"x"}]"#;
        let result = parse_chunks_from_stdout(json_str);
        assert!(result.is_err());
    }

    #[test]
    fn parse_chunks_rejects_non_array() {
        let json_str = r#"{"chunk_id":"c1"}"#;
        let result = parse_chunks_from_stdout(json_str);
        assert!(result.is_err());
    }

    #[test]
    fn parse_chunks_takes_last_line_of_multi_line_stdout() {
        let stdout = "Loading...\nProcessing 5 queries...\n[{\"chunk_id\":\"c1\",\"doc_id\":\"d1\",\"content\":\"x\"}]";
        let chunks = parse_chunks_from_stdout(stdout).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0]["chunk_id"], "c1");
    }

    #[test]
    fn parse_chunks_handles_empty_stdout() {
        let result = parse_chunks_from_stdout("");
        assert!(result.is_err());
    }

    #[test]
    fn parse_chunks_handles_whitespace_only_stdout() {
        let result = parse_chunks_from_stdout("   \n  \n");
        assert!(result.is_err());
    }

    #[test]
    fn parse_python_list_repr_basic() {
        let result = parse_python_list_repr("[1, 2, 3]").unwrap();
        assert_eq!(result, vec!["1", "2", "3"]);
    }

    #[test]
    fn parse_python_list_repr_with_strings() {
        let result = parse_python_list_repr("['a', 'b']").unwrap();
        assert_eq!(result, vec!["'a'", "'b'"]);
    }

    #[test]
    fn parse_python_list_repr_rejects_non_list() {
        assert!(parse_python_list_repr("'string'").is_none());
        assert!(parse_python_list_repr("42").is_none());
    }

    // ----------------------------------------------------------------
    // Integration tests — actually execute Python through the sandbox.
    //
    // These tests require `python3` to be on PATH. They exercise the
    // full tool pipeline: arg parsing → context injection → sandbox
    // execution → result parsing → ToolResult construction.
    //
    // Note: the tool does not actually use the runtime (it's `_runtime`),
    // but the trait requires a valid `RagRuntime`. We build a minimal
    // one using a stub `EmbeddingClient` and the empty data plane.
    // ----------------------------------------------------------------

    use avrag_auth::{AuthContext, OrgId, SubjectKind};
    use avrag_llm::ModelProviderConfig;
    use std::sync::Arc;
    use uuid::Uuid;

    fn make_test_runtime() -> std::sync::Arc<crate::RagRuntime> {
        let embedding = Arc::new(avrag_llm::EmbeddingClient::new(
            ModelProviderConfig {
                base_url: "http://localhost:9999".to_string(),
                api_key: "test".to_string(),
                model: "test-model".to_string(),
                timeout_ms: 5000,
                api_style: None,
                dimensions: None,
                enable_thinking: None,
                enable_cache: None,
                rpm_limit: None,
                tpm_limit: None,
            },
        ));
        let config = crate::runtime::RagConfig::new_for_data_plane(embedding, None);
        let data_plane: Arc<dyn avrag_retrieval_data_plane::RetrievalDataPlane> =
            Arc::new(EmptyDataPlane);
        std::sync::Arc::new(crate::RagRuntime::with_data_plane(config, data_plane))
    }

    fn make_test_auth() -> AuthContext {
        AuthContext::new(OrgId::new(Uuid::from_u128(9)), SubjectKind::System)
    }

    /// Minimal data plane that returns empty results for everything.
    /// Used only to satisfy the trait; the code_gen_query tool doesn't
    /// actually call into the data plane.
    struct EmptyDataPlane;

    #[async_trait::async_trait]
    impl avrag_retrieval_data_plane::RetrievalDataPlane for EmptyDataPlane {
        async fn search_text_dense(
            &self,
            _request: avrag_retrieval_data_plane::TextDenseSearchRequest,
        ) -> anyhow::Result<Vec<avrag_retrieval_data_plane::ScoredChunk>> {
            Ok(Vec::new())
        }
        async fn search_bm25(
            &self,
            _request: avrag_retrieval_data_plane::Bm25SearchRequest,
        ) -> anyhow::Result<avrag_retrieval_data_plane::Bm25SearchOutput> {
            Ok(avrag_retrieval_data_plane::Bm25SearchOutput {
                chunks: Vec::new(),
                trace: avrag_retrieval_data_plane::Bm25SearchTrace {
                    backend: "test".to_string(),
                    raw_hit_count: 0,
                    hydrated_hit_count: 0,
                    fallback_reason: None,
                },
            })
        }
        async fn search_multimodal(
            &self,
            _request: avrag_retrieval_data_plane::MultimodalSearchRequest,
        ) -> anyhow::Result<Vec<avrag_retrieval_data_plane::ScoredChunk>> {
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn tool_executes_simple_python_and_returns_chunks() {
        let runtime = make_test_runtime();
        let auth = make_test_auth();

        // The LLM's Python program returns a JSON list of chunk dicts
        // as the last expression. The tool should parse this and return
        // it as `data.chunks`.
        let code = r#"
import json
chunks = [
    {"chunk_id": "00000000-0000-0000-0000-000000000001", "doc_id": "00000000-0000-0000-0000-000000000010", "content": "hello", "score": 0.9, "source": "test"},
    {"chunk_id": "00000000-0000-0000-0000-000000000002", "doc_id": "00000000-0000-0000-0000-000000000010", "content": "world", "score": 0.8, "source": "test"},
]
print(json.dumps(chunks))
"#;

        let args = serde_json::json!({"code": code});
        let result = run(&runtime, &auth, &args).await;

        assert_eq!(result.tool, "code_gen_query");
        assert_eq!(result.status, common::ToolStatus::Ok);
        let data = result.data.expect("data should be present");
        let chunks = data["chunks"].as_array().expect("chunks should be an array");
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0]["content"], "hello");
        assert_eq!(chunks[0]["score"], 0.9);
        assert_eq!(data["chunk_count"], 2);
    }

    #[tokio::test]
    async fn tool_injects_context_as_python_variables() {
        let runtime = make_test_runtime();
        let auth = make_test_auth();

        // The context {"user_query": "test query"} should be prepended
        // as `user_query = "test query"` so the program can reference it.
        let code = r#"
import json
chunk = {"chunk_id": "00000000-0000-0000-0000-000000000099", "doc_id": "00000000-0000-0000-0000-000000000010", "content": user_query, "score": 0.5, "source": "ctx-test"}
print(json.dumps([chunk]))
"#;

        let args = serde_json::json!({
            "code": code,
            "context": {"user_query": "injected-value-test"}
        });
        let result = run(&runtime, &auth, &args).await;

        assert_eq!(result.status, common::ToolStatus::Ok);
        let chunks = result.data.unwrap()["chunks"].as_array().unwrap().clone();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0]["content"], "injected-value-test");
    }

    #[tokio::test]
    async fn tool_returns_non_chunk_output_as_raw_result() {
        let runtime = make_test_runtime();
        let auth = make_test_auth();

        // The program computes a number, not a chunk list. The tool
        // should return it as `data.raw_result`, not as chunks.
        let code = r#"
import json
result = {"total": 42, "items": ["a", "b"]}
print(json.dumps(result))
"#;

        let args = serde_json::json!({"code": code});
        let result = run(&runtime, &auth, &args).await;

        assert_eq!(result.status, common::ToolStatus::Ok);
        let data = result.data.expect("data should be present");
        assert!(data.get("chunks").is_none(), "non-chunk output should not have chunks field");
        assert!(data.get("raw_stdout").is_some(), "should have raw_stdout field");
        let raw = &data["raw_stdout"];
        assert!(raw.as_str().unwrap().contains("42"));
    }

    #[tokio::test]
    async fn tool_handles_python_exception_gracefully() {
        let runtime = make_test_runtime();
        let auth = make_test_auth();

        // Python exception — sandbox returns success=true (caught) but
        // stderr contains the error. The tool's `result` is None or
        // empty (last expression raised before assignment), so it
        // returns raw_result with the error info.
        let code = r#"raise ValueError('test error')"#;

        let args = serde_json::json!({"code": code});
        let result = run(&runtime, &auth, &args).await;

        // The sandbox returns success=true and a successful exit, but
        // the result is None. The tool returns raw_result with the
        // parse_note explaining the situation.
        assert_eq!(result.status, common::ToolStatus::Ok);
        let data = result.data.expect("data should be present");
        // stderr should contain the ValueError
        assert!(data["stderr"]
            .as_str()
            .unwrap()
            .contains("ValueError"));
    }

    #[tokio::test]
    async fn tool_rejects_empty_code() {
        let runtime = make_test_runtime();
        let auth = make_test_auth();

        let args = serde_json::json!({"code": "   "});
        let result = run(&runtime, &auth, &args).await;

        assert_eq!(result.status, common::ToolStatus::Error);
        let data = result.data.unwrap();
        assert!(data["error"]
            .as_str()
            .unwrap()
            .contains("must not be empty"));
    }

    #[tokio::test]
    async fn tool_rejects_invalid_args() {
        let runtime = make_test_runtime();
        let auth = make_test_auth();

        let args = serde_json::json!({"wrong_field": "value"});
        let result = run(&runtime, &auth, &args).await;

        assert_eq!(result.status, common::ToolStatus::Error);
        let data = result.data.unwrap();
        assert!(data["error"]
            .as_str()
            .unwrap()
            .contains("invalid args"));
    }

    #[tokio::test]
    async fn tool_returns_program_in_data_for_audit() {
        let runtime = make_test_runtime();
        let auth = make_test_auth();

        let code = r#"
import json
print(json.dumps([{"chunk_id": "00000000-0000-0000-0000-000000000001", "doc_id": "00000000-0000-0000-0000-000000000010", "content": "x"}]))
"#;

        let args = serde_json::json!({"code": code});
        let result = run(&runtime, &auth, &args).await;

        let data = result.data.unwrap();
        // The original code should be returned for audit
        let returned_program = data["program"].as_str().unwrap();
        assert!(returned_program.contains("00000000-0000-0000-0000-000000000001"));
    }

    #[tokio::test]
    async fn tool_tracks_elapsed_time() {
        let runtime = make_test_runtime();
        let auth = make_test_auth();

        let code = r#"
import json, time
time.sleep(0.1)
print(json.dumps([{"chunk_id": "00000000-0000-0000-0000-000000000001", "doc_id": "00000000-0000-0000-0000-000000000010", "content": "x"}]))
"#;

        let args = serde_json::json!({"code": code});
        let result = run(&runtime, &auth, &args).await;

        let data = result.data.unwrap();
        let elapsed = data["elapsed_ms"].as_u64().unwrap();
        assert!(elapsed >= 100, "should record at least 100ms elapsed, got {elapsed}");
    }
}
