use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    ExecutePlanItem, ExecutePlanRequest, ExecutePlanSummaryMode, GraphHint, PlaceholderTriplet,
    QueryEntity,
};

/// Tool catalog entry: describes one callable tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub version: String,
    pub description: String,
    /// JSON Schema for the `args` field of a ToolCall.
    pub input_schema: serde_json::Value,
    /// JSON Schema for the `data` field of a ToolResult.
    pub output_schema: serde_json::Value,
}

/// A single tool invocation emitted by the planner.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolCall {
    pub tool: String,
    pub version: String,
    pub args: serde_json::Value,
}

/// Execution status for a single tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Ok,
    Timeout,
    Error,
    NotFound,
    NotImplemented,
}

/// Per-tool execution trace (latency, hit counts, degrade reason).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolTrace {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_hit_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hydrated_hit_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degrade_reason: Option<String>,
}

/// Result returned by the runtime for one ToolCall.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolResult {
    pub tool: String,
    pub version: String,
    pub status: ToolStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<ToolTrace>,
}

/// Planner decides what to do after emitting calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NextStep {
    Answer,
    Replan,
}

fn default_next_step() -> NextStep {
    NextStep::Answer
}

/// Full planner output in the tool-call paradigm.
/// Renamed from `PlannerOutput` to avoid collision with `contracts::chat::PlannerOutput`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetrievalPlannerOutput {
    pub calls: Vec<ToolCall>,
    #[serde(default = "default_next_step")]
    pub next_step: NextStep,
    /// Optional output-format skills selected by the planner for the Answer phase.
    #[serde(default)]
    pub skills: Vec<String>,
    /// Writing styles selected by the planner for the Answer phase.
    #[serde(default)]
    pub writing_styles: Vec<String>,
    /// Behavior mode selected by the planner for the Answer phase.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behavior_mode: Option<String>,
}

/// Optional merge strategy for the external `/runtime/execute` endpoint.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MergeConfig {
    pub strategy: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub weights: HashMap<String, f32>,
}

/// Request body for `POST /v1/runtime/execute`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeExecuteRequest {
    pub calls: Vec<ToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge: Option<MergeConfig>,
}

/// Response body for `POST /v1/runtime/execute`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeExecuteResponse {
    pub results: Vec<ToolResult>,
}

