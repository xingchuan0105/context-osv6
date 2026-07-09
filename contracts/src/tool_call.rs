use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use typeshare::typeshare;

use crate::rag_execute::{GraphHint, PlaceholderTriplet};

/// Tool catalog entry: describes one callable tool.
#[typeshare]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// Re-export the canonical types from chat to avoid duplication.
pub use crate::chat::{ToolResult, ToolStatus, ToolTrace};

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
    /// Document IDs to restrict the search scope.
    /// When empty, the search is unrestricted (org-wide).
    #[serde(default)]
    pub doc_scope: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenseRetrievalModality {
    Text,
    #[serde(alias = "image")]
    Mm,
    #[default]
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LexicalRetrievalArgs {
    pub terms: Vec<String>,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    /// Document IDs to restrict the search scope.
    /// When empty, the search is unrestricted (org-wide).
    #[serde(default)]
    pub doc_scope: Vec<String>,
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
    /// Document IDs to restrict the search scope.
    /// When empty, the search is unrestricted (org-wide).
    #[serde(default)]
    pub doc_scope: Vec<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocProfileArgs {
    pub doc_ids: Vec<String>,
    #[serde(default)]
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocChunksArgs {
    pub doc_ids: Vec<String>,
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dense_retrieval_args_default_modality_is_both() {
        let args: DenseRetrievalArgs =
            serde_json::from_str(r#"{"queries":["black swan"]}"#).unwrap();
        assert_eq!(args.modality, DenseRetrievalModality::Both);
    }

    #[test]
    fn dense_retrieval_modality_accepts_image_alias_for_mm() {
        let args: DenseRetrievalArgs = serde_json::from_value(serde_json::json!({
            "queries": ["test"],
            "modality": "image",
        }))
        .unwrap();
        assert_eq!(args.modality, DenseRetrievalModality::Mm);
    }
}
