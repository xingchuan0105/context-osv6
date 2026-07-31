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

/// doc_grep (2026-07-29, grep 化检索替代 doc_scan): coding-agent 语义的行级
/// 检索——关键词/正则、命中计数、行号、上下文。完备性由 total_hits 承载。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocGrepArgs {
    pub pattern: String,
    #[serde(default)]
    pub doc_ids: Vec<String>,
    /// true → Rust regex 语法；false → 字面子串。
    #[serde(default)]
    pub regex: bool,
    /// 每个命中行两侧携带的上下文行数（0-3）。
    #[serde(default)]
    pub context: u32,
    /// 返回命中上限（默认 50，硬顶 200）；total_hits 不受其影响。
    #[serde(default)]
    pub max_hits: Option<u32>,
}

/// doc_read_lines: 按行号区间读取文档原文（与 doc_grep 同一虚拟行视图）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocReadLinesArgs {
    pub doc_id: String,
    /// 1-based 起始行（含）。
    pub start: u32,
    /// 1-based 结束行（含）；区间硬顶 400 行。
    pub end: u32,
}

/// struct_catalog (2026-07-31, docs/plans/2026-07-31-struct-query-virtual-tables.md):
/// 列出 doc scope 内 per-doc DuckDB 表格存储中的 relation。无存储/无表 → relations 为空（ok）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructCatalogArgs {
    #[serde(default)]
    pub doc_ids: Vec<String>,
}

/// struct_query: 在表格存储上执行受限 SQL（单条 SELECT、标识符 ∈ catalog、只读加固）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructQueryArgs {
    pub sql: String,
    #[serde(default)]
    pub doc_ids: Vec<String>,
}

/// Tolerate singular `doc_id` (string or array) as an alias for `doc_ids`.
/// Models frequently emit the singular form; strict `deny_unknown_fields`
/// rejection burns a whole retrieval retry (2026-07-18 incident).
pub fn normalize_doc_id_alias(args: &mut serde_json::Value) {
    let Some(obj) = args.as_object_mut() else {
        return;
    };
    let Some(v) = obj.remove("doc_id") else {
        return;
    };
    let mut ids: Vec<serde_json::Value> = match v {
        serde_json::Value::String(s) => vec![serde_json::Value::String(s)],
        serde_json::Value::Array(a) => a,
        other => vec![other],
    };
    match obj.get_mut("doc_ids") {
        Some(serde_json::Value::Array(existing)) => {
            existing.append(&mut ids);
        }
        _ => {
            obj.insert("doc_ids".to_string(), serde_json::Value::Array(ids));
        }
    }
}

#[cfg(test)]
mod alias_tests {
    use super::normalize_doc_id_alias;

    #[test]
    fn singular_string_becomes_doc_ids() {
        let mut v = serde_json::json!({"doc_id": "abc"});
        normalize_doc_id_alias(&mut v);
        assert_eq!(v, serde_json::json!({"doc_ids": ["abc"]}));
    }

    #[test]
    fn singular_array_merges_with_existing() {
        let mut v = serde_json::json!({"doc_ids": ["a"], "doc_id": ["b", "c"]});
        normalize_doc_id_alias(&mut v);
        assert_eq!(v, serde_json::json!({"doc_ids": ["a", "b", "c"]}));
    }

    #[test]
    fn no_alias_no_change() {
        let mut v = serde_json::json!({"doc_ids": ["a"]});
        normalize_doc_id_alias(&mut v);
        assert_eq!(v, serde_json::json!({"doc_ids": ["a"]}));
    }
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
