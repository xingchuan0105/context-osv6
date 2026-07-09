//! Extraction of retrieval-layer vs selection-layer chunks from a `ChatResponse`.
//!
//! The RAG agent loop emits two distinct chunk signals into `ChatResponse`:
//!
//! - **Retrieval layer** (`tool_results`): every chunk returned by retrieval
//!   tools (`dense_retrieval` / `lexical_retrieval` / `graph_retrieval` /
//!   `index_lookup`) across all loop rounds. This is what the *retriever*
//!   actually found, independent of what the synthesizer chose to cite.
//! - **Selection layer** (`citations`): the chunks the synthesizer picked to
//!   back the final answer.
//!
//! Previous versions of the harness built the `chunks` list from `citations`,
//! which conflated the two layers — `Recall@15` then measured
//! "retrieve + LLM-select", not retrieval. This module keeps them separate so
//! `metrics_v2` can score each layer independently.
//!
//! ## Data shape
//!
//! Real retrieval tools (`rag-core/src/runtime/tools/{dense,lexical}.rs`) emit
//! `ToolResult.data` as a **JSON array** of chunk objects with a `text` field.
//! The codegen-bridge fallback (`codegen_bridge.rs`) also emits an array, with
//! `text` normalized from `content`. A few paths wrap chunks under a `chunks`
//! key. We handle both.

use contracts::chat::Citation;
use contracts::{ToolResult, ToolStatus};
use serde::{Deserialize, Serialize};

/// Tools whose `data` carries retrieved chunks (the retrieval layer).
pub const RETRIEVAL_TOOLS: &[&str] = &[
    "dense_retrieval",
    "lexical_retrieval",
    "graph_retrieval",
    "index_lookup",
];

/// A single chunk recovered from the retrieval layer, with rank/score for nDCG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedChunk {
    pub chunk_id: String,
    pub content: String,
    pub score: Option<f32>,
    /// 0-indexed position in first-seen dedup order across all rounds.
    pub rank: usize,
    /// Which retrieval tool produced this chunk.
    pub tool: String,
}

/// All retrieved chunks across all loop rounds, deduped by `chunk_id`,
/// preserving first-seen order (which is the retriever's effective ranking
/// when results from multiple rounds/tools are concatenated).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RetrievedChunks {
    pub chunks: Vec<RetrievedChunk>,
}

impl RetrievedChunks {
    pub fn contents(&self) -> Vec<String> {
        self.chunks.iter().map(|c| c.content.clone()).collect()
    }

    pub fn chunk_ids(&self) -> Vec<String> {
        self.chunks.iter().map(|c| c.chunk_id.clone()).collect()
    }

    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }
}

/// A chunk the synthesizer selected into the final answer (selection layer).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitedChunk {
    pub chunk_id: Option<String>,
    pub citation_id: i64,
    pub content: String,
    pub score: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CitedChunks {
    pub chunks: Vec<CitedChunk>,
}

impl CitedChunks {
    pub fn contents(&self) -> Vec<String> {
        self.chunks.iter().map(|c| c.content.clone()).collect()
    }

    pub fn chunk_ids(&self) -> Vec<String> {
        self.chunks
            .iter()
            .filter_map(|c| c.chunk_id.clone())
            .collect()
    }

    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }
}

/// Pull the chunk array out of a retrieval `ToolResult.data`, handling both
/// `data: [...]` and `data: {"chunks": [...]}` shapes. Returns `None` if the
/// result is not a retrieval tool, not `Ok`, or carries no chunk array.
fn chunk_array(data: &serde_json::Value) -> Option<&Vec<serde_json::Value>> {
    if let Some(arr) = data.as_array() {
        return Some(arr);
    }
    data.get("chunks").and_then(|v| v.as_array())
}

