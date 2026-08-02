use std::collections::HashSet;

use contracts::{AnswerContextChunk, RetrievalBundle};

/// Build answer-context chunks from a retrieval bundle (tool results path).
pub fn answer_context(bundle: &RetrievalBundle) -> Vec<AnswerContextChunk> {
    bundle.answer_context_chunks()
}

pub fn extract_referenced_chunk_ids(answer_text: &str) -> HashSet<String> {
    avrag_rag_core::runtime::markers::extract_chunk_ids(answer_text)
        .into_iter()
        .collect()
}
