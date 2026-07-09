//! Sandbox retrieval bridge — maps Python shim RPC to `RagRuntime` tool dispatch.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use contracts::auth_runtime::AuthContext;
use avrag_code_interpreter::HostBridge;
use contracts::{
    DenseRetrievalArgs, DenseRetrievalModality, DocChunksArgs, DocProfileArgs, DocSummaryArgs,
    DocSummaryLevel, GraphRetrievalArgs, IndexLookupArgs, LexicalRetrievalArgs, ToolCall,
    ToolResult, ToolStatus,
};
use serde_json::{Value, json};
use tracing::info;

use super::tools;
use crate::RagRuntime;

/// Host-side bridge backed by `RagRuntime` tool dispatch.
pub struct RuntimeBridge {
    runtime: Arc<RagRuntime>,
    auth: AuthContext,
    doc_scope: Vec<String>,
    captured_results: Arc<Mutex<Vec<ToolResult>>>,
}

impl RuntimeBridge {
    pub fn new(runtime: Arc<RagRuntime>, auth: AuthContext, doc_scope: Vec<String>) -> Self {
        Self {
            runtime,
            auth,
            doc_scope,
            captured_results: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Drain tool results recorded during sandbox bridge calls (for citation/degrade assembly).
    pub fn take_captured_results(&self) -> Vec<ToolResult> {
        self.captured_results
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
            .collect()
    }

    /// RPC methods supported by `method_to_tool_call` (must match Python shim `client`).
    pub fn supported_method_names() -> &'static [&'static str] {
        &[
            "dense_search",
            "lexical_search",
            "graph_search",
            "chunk_fetch",
            "doc_summary",
            "doc_profile",
            "doc_chunks",
        ]
    }

    fn bridge_error(code: &str, message: impl Into<String>) -> Value {
        json!({
            "error": {
                "code": code,
                "message": message.into(),
            }
        })
    }

    /// Intersect caller-supplied doc ids against the bridge's session scope.
    /// Mirrors the agent-loop `intersect_doc_scope`: the LLM/codegen caller can never
    /// widen scope beyond what the session established.
    fn resolve_doc_ids(&self, caller: &[String]) -> Vec<String> {
        intersect_doc_scope(caller, &self.doc_scope)
    }

    fn method_to_tool_call(&self, method: &str, args: &Value) -> Result<ToolCall, Value> {
        match method {
            "dense_search" => {
                let query = args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Self::bridge_error("invalid_args", "query is required"))?;
                let top_k = args.get("top_k").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
                Ok(ToolCall {
                    tool: "dense_retrieval".to_string(),
                    version: "1.0".to_string(),
                    args: serde_json::to_value(DenseRetrievalArgs {
                        queries: vec![query.to_string()],
                        modality: DenseRetrievalModality::Both,
                        top_k,
                        doc_scope: self.doc_scope.clone(),
                    })
                    .unwrap_or_default(),
                })
            }
            "lexical_search" => {
                let query = args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Self::bridge_error("invalid_args", "query is required"))?;
                let top_k = args.get("top_k").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
                let terms: Vec<String> = query.split_whitespace().map(ToOwned::to_owned).collect();
                let terms = if terms.is_empty() {
                    vec![query.to_string()]
                } else {
                    terms
                };
                Ok(ToolCall {
                    tool: "lexical_retrieval".to_string(),
                    version: "1.0".to_string(),
                    args: serde_json::to_value(LexicalRetrievalArgs {
                        terms,
                        top_k,
                        doc_scope: self.doc_scope.clone(),
                    })
                    .unwrap_or_default(),
                })
            }
            "graph_search" => {
                let query = args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Self::bridge_error("invalid_args", "query is required"))?;
                let depth = args.get("depth").and_then(|v| v.as_u64()).unwrap_or(2) as usize;
                Ok(ToolCall {
                    tool: "graph_retrieval".to_string(),
                    version: "1.0".to_string(),
                    args: serde_json::to_value(GraphRetrievalArgs {
                        graph_hints: Vec::new(),
                        placeholder_triplets: Vec::new(),
                        relation_limit: 20,
                        supporting_chunk_limit: 10,
                        hop_limit: depth,
                        fan_out_limit: 10,
                        query: Some(query.to_string()),
                        doc_scope: self.doc_scope.clone(),
                    })
                    .unwrap_or_default(),
                })
            }
            "doc_summary" => {
                let caller_doc_ids = args
                    .get("doc_ids")
                    .and_then(|v| v.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let doc_ids = self.resolve_doc_ids(&caller_doc_ids);
                if doc_ids.is_empty() {
                    return Err(Self::bridge_error(
                        "invalid_args",
                        "doc_ids is required when doc_scope is empty",
                    ));
                }
                let level = match args.get("level").and_then(|v| v.as_str()).unwrap_or("doc") {
                    "section" => DocSummaryLevel::Section,
                    _ => DocSummaryLevel::Doc,
                };
                Ok(ToolCall {
                    tool: "doc_summary".to_string(),
                    version: "1.0".to_string(),
                    args: serde_json::to_value(DocSummaryArgs { doc_ids, level })
                        .unwrap_or_default(),
                })
            }
            "chunk_fetch" => {
                let chunk_id = args
                    .get("chunk_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Self::bridge_error("invalid_args", "chunk_id is required"))?;
                if self.doc_scope.is_empty() {
                    // Without a session doc_scope we cannot determine which doc the
                    // chunk belongs to; refusing is safer than silently passing an empty
                    // doc_id (which `index_lookup` would treat as a wildcard lookup).
                    return Err(Self::bridge_error(
                        "invalid_scope",
                        "chunk_fetch requires a non-empty doc_scope",
                    ));
                }
                // Limitation: `index_lookup` only takes a single `doc_id`, so for a
                // multi-doc session scope we resolve to the first doc. The retrieved
                // chunk is still validated against that doc by the data plane.
                let doc_id = self.doc_scope.first().cloned().unwrap_or_default();
                Ok(ToolCall {
                    tool: "index_lookup".to_string(),
                    version: "1.0".to_string(),
                    args: serde_json::to_value(IndexLookupArgs {
                        doc_id,
                        chunk_ids: vec![chunk_id.to_string()],
                    })
                    .unwrap_or_default(),
                })
            }
            "doc_profile" => {
                let caller_doc_ids = args
                    .get("doc_ids")
                    .and_then(|v| v.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let doc_ids = self.resolve_doc_ids(&caller_doc_ids);
                if doc_ids.is_empty() {
                    return Err(Self::bridge_error(
                        "invalid_args",
                        "doc_ids is required when doc_scope is empty",
                    ));
                }
                let fields = args
                    .get("fields")
                    .and_then(|v| v.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                Ok(ToolCall {
                    tool: "doc_profile".to_string(),
                    version: "1.0".to_string(),
                    args: serde_json::to_value(DocProfileArgs { doc_ids, fields })
                        .unwrap_or_default(),
                })
            }
            "doc_chunks" => {
                let caller_doc_ids = args
                    .get("doc_ids")
                    .and_then(|v| v.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let doc_ids = self.resolve_doc_ids(&caller_doc_ids);
                if doc_ids.is_empty() {
                    return Err(Self::bridge_error(
                        "invalid_args",
                        "doc_ids is required when doc_scope is empty",
                    ));
                }
                Ok(ToolCall {
                    tool: "doc_scan".to_string(),
                    version: "1.0".to_string(),
                    args: serde_json::to_value(DocChunksArgs { doc_ids }).unwrap_or_default(),
                })
            }
            other => Err(Self::bridge_error(
                "unknown_method",
                format!("unsupported bridge method: {other}"),
            )),
        }
    }

    fn tool_result_to_bridge_data(result: &ToolResult) -> Value {
        if result.status != ToolStatus::Ok {
            let message = result
                .data
                .as_ref()
                .and_then(|d| d.get("error"))
                .and_then(|e| e.as_str())
                .unwrap_or("tool execution failed");
            return Self::bridge_error("tool_error", message);
        }

        let Some(data) = &result.data else {
            return json!({ "chunks": [] });
        };

        match result.tool.as_str() {
            "dense_retrieval" | "lexical_retrieval" | "index_lookup" | "doc_scan" => {
                json!({ "chunks": chunks_with_content_field(data) })
            }
            "graph_retrieval" => json!({ "chunks": data }),
            "doc_summary" | "doc_profile" => json!({ "chunks": data }),
            _ => json!({ "chunks": data }),
        }
    }
}

/// Intersect caller-supplied doc ids against the session scope.
/// - If `scope` is empty: no enforcement (org-wide permitted by upstream).
/// - If `scope` is non-empty: result is caller ∩ scope; if caller is empty, use scope;
///   if caller has items but none match scope, return scope (fall back to session scope
///   rather than allowing an out-of-scope id or an empty all-matching scope).
///
/// Mirrors the agent-loop `intersect_doc_scope` so the LLM/codegen caller can never
/// widen scope beyond what the session established.
fn intersect_doc_scope(caller: &[String], scope: &[String]) -> Vec<String> {
    if scope.is_empty() {
        return caller.to_vec();
    }
    if caller.is_empty() {
        return scope.to_vec();
    }
    let scope_set: std::collections::HashSet<&String> = scope.iter().collect();
    let intersection: Vec<String> = caller
        .iter()
        .filter(|c| scope_set.contains(*c))
        .cloned()
        .collect();
    if intersection.is_empty() {
        scope.to_vec()
    } else {
        intersection
    }
}

fn chunks_with_content_field(data: &Value) -> Value {
    let items = match data {
        Value::Array(items) => items.clone(),
        other => vec![other.clone()],
    };

    Value::Array(
        items
            .into_iter()
            .map(|mut item| {
                if let Some(obj) = item.as_object_mut() {
                    if let Some(text) = obj.remove("text") {
                        obj.insert("content".to_string(), text);
                    }
                }
                item
            })
            .collect(),
    )
}

#[async_trait]
impl HostBridge for RuntimeBridge {
    async fn call(&self, method: &str, args: Value) -> Value {
        let started = std::time::Instant::now();
        let tool_call = match self.method_to_tool_call(method, &args) {
            Ok(call) => call,
            Err(err) => return err,
        };

        let result = tools::dispatch(&self.runtime, &self.auth, &tool_call).await;
        self.captured_results
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(result.clone());
        let data = Self::tool_result_to_bridge_data(&result);
        let chunk_count = data
            .get("chunks")
            .and_then(|c| c.as_array())
            .map(|a| a.len())
            .unwrap_or(0);

        info!(
            bridge_method = method,
            bridge_tool = %tool_call.tool,
            bridge_elapsed_ms = started.elapsed().as_millis() as u64,
            bridge_chunk_count = chunk_count,
            "sandbox retrieval bridge call"
        );

        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use contracts::auth_runtime::{OrgId, SubjectKind};
    use avrag_llm::ModelProviderConfig;
    use avrag_retrieval_data_plane::{
        Bm25SearchOutput, Bm25SearchRequest, Bm25SearchTrace, GraphSearchOutput,
        GraphSearchRequest, MultimodalSearchRequest, RelationPathCandidate, ScoredChunk,
        TextDenseSearchRequest,
    };
    use uuid::Uuid;

    struct StubDataPlane {
        chunk_id: uuid::Uuid,
        doc_id: uuid::Uuid,
    }

    #[async_trait]
    impl avrag_retrieval_data_plane::RetrievalReadPort for StubDataPlane {
        async fn search_text_dense(
            &self,
            _request: TextDenseSearchRequest,
        ) -> anyhow::Result<Vec<ScoredChunk>> {
            Ok(vec![ScoredChunk {
                chunk_id: self.chunk_id,
                doc_id: self.doc_id,
                content: "bridge hit".to_string(),
                score: 0.95,
                source: "stub".to_string(),
                page: Some(1),
                chunk_type: "text".to_string(),
                asset_id: None,
                caption: None,
                image_path: None,
                parser_backend: None,
                source_locator: None,
                parse_run_id: None,
            }])
        }

        async fn search_bm25(
            &self,
            _request: Bm25SearchRequest,
        ) -> anyhow::Result<Bm25SearchOutput> {
            let chunk = ScoredChunk {
                chunk_id: self.chunk_id,
                doc_id: self.doc_id,
                content: "bridge hit".to_string(),
                score: 0.95,
                source: "stub".to_string(),
                page: Some(1),
                chunk_type: "text".to_string(),
                asset_id: None,
                caption: None,
                image_path: None,
                parser_backend: None,
                source_locator: None,
                parse_run_id: None,
            };
            Ok(Bm25SearchOutput {
                chunks: vec![chunk],
                trace: Bm25SearchTrace {
                    backend: "stub".to_string(),
                    raw_hit_count: 1,
                    hydrated_hit_count: 1,
                    fallback_reason: None,
                },
            })
        }

        async fn search_multimodal(
            &self,
            _request: MultimodalSearchRequest,
        ) -> anyhow::Result<Vec<ScoredChunk>> {
            Ok(Vec::new())
        }

        async fn search_graph(
            &self,
            _request: GraphSearchRequest,
        ) -> anyhow::Result<GraphSearchOutput> {
            Ok(GraphSearchOutput {
                relation_paths: Vec::<RelationPathCandidate>::new(),
                supporting_chunks: Vec::new(),
            })
        }

        async fn list_text_chunks(
            &self,
            _auth: &AuthContext,
            doc_ids: &[Uuid],
        ) -> anyhow::Result<Vec<ScoredChunk>> {
            if !doc_ids.contains(&self.doc_id) {
                return Ok(Vec::new());
            }
            Ok(vec![ScoredChunk {
                chunk_id: self.chunk_id,
                doc_id: self.doc_id,
                content: "scan hit".to_string(),
                score: 0.0,
                source: "stub".to_string(),
                page: Some(1),
                chunk_type: "text".to_string(),
                asset_id: None,
                caption: None,
                image_path: None,
                parser_backend: None,
                source_locator: None,
                parse_run_id: None,
            }])
        }
    }

    fn make_runtime() -> Arc<RagRuntime> {
        let embedding = Arc::new(avrag_llm::EmbeddingClient::new(ModelProviderConfig {
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
        }));
        let config = super::super::config::RagConfig::new_for_data_plane(embedding, None);
        let chunk_id = Uuid::from_u128(1);
        let doc_id = Uuid::parse_str("00000000-0000-0000-0000-000000000010").unwrap();
        let data_plane: Arc<dyn avrag_retrieval_data_plane::RetrievalReadPort> =
            Arc::new(StubDataPlane { chunk_id, doc_id });
        Arc::new(RagRuntime::with_data_plane(config, data_plane))
    }

    fn make_auth() -> AuthContext {
        AuthContext::new(OrgId::new(Uuid::from_u128(9)), SubjectKind::System)
    }

    #[test]
    fn bridge_host_methods_match_python_shim() {
        assert_eq!(
            RuntimeBridge::supported_method_names(),
            avrag_code_interpreter::bridge_shim_client_method_names()
        );
    }

    #[tokio::test]
    async fn runtime_bridge_dense_search_returns_chunks_with_content() {
        let runtime = make_runtime();
        let doc_scope = vec!["00000000-0000-0000-0000-000000000010".to_string()];
        let bridge = RuntimeBridge::new(runtime, make_auth(), doc_scope);
        let data = bridge
            .call(
                "dense_search",
                json!({"query": "antifragility", "top_k": 5}),
            )
            .await;
        let chunks = data["chunks"].as_array().expect("chunks array");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0]["content"], "bridge hit");
        assert_eq!(
            chunks[0]["chunk_id"],
            "00000000-0000-0000-0000-000000000001"
        );
    }

    #[tokio::test]
    async fn runtime_bridge_doc_chunks_returns_chunks_with_content() {
        // doc_chunks is the codegen sandbox entry for全量计数/枚举. The agent's
        // parsing code does `c["content"]`, so the bridge MUST surface the body
        // under a `content` key (not the raw `text` from scored_chunk_to_json).
        let runtime = make_runtime();
        let doc_scope = vec!["00000000-0000-0000-0000-000000000010".to_string()];
        let bridge = RuntimeBridge::new(runtime, make_auth(), doc_scope);
        let data = bridge.call("doc_chunks", json!({})).await;
        let chunks = data["chunks"].as_array().expect("chunks array");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0]["content"], "scan hit");
        assert!(chunks[0].get("text").is_none(), "must be renamed to content");
        assert_eq!(
            chunks[0]["chunk_id"],
            "00000000-0000-0000-0000-000000000001"
        );
    }

