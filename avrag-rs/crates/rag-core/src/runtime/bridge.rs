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

/// One sandbox `client.*` call with product-progress metadata (not for UI raw tool dump).
#[derive(Debug, Clone)]
pub struct CapturedBridgeCall {
    /// SDK method name: dense_search / lexical_search / …
    pub method: String,
    /// Human query / terms from call args when present.
    pub query: Option<String>,
    pub result: ToolResult,
}

/// E3 (2026-07-28): coaching hint when a `lexical_search` returns 0 hits.
///
/// Pure function over (query, hit_count) so the wording is unit-testable and
/// the call site (agent-loop codegen observation) stays a thin collector.
/// The hint is model-facing: multi-word Chinese queries tend to be tokenized
/// whole, so the first move is splitting to core terms — NOT early-stopping
/// (no counts change, hint only).
pub fn lexical_zero_hit_hint(query: &str, hit_count: usize) -> Option<String> {
    if hit_count > 0 {
        return None;
    }
    Some(format!(
        "[retrieval_hint] lexical_search 0 命中（query: \"{query}\"）。\
         多词中文查询可能被整词切分——依次尝试：\
         ① 拆出核心专名/单词条单独查（如「速冻机」而非「速冻机 年产」）；\
         ② 换同义词/近义表述；\
         ③ doc_scan 扫目录定位章节；\
         ④ 仍无结果可用 dense_search 复核语义。\
         避免对同一措辞连续无效重试。[/retrieval_hint]"
    ))
}

/// Host-side bridge backed by `RagRuntime` tool dispatch.
pub struct RuntimeBridge {
    runtime: Arc<RagRuntime>,
    auth: AuthContext,
    doc_scope: Vec<String>,
    captured_results: Arc<Mutex<Vec<ToolResult>>>,
    captured_calls: Arc<Mutex<Vec<CapturedBridgeCall>>>,
}

