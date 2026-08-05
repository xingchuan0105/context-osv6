//! Sandbox retrieval bridge — maps Python shim RPC to `RagRuntime` tool dispatch.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use avrag_code_interpreter::HostBridge;
use contracts::auth_runtime::AuthContext;
use contracts::{
    DenseRetrievalArgs, DenseRetrievalModality, DocProfileArgs, DocSummaryArgs, DocSummaryLevel,
    LexicalRetrievalArgs, ToolCall, ToolResult, ToolStatus,
};
use serde_json::{Value, json};
use tracing::info;

use super::tools;
use crate::RagRuntime;

/// One sandbox `client.*` call with product-progress metadata (not for UI raw tool dump).
#[derive(Debug, Clone)]
pub struct CapturedBridgeCall {
    /// SDK method name (registry canonical id, e.g. `dense`).
    pub method: String,
    /// Human query / terms from call args when present.
    pub query: Option<String>,
    pub result: ToolResult,
}

/// Host-side bridge backed by `RagRuntime` tool dispatch.
pub struct RuntimeBridge {
    runtime: Arc<RagRuntime>,
    auth: AuthContext,
    doc_scope: Vec<String>,
    captured_results: Arc<Mutex<Vec<ToolResult>>>,
    captured_calls: Arc<Mutex<Vec<CapturedBridgeCall>>>,
    /// K2: retrieval-log alias counter (`#1 #2 …`), shared across all blocks
    /// of one worker run (per-worker namespace; agent-loop IterationState
    /// owns the Arc). Tests default to a fresh counter.
    alias_counter: Arc<std::sync::atomic::AtomicU64>,
    /// Cross-round `chunk_id → first alias` for reseen closure (any member of
    /// an S+L run maps to the same alias). Shared across the run.
    seen_chunk_aliases: Arc<Mutex<std::collections::HashMap<String, String>>>,
    /// Durable full body text keyed by chunk_id (for card/expand plan).
    seen_chunk_bodies: Arc<Mutex<std::collections::HashMap<String, String>>>,
}