// ---------------------------------------------------------------------------
// Strongly-typed args for each known tool (used by the adapter below).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DenseRetrievalArgs {
    pub queries: Vec<String>,
    #[serde(default)]
    pub modality: DenseRetrievalModality,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenseRetrievalModality {
    #[default]
    Text,
    Mm,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LexicalRetrievalArgs {
    pub terms: Vec<String>,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphRetrievalArgs {
    #[serde(default)]
    pub graph_hints: Vec<GraphHint>,
    #[serde(default)]
    pub placeholder_triplets: Vec<PlaceholderTriplet>,
    #[serde(default = "default_relation_limit")]
    pub relation_limit: usize,
    #[serde(default = "default_supporting_chunk_limit")]
    pub supporting_chunk_limit: usize,
    #[serde(default = "default_hop_limit")]
    pub hop_limit: usize,
    #[serde(default = "default_fan_out_limit")]
    pub fan_out_limit: usize,
    /// Optional original user query for reranking relation paths.
    #[serde(default)]
    pub query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexLookupArgs {
    pub doc_id: String,
    pub chunk_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocSummaryArgs {
    pub doc_ids: Vec<String>,
    #[serde(default)]
    pub level: DocSummaryLevel,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocSummaryLevel {
    #[default]
    Doc,
    Section,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocMetadataArgs {
    pub doc_ids: Vec<String>,
    #[serde(default)]
    pub fields: Vec<String>,
}

fn default_top_k() -> usize {
    10
}

fn default_hop_limit() -> usize {
    1
}

fn default_fan_out_limit() -> usize {
    10
}

fn default_relation_limit() -> usize {
    20
}

fn default_supporting_chunk_limit() -> usize {
    10
}

// ---------------------------------------------------------------------------
// Adapter: convert a Vec<ToolCall> into the legacy ExecutePlanRequest.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, thiserror::Error)]
pub enum ToolCallAdapterError {
    #[error("tool '{0}' is not supported by the legacy adapter")]
    UnsupportedTool(String),
    #[error("failed to deserialize args for tool '{0}': {1}")]
    InvalidArgs(String, String),
    #[error("no retrievable calls provided")]
    EmptyCalls,
    #[error("too many retrieval calls: {0} > max {1}")]
    TooManyItems(usize, usize),
}

impl ExecutePlanRequest {
    /// Convert a list of `ToolCall`s into the legacy `ExecutePlanRequest`.
    ///
    /// This is a **compatibility shim** for Phase 1: the new planner emits
    /// `ToolCall[]`, but the existing runtime still consumes `ExecutePlanRequest`.
    /// Tools that have no legacy equivalent (`index_lookup`, `doc_metadata`)
    /// return `ToolCallAdapterError::UnsupportedTool`.
    pub fn from_tool_calls(
        doc_scope: Vec<String>,
        calls: Vec<ToolCall>,
    ) -> Result<Self, ToolCallAdapterError> {
        let mut items: Vec<ExecutePlanItem> = Vec::new();
        let mut graph_hints: Vec<GraphHint> = Vec::new();
        let mut placeholder_triplets: Vec<PlaceholderTriplet> = Vec::new();
        let query_entities: Vec<QueryEntity> = Vec::new();
        let mut summary_mode = ExecutePlanSummaryMode::None;

        for call in calls {
            match call.tool.as_str() {
                "dense_retrieval" => {
                    let args: DenseRetrievalArgs = serde_json::from_value(call.args).map_err(
                        |e| ToolCallAdapterError::InvalidArgs(call.tool.clone(), e.to_string()),
                    )?;
                    for (idx, query) in args.queries.into_iter().enumerate() {
                        let priority = 1.0 - (idx as f32 * 0.1);
                        items.push(ExecutePlanItem {
                            priority: priority.clamp(0.1, 1.0),
                            query: Some(query),
                            bm25_terms: None,
                        });
                    }
                }
                "lexical_retrieval" => {
                    let args: LexicalRetrievalArgs = serde_json::from_value(call.args).map_err(
                        |e| ToolCallAdapterError::InvalidArgs(call.tool.clone(), e.to_string()),
                    )?;
                    items.push(ExecutePlanItem {
                        priority: 1.0,
                        query: None,
                        bm25_terms: Some(args.terms),
                    });
                }
                "graph_retrieval" => {
                    let args: GraphRetrievalArgs = serde_json::from_value(call.args).map_err(
                        |e| ToolCallAdapterError::InvalidArgs(call.tool.clone(), e.to_string()),
                    )?;
                    graph_hints.extend(args.graph_hints);
                    placeholder_triplets.extend(args.placeholder_triplets);
                }
                "doc_summary" => {
                    let args: DocSummaryArgs = serde_json::from_value(call.args).map_err(
                        |e| ToolCallAdapterError::InvalidArgs(call.tool.clone(), e.to_string()),
                    )?;
                    summary_mode = match args.level {
                        DocSummaryLevel::Doc => ExecutePlanSummaryMode::All,
                        DocSummaryLevel::Section => ExecutePlanSummaryMode::Related,
                    };
                }
                "index_lookup" => {
                    return Err(ToolCallAdapterError::UnsupportedTool(call.tool));
                }
                "doc_metadata" => {
                    return Err(ToolCallAdapterError::UnsupportedTool(call.tool));
                }
                other => {
                    return Err(ToolCallAdapterError::UnsupportedTool(other.to_string()));
                }
            }
        }

        let has_retrieval = !items.is_empty()
            || !graph_hints.is_empty()
            || !placeholder_triplets.is_empty();
        let has_summary = summary_mode != ExecutePlanSummaryMode::None;
        if !has_retrieval && !has_summary {
            return Err(ToolCallAdapterError::EmptyCalls);
        }

        if items.len() > Self::MAX_ITEMS {
            return Err(ToolCallAdapterError::TooManyItems(
                items.len(),
                Self::MAX_ITEMS,
            ));
        }

        Ok(Self {
            plan_version: "rag-execute-v1".to_string(),
            doc_scope,
            items,
            summary_mode,
            budget: None,
            channel_budget: None,
            query_entities,
            graph_hints,
            placeholder_triplets,
            trace: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dense_call(queries: Vec<&str>) -> ToolCall {
        ToolCall {
            tool: "dense_retrieval".to_string(),
            version: "1.0".to_string(),
            args: serde_json::to_value(DenseRetrievalArgs {
                queries: queries.into_iter().map(ToOwned::to_owned).collect(),
                modality: DenseRetrievalModality::Text,
                top_k: 10,
            })
            .unwrap(),
        }
    }

    fn lexical_call(terms: Vec<&str>) -> ToolCall {
        ToolCall {
            tool: "lexical_retrieval".to_string(),
            version: "1.0".to_string(),
            args: serde_json::to_value(LexicalRetrievalArgs {
                terms: terms.into_iter().map(ToOwned::to_owned).collect(),
                top_k: 10,
            })
            .unwrap(),
        }
    }

    fn graph_call(hints: Vec<GraphHint>, triplets: Vec<PlaceholderTriplet>) -> ToolCall {
        ToolCall {
            tool: "graph_retrieval".to_string(),
            version: "1.0".to_string(),
            args: serde_json::to_value(GraphRetrievalArgs {
                graph_hints: hints,
                placeholder_triplets: triplets,
                relation_limit: 20,
                supporting_chunk_limit: 10,
                hop_limit: 1,
                fan_out_limit: 10,
                query: None,
            })
            .unwrap(),
        }
    }

    fn summary_call(level: DocSummaryLevel) -> ToolCall {
        ToolCall {
            tool: "doc_summary".to_string(),
            version: "1.0".to_string(),
            args: serde_json::to_value(DocSummaryArgs {
                doc_ids: vec!["d1".to_string()],
                level,
            })
            .unwrap(),
        }
    }

    #[test]
    fn single_dense_retrieval_maps_to_items() {
        let req = ExecutePlanRequest::from_tool_calls(
            vec!["doc-1".to_string()],
            vec![dense_call(vec!["what is RAG"])],
        )
        .unwrap();
        assert_eq!(req.items.len(), 1);
        assert_eq!(req.items[0].query, Some("what is RAG".to_string()));
        assert!(req.items[0].bm25_terms.is_none());
        assert!((req.items[0].priority - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn multiple_dense_queries_get_decreasing_priority() {
        let req = ExecutePlanRequest::from_tool_calls(
            vec!["doc-1".to_string()],
            vec![dense_call(vec!["query A", "query B", "query C"])],
        )
        .unwrap();
        assert_eq!(req.items.len(), 3);
        assert!((req.items[0].priority - 1.0).abs() < f32::EPSILON);
        assert!((req.items[1].priority - 0.9).abs() < f32::EPSILON);
        assert!((req.items[2].priority - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn lexical_retrieval_maps_to_bm25_terms() {
        let req = ExecutePlanRequest::from_tool_calls(
            vec!["doc-1".to_string()],
            vec![lexical_call(vec!["X-2024-A", "recall"])],
        )
        .unwrap();
        assert_eq!(req.items.len(), 1);
        assert!(req.items[0].query.is_none());
        assert_eq!(
            req.items[0].bm25_terms,
            Some(vec!["X-2024-A".to_string(), "recall".to_string()])
        );
    }

    #[test]
    fn mixed_dense_and_lexical_produces_multiple_items() {
        let req = ExecutePlanRequest::from_tool_calls(
            vec!["doc-1".to_string()],
            vec![dense_call(vec!["semantic"]), lexical_call(vec!["literal"])],
        )
        .unwrap();
        assert_eq!(req.items.len(), 2);
        assert_eq!(req.items[0].query, Some("semantic".to_string()));
        assert_eq!(req.items[1].bm25_terms, Some(vec!["literal".to_string()]));
    }

    #[test]
    fn graph_retrieval_maps_to_hints_and_triplets() {
        let req = ExecutePlanRequest::from_tool_calls(
            vec!["doc-1".to_string()],
            vec![graph_call(
                vec![GraphHint {
                    subject: Some("Alice".to_string()),
                    predicate: None,
                    object: Some("Bob".to_string()),
                }],
                vec![PlaceholderTriplet {
                    subject: "?x".to_string(),
                    predicate: "leads".to_string(),
                    object: "Team A".to_string(),
                }],
            )],
        )
        .unwrap();
        assert_eq!(req.graph_hints.len(), 1);
        assert_eq!(req.placeholder_triplets.len(), 1);
        assert!(req.items.is_empty());
    }

    #[test]
    fn doc_summary_doc_level_maps_to_all() {
        let req = ExecutePlanRequest::from_tool_calls(
            vec!["doc-1".to_string()],
            vec![summary_call(DocSummaryLevel::Doc)],
        )
        .unwrap();
        assert_eq!(req.summary_mode, ExecutePlanSummaryMode::All);
    }

    #[test]
    fn doc_summary_section_level_maps_to_related() {
        let req = ExecutePlanRequest::from_tool_calls(
            vec!["doc-1".to_string()],
            vec![summary_call(DocSummaryLevel::Section)],
        )
        .unwrap();
        assert_eq!(req.summary_mode, ExecutePlanSummaryMode::Related);
    }

    #[test]
    fn index_lookup_returns_unsupported() {
        let result = ExecutePlanRequest::from_tool_calls(
            vec!["doc-1".to_string()],
            vec![ToolCall {
                tool: "index_lookup".to_string(),
                version: "1.0".to_string(),
                args: serde_json::json!({"doc_id": "d1", "chunk_ids": ["c1"]}),
            }],
        );
        assert!(matches!(
            result,
            Err(ToolCallAdapterError::UnsupportedTool(t)) if t == "index_lookup"
        ));
    }

    #[test]
    fn doc_metadata_returns_unsupported() {
        let result = ExecutePlanRequest::from_tool_calls(
            vec!["doc-1".to_string()],
            vec![ToolCall {
                tool: "doc_metadata".to_string(),
                version: "1.0".to_string(),
                args: serde_json::json!({"doc_ids": ["d1"], "fields": ["title"]}),
            }],
        );
        assert!(matches!(
            result,
            Err(ToolCallAdapterError::UnsupportedTool(t)) if t == "doc_metadata"
        ));
    }

    #[test]
    fn unknown_tool_returns_unsupported() {
        let result = ExecutePlanRequest::from_tool_calls(
            vec!["doc-1".to_string()],
            vec![ToolCall {
                tool: "web_search".to_string(),
                version: "1.0".to_string(),
                args: serde_json::json!({"query": "foo"}),
            }],
        );
        assert!(matches!(
            result,
            Err(ToolCallAdapterError::UnsupportedTool(t)) if t == "web_search"
        ));
    }

    #[test]
    fn empty_calls_returns_error() {
        let result = ExecutePlanRequest::from_tool_calls(vec!["doc-1".to_string()], vec![]);
        assert!(matches!(result, Err(ToolCallAdapterError::EmptyCalls)));
    }

    #[test]
    fn too_many_dense_queries_exceeds_max_items() {
        let result = ExecutePlanRequest::from_tool_calls(
            vec!["doc-1".to_string()],
            vec![dense_call(vec!["q1", "q2", "q3", "q4", "q5"])],
        );
        assert!(matches!(
            result,
            Err(ToolCallAdapterError::TooManyItems(5, 4))
        ));
    }

    #[test]
    fn invalid_args_returns_deserialization_error() {
        let result = ExecutePlanRequest::from_tool_calls(
            vec!["doc-1".to_string()],
            vec![ToolCall {
                tool: "dense_retrieval".to_string(),
                version: "1.0".to_string(),
                args: serde_json::json!({"queries": "not-an-array"}),
            }],
        );
        assert!(matches!(
            result,
            Err(ToolCallAdapterError::InvalidArgs(tool, _)) if tool == "dense_retrieval"
        ));
    }

    #[test]
    fn dense_plus_graph_produces_both_channels() {
        let req = ExecutePlanRequest::from_tool_calls(
            vec!["doc-1".to_string()],
            vec![
                dense_call(vec!["semantic query"]),
                graph_call(
                    vec![GraphHint {
                        subject: Some("A".to_string()),
                        predicate: Some("uses".to_string()),
                        object: Some("B".to_string()),
                    }],
                    vec![],
                ),
            ],
        )
        .unwrap();
        assert_eq!(req.items.len(), 1);
        assert_eq!(req.graph_hints.len(), 1);
    }
}
