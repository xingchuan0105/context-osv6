use std::collections::HashSet;

use contracts::{AnswerContextChunk, RetrievalBundle};

/// Build answer-context chunks from a retrieval bundle (tool results path).
pub fn answer_context(bundle: &RetrievalBundle) -> Vec<AnswerContextChunk> {
    bundle.answer_context_chunks()
}

pub fn extract_referenced_chunk_ids(answer_text: &str) -> HashSet<String> {
    let mut remaining = answer_text;
    let mut ids = HashSet::new();
    while let Some(start) = remaining.find("[[") {
        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find("]]") else {
            break;
        };
        let token = after_start[..end].trim();
        if let Some(chunk_id) = token.strip_prefix("cite:").map(str::trim) {
            if !chunk_id.is_empty() {
                ids.insert(chunk_id.to_string());
            }
        } else if let Some(chunk_id) = token.strip_prefix("image:").map(str::trim)
            && !chunk_id.is_empty()
        {
            ids.insert(chunk_id.to_string());
        }
        remaining = &after_start[end + 2..];
    }
    ids
}
