//! Judge input artifact (design §4.2).
//!
//! `JudgeInput` is everything one judge call needs: the question, the golden
//! reference answer, the refusal expectation, the model answer, and the
//! context the answer should be grounded in — cited chunks first, retrieved
//! chunks as a flagged fallback when the synthesizer cited nothing.

use crate::golden_set::GoldenExample;
use crate::harness_extract::{CitedChunks, RetrievedChunks};
use serde::{Deserialize, Serialize};

/// Where `JudgeInput::cited_context` came from (design §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextSource {
    /// The synthesizer's cited chunks (preferred evidence).
    Cited,
    /// Retrieved chunks, used only when nothing was cited; the judge prompt
    /// must flag faithfulness accordingly (`context_source=retrieved_fallback`).
    RetrievedFallback,
}

impl ContextSource {
    pub fn as_str(self) -> &'static str {
        match self {
            ContextSource::Cited => "cited",
            ContextSource::RetrievedFallback => "retrieved_fallback",
        }
    }
}

/// One judge call's input (design §4.2 user-message payload).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeInput {
    pub question: String,
    pub reference_answer: String,
    pub expected_should_answer: bool,
    pub model_answer: String,
    /// Chunk texts the answer must be grounded in (see `context_source`).
    pub cited_context: Vec<String>,
    pub context_source: ContextSource,
    /// Optional golden rubric notes passed through to the judge prompt.
    #[serde(default)]
    pub rubric_notes: Option<String>,
}

impl JudgeInput {
    /// Build from a golden example, the run's retrieved/cited chunks, and the
    /// rendered answer. When the synthesizer cited nothing, the retrieved
    /// chunks (in first-seen rank order) stand in as context and the source is
    /// marked so the judge can discount accordingly (design §4.2 faithfulness
    /// rule). Truncation for the prompt is P1's concern.
    pub fn new(
        example: &GoldenExample,
        retrieved: &RetrievedChunks,
        cited: &CitedChunks,
        answer: &str,
    ) -> Self {
        let (cited_context, context_source) = if cited.is_empty() {
            (retrieved.contents(), ContextSource::RetrievedFallback)
        } else {
            (cited.contents(), ContextSource::Cited)
        };
        Self {
            question: example.query.clone(),
            reference_answer: example.reference_answer().to_string(),
            expected_should_answer: example.expected_should_answer,
            model_answer: answer.to_string(),
            cited_context,
            context_source,
            rubric_notes: example.rubric_notes.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::golden_set::ChunkMatch;
    use crate::harness_extract::{CitedChunk, RetrievedChunk};

    fn example() -> GoldenExample {
        GoldenExample {
            query: "Y公司哪一年在大连建厂？".to_string(),
            expected_answer: "Y公司2019年在大连建厂。".to_string(),
            source_chunks: vec![ChunkMatch::Substring {
                text: "2019年于大连市投资建厂".to_string(),
            }],
            expected_citations: vec![],
            mode: "rag".to_string(),
            description: String::new(),
            is_adversarial: false,
            expected_should_answer: true,
            refusal_keywords: vec![],
            must_include: vec![],
            must_not_include: vec![],
            retrieval_hints: vec![],
            difficulty: Default::default(),
            relevance_grades: Default::default(),
            expected_tool: None,
            expected_tool_sequence: None,
            requires_triplet_reingest: false,
            capabilities: vec![],
            doc_scope_hint: "all".to_string(),
            expect_citations: None,
            requires_network: false,
            prior_turns: vec![],
            client_time: None,
            rubric_notes: Some("接受「2019 年」「2019年」".to_string()),
        }
    }

    fn retrieved() -> RetrievedChunks {
        RetrievedChunks {
            chunks: vec![RetrievedChunk {
                chunk_id: "c0".to_string(),
                content: "retrieved text".to_string(),
                score: Some(0.9),
                rank: 0,
                tool: "dense_retrieval".to_string(),
            }],
        }
    }

    #[test]
    fn cited_chunks_are_preferred_context() {
        let cited = CitedChunks {
            chunks: vec![CitedChunk {
                chunk_id: Some("c0".to_string()),
                citation_id: 1,
                content: "cited text".to_string(),
                score: 0.9,
            }],
        };
        let input = JudgeInput::new(&example(), &retrieved(), &cited, "2019年在大连建厂");
        assert_eq!(input.question, "Y公司哪一年在大连建厂？");
        assert_eq!(input.reference_answer, "Y公司2019年在大连建厂。");
        assert!(input.expected_should_answer);
        assert_eq!(input.model_answer, "2019年在大连建厂");
        assert_eq!(input.cited_context, vec!["cited text".to_string()]);
        assert_eq!(input.context_source, ContextSource::Cited);
        assert_eq!(input.context_source.as_str(), "cited");
        assert_eq!(input.rubric_notes.as_deref(), Some("接受「2019 年」「2019年」"));
    }

    #[test]
    fn empty_citations_fall_back_to_retrieved() {
        let input = JudgeInput::new(&example(), &retrieved(), &CitedChunks::default(), "a");
        assert_eq!(input.cited_context, vec!["retrieved text".to_string()]);
        assert_eq!(input.context_source, ContextSource::RetrievedFallback);
        assert_eq!(input.context_source.as_str(), "retrieved_fallback");
    }
}