    #[test]
    fn chunk_fetch_tool_call_errors_on_empty_doc_scope() {
        // Previously chunk_fetch silently passed an empty doc_id to index_lookup
        // (effectively a wildcard). It must now refuse when the session scope is empty.
        let runtime = make_runtime();
        let bridge = RuntimeBridge::new(runtime, make_auth(), vec![]);
        let result = bridge.method_to_tool_call(
            "chunk_fetch",
            &json!({"chunk_id": "00000000-0000-0000-0000-000000000001"}),
        );
        let err = result.expect_err("expected scope error");
        assert_eq!(err["error"]["code"], "invalid_scope");
    }

    #[test]
    fn chunk_fetch_uses_scope_first_doc_when_non_empty() {
        let runtime = make_runtime();
        let doc_id = "00000000-0000-0000-0000-000000000010".to_string();
        let bridge = RuntimeBridge::new(runtime, make_auth(), vec![doc_id.clone()]);
        let call = bridge
            .method_to_tool_call(
                "chunk_fetch",
                &json!({"chunk_id": "00000000-0000-0000-0000-000000000001"}),
            )
            .expect("tool call");
        assert_eq!(call.tool, "index_lookup");
        let args: IndexLookupArgs = serde_json::from_value(call.args).unwrap();
        assert_eq!(args.doc_id, doc_id);
        assert_eq!(
            args.chunk_ids,
            vec!["00000000-0000-0000-0000-000000000001".to_string()]
        );
    }