/// Extract deduped retrieved chunks from all retrieval tool results.
///
/// Iterates `tool_results` in order, keeping only `RETRIEVAL_TOOLS` entries
/// with `status == Ok`, and dedupes by `chunk_id` (first-seen wins, which
/// preserves the retriever's ranking for nDCG/MRR).
pub fn extract_retrieved_chunks(tool_results: &[ToolResult]) -> RetrievedChunks {
    let mut chunks = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for result in tool_results {
        if result.status != ToolStatus::Ok {
            continue;
        }
        if !RETRIEVAL_TOOLS.contains(&result.tool.as_str()) {
            continue;
        }
        let Some(data) = &result.data else { continue };
        let Some(arr) = chunk_array(data) else {
            continue;
        };

        for chunk in arr {
            let Some(chunk_id) = chunk.get("chunk_id").and_then(|v| v.as_str()) else {
                continue;
            };
            if chunk_id.is_empty() {
                continue;
            }
            if !seen.insert(chunk_id.to_string()) {
                continue;
            }
            // Real retrieval tools use `text`; codegen-bridge fallback normalizes
            // `content` -> `text`, but tolerate either for robustness.
            let content = chunk
                .get("text")
                .and_then(|v| v.as_str())
                .or_else(|| chunk.get("content").and_then(|v| v.as_str()))
                .unwrap_or("")
                .to_string();
            let score = chunk
                .get("score")
                .and_then(|v| v.as_f64())
                .map(|f| f as f32);
            chunks.push(RetrievedChunk {
                chunk_id: chunk_id.to_string(),
                content,
                score,
                rank: chunks.len(),
                tool: result.tool.clone(),
            });
        }
    }

    RetrievedChunks { chunks }
}

/// Extract the synthesizer's selected citations (selection layer).
pub fn extract_cited_chunks(citations: &[Citation]) -> CitedChunks {
    let chunks = citations
        .iter()
        .map(|c| CitedChunk {
            chunk_id: c.chunk_id.clone(),
            citation_id: c.citation_id,
            content: c.content.clone().unwrap_or_default(),
            score: c.score,
        })
        .collect();
    CitedChunks { chunks }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::ToolTrace;
    use contracts::chat::Citation;

    fn tr(tool: &str, data: serde_json::Value) -> ToolResult {
        ToolResult {
            tool: tool.to_string(),
            version: "1.0".to_string(),
            status: ToolStatus::Ok,
            data: Some(data),
            trace: None::<ToolTrace>,
        }
    }

    #[test]
    fn extracts_array_shape_with_text_field() {
        let results = vec![tr(
            "dense_retrieval",
            serde_json::json!([
                {"chunk_id": "c1", "doc_id": "d1", "text": "alpha beta", "score": 0.9},
                {"chunk_id": "c2", "doc_id": "d1", "text": "gamma", "score": 0.7}
            ]),
        )];
        let got = extract_retrieved_chunks(&results);
        assert_eq!(got.len(), 2);
        assert_eq!(got.chunks[0].content, "alpha beta");
        assert_eq!(got.chunks[0].rank, 0);
        assert_eq!(got.chunks[0].score, Some(0.9));
        assert_eq!(got.chunks[1].rank, 1);
    }

    #[test]
    fn extracts_chunks_key_shape_with_content_field() {
        let results = vec![tr(
            "lexical_retrieval",
            serde_json::json!({"chunks": [
                {"chunk_id": "c1", "content": "from content key"}
            ]}),
        )];
        let got = extract_retrieved_chunks(&results);
        assert_eq!(got.len(), 1);
        assert_eq!(got.chunks[0].content, "from content key");
    }

    #[test]
    fn dedupes_by_chunk_id_across_tools_and_skips_non_retrieval() {
        let results = vec![
            tr(
                "dense_retrieval",
                serde_json::json!([{"chunk_id": "c1", "text": "a"}]),
            ),
            tr(
                "lexical_retrieval",
                serde_json::json!([{"chunk_id": "c1", "text": "a"}, {"chunk_id": "c2", "text": "b"}]),
            ),
            tr("code_gen", serde_json::json!({"result": "ignored"})),
        ];
        let got = extract_retrieved_chunks(&results);
        assert_eq!(got.chunk_ids(), vec!["c1", "c2"]);
    }

    #[test]
    fn skips_failed_and_empty() {
        let results = vec![
            ToolResult {
                tool: "dense_retrieval".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!([{"chunk_id": "c1", "text": "a"}])),
                trace: None,
            },
            tr("dense_retrieval", serde_json::json!([])),
        ];
        let got = extract_retrieved_chunks(&results);
        assert!(got.is_empty());
    }

    #[test]
    fn cited_chunks_round_trip() {
        let citations = vec![Citation {
            citation_id: 1,
            doc_id: "d1".to_string(),
            chunk_id: Some("c1".to_string()),
            page: None,
            doc_name: "doc".to_string(),
            preview: None,
            content: Some("cited text".to_string()),
            score: 0.8,
            layer: None,
            chunk_type: None,
            asset_id: None,
            caption: None,
            image_url: None,
            parser_backend: None,
            source_locator: None,
            parse_run_id: None,
        }];
        let got = extract_cited_chunks(&citations);
        assert_eq!(got.len(), 1);
        assert_eq!(got.contents(), vec!["cited text".to_string()]);
        assert_eq!(got.chunk_ids(), vec!["c1".to_string()]);
    }
}
