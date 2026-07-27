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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextSource {
    /// The synthesizer's cited chunks (preferred evidence).
    #[default]
    Cited,
    /// Retrieved chunks, used only when nothing was cited; the judge prompt
    /// must flag faithfulness accordingly (`context_source=retrieved_fallback`).
    RetrievedFallback,
    /// Non-RAG question (pure chat / tool use): nothing cited, nothing
    /// retrieved, and the golden example declares no evidence. The judge must
    /// not score faithfulness (see `build_user_prompt`).
    NoContext,
}

impl ContextSource {
    pub fn as_str(self) -> &'static str {
        match self {
            ContextSource::Cited => "cited",
            ContextSource::RetrievedFallback => "retrieved_fallback",
            ContextSource::NoContext => "no_context",
        }
    }

    /// Determine the context source for a question: cited chunks when present,
    /// retrieved chunks as the flagged fallback, and `NoContext` only when
    /// nothing was cited AND nothing was retrieved AND the golden example
    /// declares no evidence (`source_chunks` empty) — i.e. the question never
    /// expected retrieval in the first place.
    pub fn determine(
        example: &GoldenExample,
        retrieved: &RetrievedChunks,
        cited: &CitedChunks,
    ) -> Self {
        if !cited.is_empty() {
            return ContextSource::Cited;
        }
        if retrieved.is_empty() && example.source_chunks.is_empty() {
            return ContextSource::NoContext;
        }
        ContextSource::RetrievedFallback
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
    /// Golden `expect_no_retrieval`: memory/follow-up question answered from
    /// conversation context — the prompt tells the judge faithfulness is
    /// not_applicable here.
    #[serde(default)]
    pub expect_no_retrieval: bool,
}

impl JudgeInput {
    /// Build from a golden example, the run's retrieved/cited chunks, and the
    /// rendered answer. When the synthesizer cited nothing, the retrieved
    /// chunks (in first-seen rank order) stand in as context and the source is
    /// marked so the judge can discount accordingly (design §4.2 faithfulness
    /// rule). When nothing was cited, nothing was retrieved, and the golden
    /// declares no evidence, the source is `NoContext` and faithfulness must
    /// not be scored. Truncation for the prompt is P1's concern.
    pub fn new(
        example: &GoldenExample,
        retrieved: &RetrievedChunks,
        cited: &CitedChunks,
        answer: &str,
    ) -> Self {
        let context_source = ContextSource::determine(example, retrieved, cited);
        let cited_context = match context_source {
            ContextSource::Cited => cited.contents(),
            ContextSource::RetrievedFallback => retrieved.contents(),
            ContextSource::NoContext => Vec::new(),
        };
        Self {
            question: example.query.clone(),
            reference_answer: example.reference_answer().to_string(),
            expected_should_answer: example.expected_should_answer,
            model_answer: answer.to_string(),
            cited_context,
            context_source,
            rubric_notes: example.rubric_notes.clone(),
            expect_no_retrieval: example.expect_no_retrieval,
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
            expect_no_retrieval: false,
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

    #[test]
    fn no_context_only_when_nothing_cited_retrieved_or_expected() {
        // Pure-chat question: no cited, no retrieved, golden declares no evidence.
        let mut chat_ex = example();
        chat_ex.source_chunks = vec![];
        let input = JudgeInput::new(
            &chat_ex,
            &RetrievedChunks::default(),
            &CitedChunks::default(),
            "a",
        );
        assert_eq!(input.context_source, ContextSource::NoContext);
        assert_eq!(input.context_source.as_str(), "no_context");
        assert!(input.cited_context.is_empty());

        // Gold declares evidence but nothing came back → NOT no_context
        // (retrieval was expected; faithfulness still applies).
        let input = JudgeInput::new(
            &example(),
            &RetrievedChunks::default(),
            &CitedChunks::default(),
            "a",
        );
        assert_eq!(input.context_source, ContextSource::RetrievedFallback);
        assert!(input.cited_context.is_empty());

        // Retrieved chunks exist even though gold expects none → fallback.
        let input = JudgeInput::new(&chat_ex, &retrieved(), &CitedChunks::default(), "a");
        assert_eq!(input.context_source, ContextSource::RetrievedFallback);
        assert_eq!(input.cited_context, vec!["retrieved text".to_string()]);
    }
}