    #[tokio::test]
    async fn runtime_bridge_forces_doc_scope() {
        let runtime = make_runtime();
        let forced_scope = vec!["00000000-0000-0000-0000-000000000099".to_string()];
        let bridge = RuntimeBridge::new(runtime, make_auth(), forced_scope.clone());
        let call = bridge
            .method_to_tool_call("dense_search", &json!({"query": "x"}))
            .expect("tool call");
        let args: DenseRetrievalArgs = serde_json::from_value(call.args).unwrap();
        assert_eq!(args.doc_scope, forced_scope);
    }

    #[test]
    fn doc_summary_caller_doc_ids_outside_scope_narrowed_to_scope() {
        // The codegen sandbox requests a doc id outside the session scope; the bridge
        // must clamp it down to the session scope rather than honoring the request.
        let runtime = make_runtime();
        let scope = vec!["00000000-0000-0000-0000-000000000010".to_string()];
        let bridge = RuntimeBridge::new(runtime, make_auth(), scope.clone());
        let call = bridge
            .method_to_tool_call(
                "doc_summary",
                &json!({"doc_ids": ["00000000-0000-0000-0000-000000000099"]}),
            )
            .expect("tool call");
        let args: DocSummaryArgs = serde_json::from_value(call.args).unwrap();
        assert_eq!(args.doc_ids, scope);
    }

