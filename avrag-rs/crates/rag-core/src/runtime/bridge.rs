//! Sandbox retrieval bridge — maps Python shim RPC to `RagRuntime` tool dispatch.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use avrag_auth::AuthContext;
use avrag_code_interpreter::HostBridge;
use common::{
    DenseRetrievalArgs, DenseRetrievalModality, DocSummaryArgs, DocSummaryLevel, GraphRetrievalArgs,
    IndexLookupArgs, LexicalRetrievalArgs, ToolCall, ToolResult, ToolStatus,
};
use serde_json::{json, Value};
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
                        modality: DenseRetrievalModality::Text,
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
                let terms: Vec<String> = query
                    .split_whitespace()
                    .map(ToOwned::to_owned)
                    .collect();
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
                let doc_ids = args
                    .get("doc_ids")
                    .and_then(|v| v.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .collect::<Vec<_>>()
                    })
                    .filter(|ids| !ids.is_empty())
                    .ok_or_else(|| Self::bridge_error("invalid_args", "doc_ids is required"))?;
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
            "dense_retrieval" | "lexical_retrieval" | "index_lookup" => {
                json!({ "chunks": chunks_with_content_field(data) })
            }
            "graph_retrieval" => json!({ "chunks": data }),
            "doc_summary" => json!({ "chunks": data }),
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
    use avrag_auth::{OrgId, SubjectKind};
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
    impl avrag_retrieval_data_plane::RetrievalDataPlane for StubDataPlane {
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
        let data_plane: Arc<dyn avrag_retrieval_data_plane::RetrievalDataPlane> =
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

    #[test]
    fn chunk_fetch_tool_call_allows_empty_doc_scope() {
        let runtime = make_runtime();
        let bridge = RuntimeBridge::new(runtime, make_auth(), vec![]);
        let call = bridge
            .method_to_tool_call(
                "chunk_fetch",
                &json!({"chunk_id": "00000000-0000-0000-0000-000000000001"}),
            )
            .expect("tool call");
        assert_eq!(call.tool, "index_lookup");
        let args: IndexLookupArgs = serde_json::from_value(call.args).unwrap();
        assert!(args.doc_id.is_empty());
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
        assert!(result.stdout.contains("bridge hit"), "stdout={}", result.stdout);
        assert!(result.stdout.contains("00000000-0000-0000-0000-000000000001"));
    }
}