impl RuntimeBridge {
    pub fn new(runtime: Arc<RagRuntime>, auth: AuthContext, doc_scope: Vec<String>) -> Self {
        Self {
            runtime,
            auth,
            doc_scope,
            captured_results: Arc::new(Mutex::new(Vec::new())),
            captured_calls: Arc::new(Mutex::new(Vec::new())),
            alias_counter: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            seen_chunk_aliases: Arc::new(Mutex::new(std::collections::HashMap::new())),
            seen_chunk_bodies: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// K2: share one alias counter across the worker run's blocks.
    pub fn with_alias_counter(mut self, counter: Arc<std::sync::atomic::AtomicU64>) -> Self {
        self.alias_counter = counter;
        self
    }

    /// Share cross-round chunk→alias map for body dedupe / reseen closure.
    pub fn with_seen_chunk_aliases(
        mut self,
        map: Arc<Mutex<std::collections::HashMap<String, String>>>,
    ) -> Self {
        self.seen_chunk_aliases = map;
        self
    }

    /// Share durable body store (optional; default private map).
    pub fn with_seen_chunk_bodies(
        mut self,
        map: Arc<Mutex<std::collections::HashMap<String, String>>>,
    ) -> Self {
        self.seen_chunk_bodies = map;
        self
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
            "dense" | "lexical" | "web" => args
                .get("query")
                .and_then(|v| v.as_str())
                .map(str::to_owned),
            "fetch" => args.get("url").and_then(|v| v.as_str()).map(str::to_owned),
            _ => None,
        }
    }

    /// SaC retrieval methods implemented by this host (parity with shim minus
    /// base/web ports, which the composite host in agent-loop fills).
    ///
    /// Derived from the `contracts::sdk_primitives` registry RAG face (D10);
    /// full product surface: `avrag_code_interpreter::bridge_shim_client_method_names`.
    pub fn supported_method_names() -> &'static [&'static str] {
        use contracts::sdk_primitives::{SdkCapability, ids_for};
        static NAMES: std::sync::OnceLock<&'static [&'static str]> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| {
            let ids = ids_for(SdkCapability::RAG);
            Box::leak(ids.into_boxed_slice())
        })
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
    /// Uses the single shared `intersect_doc_scope` (scoped_rag_dispatch): the
    /// LLM/codegen caller can never widen scope beyond what the session established.
    fn resolve_doc_ids(&self, caller: &[String]) -> Vec<String> {
        super::scoped_rag_dispatch::intersect_doc_scope(caller, &self.doc_scope)
    }

    fn method_to_tool_call(&self, method: &str, args: &Value) -> Result<ToolCall, Value> {
        // Host-fixed top_k (A4): SDK never exposes top_k; contracts default is 10.
        const HOST_TOP_K: usize = 10;
        match method {
            "dense" => {
                let query = args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Self::bridge_error("invalid_args", "query is required"))?;
                Ok(ToolCall {
                    tool: "dense_retrieval".to_string(),
                    version: "1.0".to_string(),
                    args: serde_json::to_value(DenseRetrievalArgs {
                        queries: vec![query.to_string()],
                        modality: DenseRetrievalModality::Both,
                        top_k: HOST_TOP_K,
                        doc_scope: self.doc_scope.clone(),
                    })
                    .unwrap_or_default(),
                })
            }
            "lexical" => {
                let query = args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Self::bridge_error("invalid_args", "query is required"))?;
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
                        top_k: HOST_TOP_K,
                        doc_scope: self.doc_scope.clone(),
                    })
                    .unwrap_or_default(),
                })
            }
            // `client.graph` removed from SaC surface: graph expand is inside dense (VGRAG).
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
            // grep: line-level locate + exact total_hits (A4: sole line primitive).
            // doc_ids intersect session scope (resolve_doc_ids).
            "grep" => {
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
                let pattern = args
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                Ok(ToolCall {
                    tool: "doc_grep".to_string(),
                    version: "1.0".to_string(),
                    args: serde_json::to_value(contracts::DocGrepArgs {
                        pattern,
                        doc_ids,
                        regex: args.get("regex").and_then(|v| v.as_bool()).unwrap_or(false),
                        context: args.get("context").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                        max_hits: args
                            .get("max_hits")
                            .and_then(|v| v.as_u64())
                            .map(|v| v as u32),
                    })
                    .unwrap_or_default(),
                })
            }
            // struct_catalog / struct_query: per-doc DuckDB 表格存储(2026-07-31 计划)。
            // doc_ids 与 grep 同款 scope 交叉。
            "struct_catalog" => {
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
                    tool: "struct_catalog".to_string(),
                    version: "1.0".to_string(),
                    args: serde_json::to_value(contracts::StructCatalogArgs { doc_ids })
                        .unwrap_or_default(),
                })
            }
            "struct_query" => {
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
                let sql = args
                    .get("sql")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                Ok(ToolCall {
                    tool: "struct_query".to_string(),
                    version: "1.0".to_string(),
                    args: serde_json::to_value(contracts::StructQueryArgs { sql, doc_ids })
                        .unwrap_or_default(),
                })
            }
            // Non-retrieval SaC ports are owned by the composite host (agent-loop).
            "web" | "fetch" | "history" | "user_profile" | "save" | "load"
            | "user_context" | "calculator" | "weather_query" => Err(Self::bridge_error(
                "not_configured",
                format!(
                    "{method} is a SaC SDK method but this RuntimeBridge has no web/memory/fs/tool port; \
                     use the product SacHostBridge"
                ),
            )),
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
            "dense_retrieval" | "index_lookup" => {
                // K1: native results are now object-shaped ({chunks, hint…})
                // — the sandbox only ever sees the chunk list.
                let inner = data.get("chunks").unwrap_or(data);
                json!({ "chunks": chunks_with_content_field(inner) })
            }
            // grep: full payload (total_hits / line hits) — do not strip to list.
            "doc_grep" => data.clone(),
            // struct_*: full payload (relations / rows+evidence) — 同 grep 不削形。
            "struct_catalog" | "struct_query" => data.clone(),
            // lexical may already be `{ chunks, graph_context }` (A5: graph bound to lexical).
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
            "doc_summary" | "doc_profile" => json!({ "chunks": data }),
            "web_search" | "web_fetch" | "conversation_history_load" | "user_profile_load" => {
                data.clone()
            }
            _ => json!({ "chunks": data }),
        }
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
        let canonical = method;
        let tool_call = match self.method_to_tool_call(method, &args) {
            Ok(call) => call,
            Err(err) => return err,
        };

        // Lexical force-augment runs inside lexical_retrieval (A5). dense never
        // gets graph_context. Bridge only surfaces + telemetry-splits.
        let result = tools::dispatch(&self.runtime, &self.auth, &tool_call).await;

        self.captured_results
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(result.clone());
        self.captured_calls
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(CapturedBridgeCall {
                method: canonical.to_string(),
                query: Self::extract_query(method, &args),
                result: result.clone(),
            });
        let mut data = Self::tool_result_to_bridge_data(&result);

        // K2: inject retrieval-log alias (`#1 #2 …`) into chunk lists the sandbox sees.
        // struct_query's `chunks` carrier is the table-level evidence md (query
        // result set rendered), so it joins the same alias namespace.
        const ALIASED_METHODS: &[&str] = &["dense", "lexical", "grep", "struct_query"];
        if ALIASED_METHODS.contains(&canonical)
            && let Some(items) = data.get_mut("chunks").and_then(|v| v.as_array_mut())
        {
            {
                let mut seen = self
                    .seen_chunk_aliases
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                for item in items.iter_mut() {
                    let members = super::visibility::member_ids_from_item(item);
                    if members.is_empty() {
                        continue;
                    }
                    // Reseen if **any** member already registered (S+L closure).
                    let prev = members.iter().find_map(|m| seen.get(m).cloned());
                    if let Some(prev_alias) = prev {
                        item["alias"] = json!(prev_alias.clone());
                        item["reseen"] = json!(prev_alias);
                        item["body_omitted"] = json!(true);
                        if let Some(obj) = item.as_object_mut() {
                            if let Some(t) = obj.get_mut("text") {
                                *t = json!("");
                            }
                            if let Some(t) = obj.get_mut("content") {
                                *t = json!("");
                            }
                        }
                    } else {
                        let n = self
                            .alias_counter
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                            + 1;
                        let alias = format!("#{n}");
                        item["alias"] = json!(alias.clone());
                        for m in members {
                            seen.insert(m, alias.clone());
                        }
                    }
                }
            }
            // U-P1: expand / card / stub (adjacent always expand).
            let mut bodies = self
                .seen_chunk_bodies
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let (exp_n, card_n, stub_n, exp_chars) = super::visibility::apply_visibility_to_chunks(
                items,
                &mut bodies,
                super::visibility::EXPAND_CHAR_BUDGET_PER_CALL,
            );
            if let Some(obj) = data.as_object_mut() {
                obj.insert("visibility_expanded_n".into(), json!(exp_n));
                obj.insert("visibility_card_n".into(), json!(card_n));
                obj.insert("visibility_stub_n".into(), json!(stub_n));
                obj.insert("visibility_expand_chars".into(), json!(exp_chars));
            }
        }

        // Telemetry: non-empty graph_context → side-car degrade_reason=graph_augment.
        let graph_context_count = if canonical == "lexical" {
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
            if canonical == "dense" {
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
            bridge_method = canonical,
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
    use avrag_retrieval_data_plane::{
        Bm25SearchOutput, Bm25SearchRequest, Bm25SearchTrace, GraphSearchOutput,
        GraphSearchRequest, MultimodalSearchRequest, RelationPathCandidate, ScoredChunk,
        TextDenseSearchRequest,
    };
    use contracts::auth_runtime::{SubjectKind, UserId};
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
            cursor: None,
            member_chunk_ids: vec![],
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
            cursor: None,
            member_chunk_ids: vec![],
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
                    doc_id: self.doc_id,
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
            cursor: None,
            member_chunk_ids: vec![],
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
            cursor: None,
            member_chunk_ids: vec![],
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
    fn bridge_retrieval_methods_are_subset_of_shim() {
        let host = RuntimeBridge::supported_method_names();
        let shim = avrag_code_interpreter::bridge_shim_client_method_names();
        for m in host {
            assert!(
                shim.contains(m),
                "host retrieval method {m} missing from shim: {shim:?}"
            );
        }
        // A4/A5: removed from SaC surface.
        for banned in ["graph_search", "chunk_fetch", "read_lines", "doc_scan"] {
            assert!(!host.contains(&banned));
            assert!(!shim.contains(&banned));
        }
        // Web/memory/fs live on the composite SacHostBridge (agent-loop).
        for extra in ["web", "fetch", "history", "user_profile", "save", "load"] {
            assert!(shim.contains(&extra), "shim must list {extra}");
            assert!(
                !host.contains(&extra),
                "RuntimeBridge must not claim {extra} (no web/memory/fs port)"
            );
        }
    }

    #[tokio::test]
    async fn runtime_bridge_dense_returns_chunks_with_content() {
        let runtime = make_runtime();
        let doc_scope = vec!["00000000-0000-0000-0000-000000000010".to_string()];
        let bridge = RuntimeBridge::new(runtime, make_auth(), doc_scope);
        let data = bridge
            .call("dense", json!({"query": "antifragility"}))
            .await;
        let chunks = data["chunks"].as_array().expect("chunks array");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0]["content"], "bridge hit");
        assert_eq!(
            chunks[0]["chunk_id"],
            "00000000-0000-0000-0000-000000000001"
        );
    }

    #[test]
    fn dense_ignores_caller_top_k() {
        let runtime = make_runtime();
        let bridge = RuntimeBridge::new(runtime, make_auth(), vec![]);
        let call = bridge
            .method_to_tool_call("dense", &json!({"query": "x", "top_k": 99}))
            .expect("tool call");
        let args: DenseRetrievalArgs = serde_json::from_value(call.args).unwrap();
        assert_eq!(args.top_k, 10, "host fixes top_k; SDK must not control it");
    }

    #[tokio::test]
    async fn runtime_bridge_grep_returns_full_payload() {
        // grep (doc_scan 继任者): 沙箱拿到完整载荷——total_hits 精确计数、
        // 行号命中，不削成行列表（计数语义不能被 chunks 化丢失）。
        let runtime = make_runtime();
        let doc_scope = vec!["00000000-0000-0000-0000-000000000010".to_string()];
        let bridge = RuntimeBridge::new(runtime, make_auth(), doc_scope);
        let data = bridge.call("grep", json!({"pattern": "scan"})).await;
        assert_eq!(data["total_hits"], 1, "{data}");
        assert_eq!(data["returned"], 1, "{data}");
        assert_eq!(data["truncated"], false, "{data}");
        let hits = data["hits"].as_array().expect("hits array");
        assert_eq!(hits[0]["line"], 1);
        assert_eq!(hits[0]["text"], "scan hit");
    }

    #[test]
    fn removed_methods_are_unknown() {
        let runtime = make_runtime();
        let bridge = RuntimeBridge::new(runtime, make_auth(), vec![]);
        for method in ["graph_search", "chunk_fetch", "read_lines"] {
            let err = bridge
                .method_to_tool_call(method, &json!({"query": "x", "chunk_id": "c"}))
                .expect_err("removed method");
            assert_eq!(err["error"]["code"], "unknown_method", "{method}: {err}");
        }
    }

    #[tokio::test]
    async fn runtime_bridge_forces_doc_scope() {
        let runtime = make_runtime();
        let forced_scope = vec!["00000000-0000-0000-0000-000000000099".to_string()];
        let bridge = RuntimeBridge::new(runtime, make_auth(), forced_scope.clone());
        let call = bridge
            .method_to_tool_call("dense", &json!({"query": "x"}))
            .expect("tool call");
        let args: DenseRetrievalArgs = serde_json::from_value(call.args).unwrap();
        assert_eq!(args.doc_scope, forced_scope);
    }

    #[test]
    fn doc_summary_caller_doc_ids_outside_scope_narrowed_to_scope() {
        // The codegen sandbox requests a doc id outside the session scope; the bridge
        // must not honor the request. With fail-closed intersection semantics the
        // resolved doc set is empty, so the doc-centric tool rejects the call rather
        // than widening to the whole session scope.
        let runtime = make_runtime();
        let scope = vec!["00000000-0000-0000-0000-000000000010".to_string()];
        let bridge = RuntimeBridge::new(runtime, make_auth(), scope.clone());
        let err = bridge
            .method_to_tool_call(
                "doc_summary",
                &json!({"doc_ids": ["00000000-0000-0000-0000-000000000099"]}),
            )
            .expect_err("out-of-scope caller must fail closed");
        assert_eq!(err["error"]["code"], "invalid_args");
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
chunks = await client.dense(query="antifragility")
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
        let data = bridge.call("lexical", json!({"query": "DRC DRO"})).await;

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
        assert_eq!(gc[0]["expansion_hop_limit"], 1);
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

    /// A5: dense never attaches graph_context from augment.
    #[tokio::test]
    async fn dense_never_gets_graph_augment_sidecar() {
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
        let data = bridge.call("dense", json!({"query": "DRC DRO"})).await;

        tools::graph_augment::clear_test_config();

        assert!(data.get("chunks").is_some());
        assert!(
            data.get("graph_context").is_none(),
            "dense must not get graph_context: {data}"
        );
    }

    #[tokio::test]
    async fn lexical_graph_augment_off_has_no_graph_context_key() {
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
        let data = bridge.call("lexical", json!({"query": "DRC DRO"})).await;

        tools::graph_augment::clear_test_config();

        assert!(data.get("chunks").is_some());
        assert!(
            data.get("graph_context").is_none(),
            "switch off must not attach graph_context: {data}"
        );
    }

    // ---- K2: retrieval-log alias injection ----------------------------------

    #[tokio::test]
    async fn aliases_increment_across_methods_within_one_worker() {
        // Stub plane always returns the same chunk_id: first hit assigns #1;
        // later hits are reseen (same alias, body omitted) — multi-round P0.
        let runtime = make_runtime();
        let doc_scope = vec!["00000000-0000-0000-0000-000000000010".to_string()];
        let bridge = RuntimeBridge::new(runtime, make_auth(), doc_scope);

        let d1 = bridge
            .call("dense", json!({"query": "antifragility"}))
            .await;
        let d2 = bridge.call("dense", json!({"query": "again"})).await;
        let d3 = bridge.call("lexical", json!({"query": "third"})).await;

        let alias_of = |data: &Value| data["chunks"][0]["alias"].as_str().unwrap().to_string();
        assert_eq!(alias_of(&d1), "#1");
        assert_eq!(
            alias_of(&d2),
            "#1",
            "same stub chunk_id reseens first alias"
        );
        assert!(
            d2["chunks"][0].get("reseen").is_some(),
            "second dense should mark reseen: {}",
            d2
        );
        assert_eq!(
            alias_of(&d3),
            "#1",
            "lexical same stub chunk shares reseen alias"
        );
    }

    #[tokio::test]
    async fn two_workers_have_independent_alias_namespaces() {
        let runtime = make_runtime();
        let doc_scope = vec!["00000000-0000-0000-0000-000000000010".to_string()];
        let counter_a = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let counter_b = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let worker_a = RuntimeBridge::new(Arc::clone(&runtime), make_auth(), doc_scope.clone())
            .with_alias_counter(Arc::clone(&counter_a));
        let worker_b = RuntimeBridge::new(runtime, make_auth(), doc_scope)
            .with_alias_counter(Arc::clone(&counter_b));

        let a1 = worker_a.call("dense", json!({"query": "q"})).await;
        let a2 = worker_a.call("dense", json!({"query": "q"})).await;
        let b1 = worker_b.call("dense", json!({"query": "q"})).await;

        assert_eq!(a1["chunks"][0]["alias"], "#1");
        assert_eq!(
            a2["chunks"][0]["alias"], "#1",
            "reseen keeps first alias within worker"
        );
        assert!(a2["chunks"][0].get("reseen").is_some());
        assert_eq!(b1["chunks"][0]["alias"], "#1", "per-worker namespace");
        // Only one *new* alias assignment per worker (second a-call reseens).
        assert_eq!(counter_a.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert_eq!(counter_b.load(std::sync::atomic::Ordering::Relaxed), 1);
    }
}
