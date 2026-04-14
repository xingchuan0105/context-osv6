use std::sync::Arc;

use common::AnswerContextChunk;
use avrag_llm::AnswerSynthesizer;

use crate::SearchResult;

pub(crate) async fn synthesize_answer(
    query: &str,
    results: &[SearchResult],
    synthesizer: Option<&Arc<AnswerSynthesizer>>,
) -> anyhow::Result<String> {
    if results.is_empty() {
        if let Some(synthesizer) = synthesizer {
            return synthesizer
                .synthesize(query, &[], &None, &[], None)
                .await
                .map(|output| output.answer_text)
                .or_else(|_| {
                    Ok("No external evidence was found for this query. Please try a more specific query."
                        .to_string())
                });
        }
        return Ok(
            "No external evidence was found for this query. Please try a more specific query."
                .to_string(),
        );
    }

    if let Some(synthesizer) = synthesizer {
        let context_chunks: Vec<AnswerContextChunk> = results
            .iter()
            .enumerate()
            .map(|(index, item)| AnswerContextChunk {
                chunk_id: format!("search_result_{}", index + 1),
                doc_id: Some(item.url.clone()),
                chunk_type: "search_result".to_string(),
                page: None,
                text: format!(
                    "Title: {}\nURL: {}\nSnippet: {}",
                    item.title, item.url, item.snippet
                ),
                caption: None,
                image_url: None,
            })
            .collect();
        return synthesizer
            .synthesize(query, &context_chunks, &None, &[], None)
            .await
            .map(|output| output.answer_text)
            .or_else(|_| Ok(build_fallback_synthesis(results)));
    }

    Ok(build_fallback_synthesis(results))
}

pub(crate) fn build_fallback_synthesis(results: &[SearchResult]) -> String {
    let lines = results
        .iter()
        .take(5)
        .map(|item| format!("- {}: {}", item.title, item.snippet))
        .collect::<Vec<_>>();
    format!("I found these external sources:\n{}", lines.join("\n"))
}
