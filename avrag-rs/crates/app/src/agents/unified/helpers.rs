use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::runtime::AgentRunUsage;
use avrag_llm::LlmUsage;
use common::{AnswerContextChunk, Citation, SourceRef, ToolResult, ToolStatus};

pub fn merge_usage(existing: Option<&LlmUsage>, new: &LlmUsage) -> LlmUsage {
    match existing {
        Some(prev) => LlmUsage {
            provider: new.provider.clone(),
            model: new.model.clone(),
            prompt_tokens: prev.prompt_tokens.saturating_add(new.prompt_tokens),
            completion_tokens: prev.completion_tokens.saturating_add(new.completion_tokens),
            total_tokens: prev.total_tokens.saturating_add(new.total_tokens),
            cached_tokens: prev.cached_tokens.saturating_add(new.cached_tokens),
        },
        None => new.clone(),
    }
}

pub fn build_run_usage(usage: Option<&LlmUsage>, request_count: u64) -> Option<AgentRunUsage> {
    usage.map(|u| AgentRunUsage {
        provider: u.provider.clone(),
        model: u.model.clone(),
        prompt_tokens: u.prompt_tokens as u64,
        completion_tokens: u.completion_tokens as u64,
        total_tokens: u.total_tokens as u64,
        request_count,
    })
}

pub fn run_usage_to_agent_usage(usage: &AgentRunUsage) -> crate::agents::events::AgentUsage {
    crate::agents::events::AgentUsage {
        provider: usage.provider.clone(),
        model: usage.model.clone(),
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
    }
}

pub async fn emit_usage(sink: &dyn AgentEventSink, usage: Option<&AgentRunUsage>) {
    if let Some(u) = usage {
        let _ = sink
            .emit(AgentEvent::Usage {
                provider: u.provider.clone(),
                model: u.model.clone(),
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
                request_count: u.request_count,
                metadata: Default::default(),
            })
            .await;
    }
}

pub fn has_evidence(tool_results: &[ToolResult]) -> bool {
    tool_results.iter().any(|result| {
        result.status == ToolStatus::Ok
            && result
                .data
                .as_ref()
                .and_then(|data| data.as_array())
                .is_some_and(|array| !array.is_empty())
    })
}

pub fn extract_chunks_with_scores(tool_results: &[ToolResult]) -> Vec<(AnswerContextChunk, f32)> {
    let mut out = Vec::new();
    for result in tool_results {
        if result.status != ToolStatus::Ok {
            continue;
        }
        let Some(items) = result.data.as_ref().and_then(|data| data.as_array()) else {
            continue;
        };
        for item in items {
            let Some(chunk_id) = item
                .get("chunk_id")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .filter(|id| !id.is_empty())
            else {
                continue;
            };
            let doc_id = item.get("doc_id").and_then(|v| v.as_str()).map(str::to_owned);
            let text = item
                .get("text")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .unwrap_or_default();
            let page = item.get("page").and_then(|v| v.as_i64());
            let score = item.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;

            out.push((
                AnswerContextChunk {
                    chunk_id,
                    doc_id,
                    chunk_type: "text".to_string(),
                    page,
                    text,
                    asset_id: None,
                    caption: None,
                    image_url: None,
                    parser_backend: None,
                    source_locator: None,
                },
                score,
            ));
        }
    }
    out
}

pub fn build_citations_from_tool_results(tool_results: &[ToolResult]) -> Vec<Citation> {
    let mut citations = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut next_id: i64 = 1;

    for result in tool_results {
        if result.status != ToolStatus::Ok {
            continue;
        }
        let Some(items) = result.data.as_ref().and_then(|data| data.as_array()) else {
            continue;
        };
        for item in items {
            let Some(chunk_id) = item
                .get("chunk_id")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .filter(|id| !id.is_empty())
            else {
                continue;
            };
            if !seen.insert(chunk_id.clone()) {
                continue;
            }
            let doc_id = item
                .get("doc_id")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .unwrap_or_default();
            let text = item
                .get("text")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .unwrap_or_default();
            let page = item.get("page").and_then(|v| v.as_i64()).map(|p| p as usize);
            let score = item.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;

            citations.push(Citation {
                citation_id: next_id,
                doc_id: doc_id.clone(),
                chunk_id: Some(chunk_id),
                page,
                doc_name: doc_id,
                preview: Some(text.chars().take(200).collect()),
                content: Some(text),
                score,
                layer: Some(result.tool.clone()),
                chunk_type: Some("text".to_string()),
                asset_id: None,
                caption: None,
                image_url: None,
                parser_backend: None,
                source_locator: None,
                parse_run_id: None,
            });
            next_id += 1;
        }
    }
    citations
}

pub fn build_sources_from_tool_results(tool_results: &[ToolResult]) -> Vec<SourceRef> {
    let mut sources = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for result in tool_results {
        if result.status != ToolStatus::Ok {
            continue;
        }
        let Some(items) = result.data.as_ref().and_then(|data| data.as_array()) else {
            continue;
        };
        for item in items {
            let Some(chunk_id) = item
                .get("chunk_id")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .filter(|id| !id.is_empty())
            else {
                continue;
            };
            if !seen.insert(chunk_id.clone()) {
                continue;
            }
            let doc_id = item.get("doc_id").and_then(|v| v.as_str()).map(str::to_owned);
            let text = item
                .get("text")
                .and_then(|v| v.as_str())
                .map(|s| s.chars().take(200).collect::<String>());
            let page = item.get("page").and_then(|v| v.as_i64()).map(|p| p as usize);

            sources.push(SourceRef {
                id: chunk_id.clone(),
                title: format!("Chunk {chunk_id}"),
                snippet: text,
                doc_id,
                page,
            });
        }
    }
    sources
}