    #[test]
    fn doc_summary_caller_doc_ids_in_scope_preserved() {
        // An in-scope caller doc id survives the intersection.
        let runtime = make_runtime();
        let scope = vec!["00000000-0000-0000-0000-000000000010".to_string()];
        let bridge = RuntimeBridge::new(runtime, make_auth(), scope.clone());
        let call = bridge
            .method_to_tool_call(
                "doc_summary",
                &json!({"doc_ids": ["00000000-0000-0000-0000-000000000010"]}),
            )
            .expect("tool call");
        let args: DocSummaryArgs = serde_json::from_value(call.args).unwrap();
        assert_eq!(args.doc_ids, scope);
    }

    #[test]
    fn doc_summary_empty_caller_uses_full_scope() {
        let runtime = make_runtime();
        let scope = vec!["00000000-0000-0000-0000-000000000010".to_string()];
        let bridge = RuntimeBridge::new(runtime, make_auth(), scope.clone());
        let call = bridge
            .method_to_tool_call("doc_summary", &json!({}))
            .expect("tool call");
        let args: DocSummaryArgs = serde_json::from_value(call.args).unwrap();
        assert_eq!(args.doc_ids, scope);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn interpreter_hits_runtime_bridge_end_to_end() {
        let runtime = make_runtime();
        let bridge = Arc::new(RuntimeBridge::new(
            runtime,
            make_auth(),
            vec!["00000000-0000-0000-0000-000000000010".to_string()],
        ));
        let interpreter = avrag_code_interpreter::CodeInterpreter::new().with_timeout(10);
        let code = r#"
chunks = await client.dense_search(query="antifragility", top_k=5)
import json
print(json.dumps(chunks))
"#;
        let result = interpreter.execute_with_bridge(code, bridge).await.unwrap();
        assert!(result.success, "stderr={}", result.stderr);
        assert!(
            result.stdout.contains("bridge hit"),
            "stdout={}",
            result.stdout
        );
        assert!(
            result
                .stdout
                .contains("00000000-0000-0000-0000-000000000001")
        );
    }
}