impl RuntimeBridge {
    pub fn new(runtime: Arc<RagRuntime>, auth: AuthContext, doc_scope: Vec<String>) -> Self {
        Self {
            runtime,
            auth,
            doc_scope,
            captured_results: Arc::new(Mutex::new(Vec::new())),
            captured_calls: Arc::new(Mutex::new(Vec::new())),
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

    /// Drain per-call progress metadata (method + query + result).
    pub fn take_captured_calls(&self) -> Vec<CapturedBridgeCall> {
        self.captured_calls
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
            .collect()
    }

    fn extract_query(method: &str, args: &Value) -> Option<String> {
        match method {
            "dense_search" | "lexical_search" | "graph_search" => args
                .get("query")
                .and_then(|v| v.as_str())
                .map(str::to_owned),
            "chunk_fetch" => args
                .get("chunk_id")
                .and_then(|v| v.as_str())
                .map(|id| format!("片段 {id}")),
            _ => None,
        }
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
            "doc_scan",
            // legacy alias — prefer doc_scan in prompts/client
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

    /// C10: multi-doc-scope chunk_fetch — probe each scoped doc with its own
    /// `index_lookup` and return the first non-empty result, or the last probe
    /// when no scoped doc contains the chunk (honest empty). Mirrors
    /// `resolve_doc_ids`'s full-scope intent within index_lookup's single-doc
    /// contract.
    async fn dispatch_chunk_fetch_full_scope(&self, args: &Value) -> ToolResult {
        let chunk_id = args
            .get("chunk_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let mut last = None;
        for doc_id in &self.doc_scope {
            let probe = ToolCall {
                tool: "index_lookup".to_string(),
                version: "1.0".to_string(),
                args: serde_json::to_value(IndexLookupArgs {
                    doc_id: doc_id.clone(),
                    chunk_ids: vec![chunk_id.to_string()],
                })
                .unwrap_or_default(),
            };
            let result = tools::dispatch(&self.runtime, &self.auth, &probe).await;
            let non_empty = result
                .data
                .as_ref()
                .and_then(|d| d.as_array())
                .is_some_and(|a| !a.is_empty());
            if non_empty {
                return result;
            }
            last = Some(result);
        }
        // The multi-doc branch guarantees a non-empty scope, so at least one
        // probe ran.
        last.expect("multi-doc scope yields at least one probe")
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
                // Single-doc limitation remains inside method_to_tool_call
                // (index_lookup takes one doc_id), but multi-doc scopes are
                // fanned out per scoped doc in `call` (C10) — see
                // dispatch_chunk_fetch_full_scope below.
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
            // doc_scan: sandbox-side material for code scan/count/filter (not chat dump).
            // doc_chunks: legacy RPC alias — same host tool.
            "doc_scan" | "doc_chunks" => {
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
            "dense_retrieval" | "index_lookup" | "doc_scan" => {
                json!({ "chunks": chunks_with_content_field(data) })
            }
            // lexical may already be `{ chunks, graph_context }` (native path alignment).
            "lexical_retrieval" => {
                if let Some(obj) = data.as_object() {
                    if obj.contains_key("chunks") {
                        let mut out = json!({
                            "chunks": chunks_with_content_field(
                                obj.get("chunks").unwrap_or(&Value::Null)
                            )
                        });
                        if let Some(gc) = obj.get("graph_context") {
                            out.as_object_mut()
                                .expect("object")
                                .insert("graph_context".to_string(), gc.clone());
                        }
                        return out;
                    }
                }
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

        // C10: chunk_fetch must resolve across the FULL session doc_scope.
        // index_lookup is single-doc by contract, so a multi-doc scope fans
        // out one lookup per scoped doc and takes the first non-empty result
        // (a chunk belongs to exactly one doc; a wrong-doc lookup returns an
        // empty array, not an error). Single-doc scope keeps the direct
        // dispatch below.
        let result = if method == "chunk_fetch" && self.doc_scope.len() > 1 {
            self.dispatch_chunk_fetch_full_scope(&args).await
        } else {
            // Lexical force-augment runs inside lexical_retrieval (single
            // place for bridge + native). dense_search never gets
            // graph_context. Bridge only surfaces + telemetry-splits.
            tools::dispatch(&self.runtime, &self.auth, &tool_call).await
        };

        self.captured_results
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(result.clone());
        self.captured_calls
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(CapturedBridgeCall {
                method: method.to_string(),
                query: Self::extract_query(method, &args),
                result: result.clone(),
            });
        let mut data = Self::tool_result_to_bridge_data(&result);

        // Telemetry for eval: non-empty graph_context → side-car with degrade_reason=graph_augment.
        let graph_context_count = if method == "lexical_search" {
            let gc = data
                .get("graph_context")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            if !gc.is_empty() {
                let elapsed = started.elapsed().as_millis() as u64;
                let telemetry = tools::graph_augment::telemetry_tool_result(&gc, elapsed);
                self.captured_results
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(telemetry);
            }
            gc.len()
        } else {
            // Ensure dense never leaks a graph_context key.
            if method == "dense_search" {
                if let Some(obj) = data.as_object_mut() {
                    obj.remove("graph_context");
                }
            }
            0
        };
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
            bridge_graph_context_count = graph_context_count,
            "sandbox retrieval bridge call"
        );

        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use contracts::auth_runtime::{UserId, SubjectKind};
    use avrag_retrieval_data_plane::{
        Bm25SearchOutput, Bm25SearchRequest, Bm25SearchTrace, GraphSearchOutput,
        GraphSearchRequest, MultimodalSearchRequest, RelationPathCandidate, ScoredChunk,
        TextDenseSearchRequest,
    };
    use uuid::Uuid;

    struct StubDataPlane {
        chunk_id: uuid::Uuid,
        doc_id: uuid::Uuid,
        /// When true, search_graph returns a fixed DRC→DRO relation for seed tests.
        graph_edge: bool,
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
            request: GraphSearchRequest,
        ) -> anyhow::Result<GraphSearchOutput> {
            if !self.graph_edge {
                return Ok(GraphSearchOutput {
                    relation_paths: Vec::<RelationPathCandidate>::new(),
                    supporting_chunks: Vec::new(),
                });
            }
            // Only seed when caller asked for DRC/DRO style entities (terms path).
            let has_seed = request
                .entity_names
                .iter()
                .chain(request.query_entities.iter())
                .any(|n| n.eq_ignore_ascii_case("DRC") || n.eq_ignore_ascii_case("DRO"));
            if !has_seed {
                return Ok(GraphSearchOutput::default());
            }
            let rel_id = Uuid::from_u128(42);
            Ok(GraphSearchOutput {
                relation_paths: vec![RelationPathCandidate {
                    subject: "DRC".to_string(),
                    predicate: "maps_to".to_string(),
                    object: "DRO".to_string(),
                    score: 0.85,
                    supporting_chunk_ids: vec![rel_id],
                }],
                supporting_chunks: vec![ScoredChunk {
                    chunk_id: rel_id,
                    doc_id: self.doc_id,
                    content: "DRC maps_to DRO in catalog".to_string(),
                    score: 0.85,
                    source: "stub_graph".to_string(),
                    page: None,
                    chunk_type: "graph_relation".to_string(),
                    asset_id: None,
                    caption: None,
                    image_path: None,
                    parser_backend: None,
                    source_locator: None,
                    parse_run_id: None,
                }],
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
        make_runtime_with_graph(false)
    }

    fn make_runtime_with_graph(graph_edge: bool) -> Arc<RagRuntime> {
        let config = crate::test_doubles::test_rag_config();
        let chunk_id = Uuid::from_u128(1);
        let doc_id = Uuid::parse_str("00000000-0000-0000-0000-000000000010").unwrap();
        let data_plane: Arc<dyn avrag_retrieval_data_plane::RetrievalReadPort> =
            Arc::new(StubDataPlane {
                chunk_id,
                doc_id,
                graph_edge,
            });
        Arc::new(RagRuntime::with_data_plane(config, data_plane))
    }

    fn make_auth() -> AuthContext {
        AuthContext::new(UserId::new(Uuid::from_u128(9)), SubjectKind::System)
    }

    #[test]
    fn bridge_host_methods_match_python_shim() {
        let host = RuntimeBridge::supported_method_names();
        let shim = avrag_code_interpreter::bridge_shim_client_method_names();
        // Every shim-advertised method must be host-supported. The host
        // additionally keeps legacy RPC aliases (doc_chunks → doc_scan, see
        // `method_to_tool_call`) that the shim no longer advertises (2026-07-20).
        for m in shim {
            assert!(
                host.contains(m),
                "shim method {m} missing from host bridge: {host:?}"
            );
        }
        assert!(
            host.contains(&"doc_chunks"),
            "legacy alias doc_chunks must stay host-side: {host:?}"
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
    async fn runtime_bridge_doc_scan_returns_chunks_with_content() {
        // doc_scan: sandbox material for code-side count/filter. Agent code uses
        // `c["content"]`, so the bridge MUST surface body under `content`.
        let runtime = make_runtime();
        let doc_scope = vec!["00000000-0000-0000-0000-000000000010".to_string()];
        let bridge = RuntimeBridge::new(runtime, make_auth(), doc_scope);
        let data = bridge.call("doc_scan", json!({})).await;
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

    /// Minimal ContentStore stub for chunk_fetch tests: serves exactly one
    /// chunk (`chunk_id`) living in `doc_id`.
    struct StubContentStore {
        chunk_id: uuid::Uuid,
        doc_id: uuid::Uuid,
    }

    #[async_trait]
    impl common::content_store::ContentStore for StubContentStore {
        async fn get_chunks_by_ids(
            &self,
            _auth: &AuthContext,
            chunk_ids: &[uuid::Uuid],
        ) -> anyhow::Result<
            std::collections::HashMap<uuid::Uuid, common::content_store::IndexedChunk>,
            common::content_store::ContentStoreError,
        > {
            let mut map = std::collections::HashMap::new();
            for id in chunk_ids {
                if *id == self.chunk_id {
                    map.insert(
                        *id,
                        common::content_store::IndexedChunk {
                            chunk_id: self.chunk_id.to_string(),
                            doc_id: self.doc_id.to_string(),
                            page: Some(1),
                            content: "second-doc body".to_string(),
                            score: Some(0.9),
                            metadata: serde_json::json!({}),
                        },
                    );
                }
            }
            Ok(map)
        }

        async fn get_document_metadata_by_ids(
            &self,
            _auth: &AuthContext,
            _doc_ids: &[uuid::Uuid],
        ) -> anyhow::Result<Vec<common::DocumentMetadata>, common::content_store::ContentStoreError>
        {
            Ok(vec![])
        }

        async fn get_summary_metadata(
            &self,
            _auth: &AuthContext,
            _doc_ids: &[uuid::Uuid],
        ) -> anyhow::Result<Vec<common::SummaryMetadata>, common::content_store::ContentStoreError>
        {
            Ok(vec![])
        }

        async fn get_document_toc_entries(
            &self,
            _auth: &AuthContext,
            _doc_ids: &[uuid::Uuid],
        ) -> anyhow::Result<
            Vec<(uuid::Uuid, common::TocEntry)>,
            common::content_store::ContentStoreError,
        > {
            Ok(vec![])
        }

        async fn get_summary_chunks(
            &self,
            _auth: &AuthContext,
            _doc_ids: &[uuid::Uuid],
        ) -> anyhow::Result<Vec<(uuid::Uuid, String)>, common::content_store::ContentStoreError>
        {
            Ok(vec![])
        }

        async fn list_documents(
            &self,
            _auth: &AuthContext,
            _workspace_id: Option<uuid::Uuid>,
            _document_id: Option<uuid::Uuid>,
        ) -> anyhow::Result<Vec<common::Document>, common::content_store::ContentStoreError>
        {
            Ok(vec![])
        }

        async fn get_document_names(
            &self,
            _auth: &AuthContext,
            _doc_ids: &[uuid::Uuid],
        ) -> anyhow::Result<
            std::collections::HashMap<uuid::Uuid, String>,
            common::content_store::ContentStoreError,
        > {
            Ok(std::collections::HashMap::new())
        }
    }

    fn make_runtime_with_content_store(
        chunk_id: uuid::Uuid,
        doc_id: uuid::Uuid,
    ) -> Arc<RagRuntime> {
        let mut config = crate::test_doubles::test_rag_config();
        config.content_store = Some(Arc::new(StubContentStore { chunk_id, doc_id }));
        let data_plane: Arc<dyn avrag_retrieval_data_plane::RetrievalReadPort> =
            Arc::new(StubDataPlane {
                chunk_id,
                doc_id,
                graph_edge: false,
            });
        Arc::new(RagRuntime::with_data_plane(config, data_plane))
    }

    #[tokio::test]
    async fn chunk_fetch_resolves_chunk_in_second_scoped_doc() {
        // C10: chunk lives in the SECOND scoped doc — the old first-doc
        // shortcut silently returned []; the full-scope fan-out must find it.
        let chunk_id = uuid::Uuid::from_u128(1);
        let real_doc = uuid::Uuid::parse_str("00000000-0000-0000-0000-0000000000aa").unwrap();
        let other_doc = uuid::Uuid::parse_str("00000000-0000-0000-0000-0000000000bb").unwrap();
        let runtime = make_runtime_with_content_store(chunk_id, real_doc);
        let bridge = RuntimeBridge::new(
            runtime,
            make_auth(),
            vec![other_doc.to_string(), real_doc.to_string()],
        );
        let data = bridge
            .call("chunk_fetch", json!({"chunk_id": chunk_id.to_string()}))
            .await;
        let chunks = data["chunks"].as_array().expect("chunks array");
        assert_eq!(chunks.len(), 1, "{data}");
        assert_eq!(chunks[0]["content"], "second-doc body");
    }

    #[tokio::test]
    async fn chunk_fetch_out_of_scope_chunk_still_rejected() {
        // C10: scope contains two docs, neither owning the chunk → still an
        // honest empty (no wildcard leak outside the scope).
        let chunk_id = uuid::Uuid::from_u128(1);
        let real_doc = uuid::Uuid::parse_str("00000000-0000-0000-0000-0000000000aa").unwrap();
        let doc_b = uuid::Uuid::parse_str("00000000-0000-0000-0000-0000000000bb").unwrap();
        let doc_c = uuid::Uuid::parse_str("00000000-0000-0000-0000-0000000000cc").unwrap();
        let runtime = make_runtime_with_content_store(chunk_id, real_doc);
        let bridge = RuntimeBridge::new(
            runtime,
            make_auth(),
            vec![doc_b.to_string(), doc_c.to_string()],
        );
        let data = bridge
            .call("chunk_fetch", json!({"chunk_id": chunk_id.to_string()}))
            .await;
        let chunks = data["chunks"].as_array().expect("chunks array");
        assert!(chunks.is_empty(), "{data}");
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

    /// A1: lexical_search with augment on may return graph_context.
    #[tokio::test]
    async fn lexical_search_graph_augment_attaches_graph_context() {
        let _serial = tools::graph_augment::TEST_CONFIG_SERIAL
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        tools::graph_augment::install_test_config(tools::graph_augment::GraphAugmentConfig {
            enabled: true,
            ..Default::default()
        });

        let runtime = make_runtime_with_graph(true);
        let doc_scope = vec!["00000000-0000-0000-0000-000000000010".to_string()];
        let bridge = RuntimeBridge::new(runtime, make_auth(), doc_scope);
        let data = bridge
            .call("lexical_search", json!({"query": "DRC DRO", "top_k": 5}))
            .await;

        tools::graph_augment::clear_test_config();

        assert!(
            data.get("chunks").and_then(|c| c.as_array()).is_some(),
            "chunks present: {data}"
        );
        let gc = data
            .get("graph_context")
            .and_then(|c| c.as_array())
            .expect("graph_context array");
        assert!(!gc.is_empty(), "expected non-empty graph_context: {data}");
        assert_eq!(gc[0]["subject"], "DRC");
        assert_eq!(gc[0]["object"], "DRO");
        assert_eq!(gc[0]["hop"], 1);
        let evidence = gc[0]["evidence_chunks"]
            .as_array()
            .expect("evidence_chunks");
        assert!(!evidence.is_empty());
        assert_eq!(evidence[0]["kept_reason"], "top1");

        // P2: non-empty augment may emit telemetry graph_retrieval with degrade_reason=graph_augment.
        let captured = bridge.take_captured_results();
        assert_eq!(captured[0].tool, "lexical_retrieval");
        assert!(
            tools::graph_augment::graph_augment_hit(&captured),
            "expected graph_augment telemetry: {captured:?}"
        );
        assert!(
            !tools::graph_augment::graph_explicit_called(&captured),
            "augment must not count as explicit graph call: {captured:?}"
        );
    }

    /// A2: dense_search never attaches graph_context from augment.
    #[tokio::test]
    async fn dense_search_never_gets_graph_augment_sidecar() {
        let _serial = tools::graph_augment::TEST_CONFIG_SERIAL
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        tools::graph_augment::install_test_config(tools::graph_augment::GraphAugmentConfig {
            enabled: true,
            ..Default::default()
        });

        let runtime = make_runtime_with_graph(true);
        let doc_scope = vec!["00000000-0000-0000-0000-000000000010".to_string()];
        let bridge = RuntimeBridge::new(runtime, make_auth(), doc_scope);
        let data = bridge
            .call("dense_search", json!({"query": "DRC DRO", "top_k": 5}))
            .await;

        tools::graph_augment::clear_test_config();

        assert!(data.get("chunks").is_some());
        assert!(
            data.get("graph_context").is_none(),
            "dense must not get graph_context: {data}"
        );
    }

    #[tokio::test]
    async fn lexical_search_graph_augment_off_has_no_graph_context_key() {
        let _serial = tools::graph_augment::TEST_CONFIG_SERIAL
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        tools::graph_augment::install_test_config(tools::graph_augment::GraphAugmentConfig {
            enabled: false,
            ..Default::default()
        });

        let runtime = make_runtime_with_graph(true);
        let doc_scope = vec!["00000000-0000-0000-0000-000000000010".to_string()];
        let bridge = RuntimeBridge::new(runtime, make_auth(), doc_scope);
        let data = bridge
            .call("lexical_search", json!({"query": "DRC DRO", "top_k": 5}))
            .await;

        tools::graph_augment::clear_test_config();

        assert!(data.get("chunks").is_some());
        assert!(
            data.get("graph_context").is_none(),
            "switch off must not attach graph_context: {data}"
        );
    }

    #[test]
    fn lexical_zero_hit_hint_fires_only_on_zero() {
        assert!(lexical_zero_hit_hint("速冻机 年产", 3).is_none());
        assert!(lexical_zero_hit_hint("速冻机 年产", 1).is_none());
        let hint = lexical_zero_hit_hint("速冻机 年产", 0).expect("0 hits → hint");
        assert!(hint.contains("0 命中"), "{hint}");
        assert!(hint.contains("速冻机 年产"), "query quoted: {hint}");
        assert!(hint.contains("拆出核心专名"), "{hint}");
        assert!(hint.contains("换同义词"), "{hint}");
        assert!(hint.contains("doc_scan"), "{hint}");
        assert!(hint.contains("dense_search 复核"), "{hint}");
        assert!(hint.contains("避免对同一措辞连续无效重试"), "{hint}");
    }
}