/// Drop the trailing whitespace-separated token from `query`.
pub fn broaden_query(query: &str) -> String {
    let words: Vec<&str> = query.split_whitespace().collect();
    if words.len() <= 1 {
        return query.to_string();
    }
    words[..words.len() - 1].join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::ToolResult;

    fn make_usage(prompt: u32, completion: u32) -> LlmUsage {
        LlmUsage {
            provider: "test".to_string(),
            model: "m".to_string(),
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens: prompt + completion,
            cached_tokens: 0,
        }
    }

    #[test]
    fn test_merge_usage_adds_tokens() {
        let a = make_usage(10, 5);
        let b = make_usage(3, 2);
        let merged = merge_usage(Some(&a), &b);
        assert_eq!(merged.prompt_tokens, 13);
        assert_eq!(merged.completion_tokens, 7);
        assert_eq!(merged.total_tokens, 20);
    }

    #[test]
    fn test_merge_usage_none_clones() {
        let b = make_usage(3, 2);
        let merged = merge_usage(None, &b);
        assert_eq!(merged.prompt_tokens, 3);
        assert_eq!(merged.completion_tokens, 2);
    }

    #[test]
    fn test_build_run_usage_maps_fields() {
        let u = make_usage(10, 5);
        let run = build_run_usage(Some(&u), 2).unwrap();
        assert_eq!(run.prompt_tokens, 10);
        assert_eq!(run.completion_tokens, 5);
        assert_eq!(run.total_tokens, 15);
        assert_eq!(run.request_count, 2);
    }

    #[test]
    fn test_build_run_usage_none_returns_none() {
        assert!(build_run_usage(None, 0).is_none());
    }

    fn tr(tool: &str, status: ToolStatus, data: Option<serde_json::Value>) -> ToolResult {
        ToolResult {
            tool: tool.to_string(),
            version: "1.0".to_string(),
            status,
            data,
            trace: None,
        }
    }

    #[test]
    fn test_has_evidence_true_when_non_empty_array() {
        let results = vec![tr("t", ToolStatus::Ok, Some(serde_json::json!([{"chunk_id": "c1"}])))];
        assert!(has_evidence(&results));
    }

    #[test]
    fn test_has_evidence_false_when_empty_array() {
        let results = vec![tr("t", ToolStatus::Ok, Some(serde_json::json!([])))];
        assert!(!has_evidence(&results));
    }

    #[test]
    fn test_has_evidence_false_when_no_data() {
        let results = vec![tr("t", ToolStatus::Ok, None)];
        assert!(!has_evidence(&results));
    }

    #[test]
    fn test_extract_chunks_with_scores_filters_ok_and_array() {
        let results = vec![
            tr("dense", ToolStatus::Ok, Some(serde_json::json!([
                {"chunk_id": "c1", "text": "hello", "score": 0.9}
            ]))),
            tr("fail", ToolStatus::Error, Some(serde_json::json!([{"chunk_id": "c2"}]))),
        ];
        let chunks = extract_chunks_with_scores(&results);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].0.chunk_id, "c1");
        assert_eq!(chunks[0].0.text, "hello");
        assert_eq!(chunks[0].1, 0.9);
    }

    #[test]
    fn test_extract_chunks_skips_missing_chunk_id() {
        let results = vec![tr("t", ToolStatus::Ok, Some(serde_json::json!([{"text": "no id"}])))];
        assert!(extract_chunks_with_scores(&results).is_empty());
    }

    #[test]
    fn test_build_citations_dedupes_by_chunk_id() {
        let results = vec![tr("dense", ToolStatus::Ok, Some(serde_json::json!([
            {"chunk_id": "c1", "doc_id": "d1", "text": "text1", "score": 0.8},
            {"chunk_id": "c1", "doc_id": "d1", "text": "text1", "score": 0.8}
        ])))];
        let citations = build_citations_from_tool_results(&results);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].citation_id, 1);
    }

    #[test]
    fn test_build_sources_dedupes_by_chunk_id() {
        let results = vec![tr("t", ToolStatus::Ok, Some(serde_json::json!([
            {"chunk_id": "c1", "doc_id": "d1"},
            {"chunk_id": "c1", "doc_id": "d1"}
        ])))];
        let sources = build_sources_from_tool_results(&results);
        assert_eq!(sources.len(), 1);
    }

    #[test]
    fn test_broaden_query_drops_last_token() {
        assert_eq!(broaden_query("a b c"), "a b");
    }

    #[test]
    fn test_broaden_query_single_word_unchanged() {
        assert_eq!(broaden_query("hello"), "hello");
    }

    #[test]
    fn test_broaden_query_empty() {
        assert_eq!(broaden_query(""), "");
    }
}
