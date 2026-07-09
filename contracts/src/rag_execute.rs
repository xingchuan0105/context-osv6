//! Retrieval result DTOs shared across agent tools and response building.
//!
//! Multi-channel `ExecutePlanRequest` / `ExecutePlanResponse` were removed (ADR-0006 /
//! TN Wave 2 physical delete). Product path is AgentLoop + `ToolCall` only.

use serde::{Deserialize, Serialize};

use crate::chat::{Citation, RagTraceItem, RagTraceSummary};
use crate::documents::AnswerContextChunk;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryEntity {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphHint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predicate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlaceholderTriplet {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceholderTripletType {
    Fuzzy,     // 2+ placeholders
    Traceable, // 1 placeholder
    Resolved,  // 0 placeholders
}

impl PlaceholderTriplet {
    pub fn classify(&self) -> PlaceholderTripletType {
        let placeholder_count = self.subject.starts_with('?') as usize
            + self.predicate.starts_with('?') as usize
            + self.object.starts_with('?') as usize;
        match placeholder_count {
            0 => PlaceholderTripletType::Resolved,
            1 => PlaceholderTripletType::Traceable,
            _ => PlaceholderTripletType::Fuzzy,
        }
    }

    /// Known entities (non-placeholder subject/object). Predicate is not an entity.
    pub fn known_entities(&self) -> Vec<String> {
        let mut entities = Vec::new();
        if !self.subject.starts_with('?') {
            entities.push(self.subject.clone());
        }
        if !self.object.starts_with('?') {
            entities.push(self.object.clone());
        }
        entities
    }

    pub fn placeholder_positions(&self) -> Vec<&'static str> {
        let mut positions = Vec::new();
        if self.subject.starts_with('?') {
            positions.push("subject");
        }
        if self.predicate.starts_with('?') {
            positions.push("predicate");
        }
        if self.object.starts_with('?') {
            positions.push("object");
        }
        positions
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreBreakdown {
    pub channel: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rerank_score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedChunk {
    pub chunk_id: String,
    pub doc_id: String,
    pub chunk_type: String,
    #[serde(default)]
    pub page: Option<i64>,
    pub text: String,
    pub score: f32,
    pub retrieval_channel: String,
    #[serde(default)]
    pub asset_id: Option<String>,
    #[serde(default)]
    pub caption: Option<String>,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub parser_backend: Option<String>,
    #[serde(default)]
    pub source_locator: Option<serde_json::Value>,
    #[serde(default)]
    pub parse_run_id: Option<String>,
    #[serde(default)]
    pub score_breakdown: Vec<ScoreBreakdown>,
}

impl RetrievedChunk {
    pub fn as_answer_context_chunk(&self) -> AnswerContextChunk {
        AnswerContextChunk {
            chunk_id: self.chunk_id.clone(),
            doc_id: Some(self.doc_id.clone()),
            chunk_type: self.chunk_type.clone(),
            page: self.page,
            text: self.text.clone(),
            asset_id: self.asset_id.clone(),
            caption: self.caption.clone(),
            image_url: self.image_url.clone(),
            parser_backend: self.parser_backend.clone(),
            source_locator: self.source_locator.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationPath {
    pub path_id: String,
    #[serde(default)]
    pub entities: Vec<String>,
    #[serde(default)]
    pub relations: Vec<String>,
    #[serde(default)]
    pub supporting_chunk_ids: Vec<String>,
    pub score: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RetrievalBundle {
    #[serde(default)]
    pub chunks: Vec<RetrievedChunk>,
    #[serde(default)]
    pub graph_supported_chunks: Vec<RetrievedChunk>,
    #[serde(default)]
    pub relation_paths: Vec<RelationPath>,
    #[serde(default)]
    pub citations: Vec<Citation>,
    #[serde(default)]
    pub summary_chunks: Vec<AnswerContextChunk>,
}

impl RetrievalBundle {
    pub fn answer_context_chunks(&self) -> Vec<AnswerContextChunk> {
        let mut chunks = self
            .chunks
            .iter()
            .map(RetrievedChunk::as_answer_context_chunk)
            .collect::<Vec<_>>();
        chunks.extend(
            self.graph_supported_chunks
                .iter()
                .map(RetrievedChunk::as_answer_context_chunk),
        );
        chunks.extend(self.summary_chunks.clone());
        chunks
    }

    /// All citation-eligible chunks; regular chunks first, graph chunks de-duplicated.
    pub fn citation_chunks(&self) -> Vec<&RetrievedChunk> {
        let mut all_chunks =
            Vec::with_capacity(self.chunks.len() + self.graph_supported_chunks.len());
        all_chunks.extend(&self.chunks);
        let regular_ids: std::collections::HashSet<_> =
            self.chunks.iter().map(|c| &c.chunk_id).collect();
        for chunk in &self.graph_supported_chunks {
            if !regular_ids.contains(&chunk.chunk_id) {
                all_chunks.push(chunk);
            }
        }
        all_chunks
    }

    pub fn has_evidence(&self) -> bool {
        !self.chunks.is_empty()
            || !self.graph_supported_chunks.is_empty()
            || !self.summary_chunks.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelCoverage {
    pub text_dense: usize,
    pub bm25: usize,
    pub multimodal_dense: usize,
    pub graph: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coverage {
    pub requested_doc_count: usize,
    pub matched_doc_count: usize,
    pub retrieved_chunk_count: usize,
    pub summary_chunk_count: usize,
    #[serde(default)]
    pub channel_coverage: ChannelCoverage,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelTraceItem {
    pub channel: String,
    pub raw_count: usize,
    pub hydrated_count: usize,
    pub selected_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degrade_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendTrace {
    #[serde(default)]
    pub item_trace: Vec<RagTraceItem>,
    #[serde(default)]
    pub channel_trace: Vec<ChannelTraceItem>,
    pub retrieval_trace: RagTraceSummary,
}
