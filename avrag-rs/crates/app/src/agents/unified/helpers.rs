use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::runtime::AgentRunUsage;
use avrag_llm::LlmUsage;
use common::{AnswerContextChunk, Citation, DegradeTraceItem, SourceRef, ToolResult, ToolStatus};

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
        cached_tokens: u.cached_tokens as u64,
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
            let doc_id = item
                .get("doc_id")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
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
            let page = item
                .get("page")
                .and_then(|v| v.as_i64())
                .map(|p| p as usize);
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

pub fn build_search_citations_from_tool_results(tool_results: &[ToolResult]) -> Vec<Citation> {
    let mut citations = Vec::new();
    let mut next_id: i64 = 1;

    for result in tool_results {
        if result.tool != "web_search" || result.status != ToolStatus::Ok {
            continue;
        }
        let Some(data) = result.data.as_ref() else {
            continue;
        };
        let Ok(response) = serde_json::from_value::<avrag_search::SearchResponse>(data.clone())
        else {
            continue;
        };
        for search_result in response.results {
            let citation_id = search_result
                .citation_index
                .map(|index| index as i64)
                .unwrap_or(next_id);
            citations.push(Citation {
                citation_id,
                doc_id: search_result.url.clone(),
                chunk_id: None,
                page: None,
                doc_name: search_result.title.clone(),
                preview: Some(search_result.snippet.chars().take(200).collect()),
                content: Some(search_result.snippet.clone()),
                score: 1.0,
                layer: Some("search".to_string()),
                chunk_type: Some("web".to_string()),
                asset_id: None,
                caption: None,
                image_url: None,
                parser_backend: None,
                source_locator: None,
                parse_run_id: None,
            });
            next_id = citation_id + 1;
        }
    }
    citations
}

pub fn build_all_citations_from_tool_results(tool_results: &[ToolResult]) -> Vec<Citation> {
    let mut citations = build_citations_from_tool_results(tool_results);
    let search_citations = build_search_citations_from_tool_results(tool_results);
    // Preserve search observation indices (1..N) for [[n]] markers — do not renumber.
    citations.extend(search_citations);
    citations
}

/// Filter citations to those explicitly referenced in the answer.
/// RAG: `[[cite:CHUNK_ID]]`; Search: `[[n]]`; no markers → empty (ADR-0008).
pub fn filter_citations_by_answer_references(
    answer: &str,
    citations: Vec<Citation>,
) -> Vec<Citation> {
    filter_citations_for_mode("rag", answer, citations)
}

pub fn filter_citations_for_mode(
    mode_id: &str,
    answer: &str,
    citations: Vec<Citation>,
) -> Vec<Citation> {
    if citations.is_empty() {
        return citations;
    }

    let filtered: Vec<Citation> = if mode_id == "search" {
        let indices = extract_search_citation_indices(answer);
        if indices.is_empty() {
            Vec::new()
        } else {
            citations
                .iter()
                .filter(|citation| {
                    citation.layer.as_deref() == Some("search")
                        && indices.contains(&citation.citation_id)
                })
                .cloned()
                .collect()
        }
    } else {
        let cited_chunk_ids = crate::rag_prompts::extract_referenced_chunk_ids(answer);
        if cited_chunk_ids.is_empty() {
            Vec::new()
        } else {
            citations
                .iter()
                .filter(|citation| {
                    citation
                        .chunk_id
                        .as_ref()
                        .is_some_and(|id| cited_chunk_ids.contains(id))
                })
                .cloned()
                .collect()
        }
    };

    if mode_id == "search" {
        return filtered;
    }

    filtered
        .into_iter()
        .enumerate()
        .map(|(index, mut citation)| {
            citation.citation_id = (index + 1) as i64;
            citation
        })
        .collect()
}

fn extract_search_citation_indices(answer: &str) -> std::collections::HashSet<i64> {
    crate::agents::r#loop::answer_contract::extract_search_indices(answer)
        .into_iter()
        .map(|index| index as i64)
        .collect()
}

pub fn degrade_trace_from_tool_results(tool_results: &[ToolResult]) -> Vec<DegradeTraceItem> {
    let mut trace = Vec::new();
    for result in tool_results {
        if let Some(tool_trace) = result.trace.as_ref()
            && let Some(reason) = tool_trace.degrade_reason.as_ref()
        {
            trace.push(DegradeTraceItem {
                stage: result.tool.clone(),
                reason: reason.clone(),
                impact: format!("{} degraded", result.tool),
            });
        }
        if result.status != ToolStatus::Ok {
            let reason = result
                .data
                .as_ref()
                .and_then(|data| data.get("error"))
                .and_then(|error| error.as_str())
                .unwrap_or("tool execution failed");
            trace.push(DegradeTraceItem {
                stage: result.tool.clone(),
                reason: reason.to_string(),
                impact: format!("{} unavailable", result.tool),
            });
        }
    }
    trace
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
            let doc_id = item
                .get("doc_id")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            let text = item
                .get("text")
                .and_then(|v| v.as_str())
                .map(|s| s.chars().take(200).collect::<String>());
            let page = item
                .get("page")
                .and_then(|v| v.as_i64())
                .map(|p| p as usize);

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
    use common::{Citation, ToolResult};

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
        let results = vec![tr(
            "t",
            ToolStatus::Ok,
            Some(serde_json::json!([{"chunk_id": "c1"}])),
        )];
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
            tr(
                "dense",
                ToolStatus::Ok,
                Some(serde_json::json!([
                    {"chunk_id": "c1", "text": "hello", "score": 0.9}
                ])),
            ),
            tr(
                "fail",
                ToolStatus::Error,
                Some(serde_json::json!([{"chunk_id": "c2"}])),
            ),
        ];
        let chunks = extract_chunks_with_scores(&results);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].0.chunk_id, "c1");
        assert_eq!(chunks[0].0.text, "hello");
        assert_eq!(chunks[0].1, 0.9);
    }

    #[test]
    fn test_extract_chunks_skips_missing_chunk_id() {
        let results = vec![tr(
            "t",
            ToolStatus::Ok,
            Some(serde_json::json!([{"text": "no id"}])),
        )];
        assert!(extract_chunks_with_scores(&results).is_empty());
    }

    #[test]
    fn test_build_citations_dedupes_by_chunk_id() {
        let results = vec![tr(
            "dense",
            ToolStatus::Ok,
            Some(serde_json::json!([
                {"chunk_id": "c1", "doc_id": "d1", "text": "text1", "score": 0.8},
                {"chunk_id": "c1", "doc_id": "d1", "text": "text1", "score": 0.8}
            ])),
        )];
        let citations = build_citations_from_tool_results(&results);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].citation_id, 1);
    }

    #[test]
    fn test_build_sources_dedupes_by_chunk_id() {
        let results = vec![tr(
            "t",
            ToolStatus::Ok,
            Some(serde_json::json!([
                {"chunk_id": "c1", "doc_id": "d1"},
                {"chunk_id": "c1", "doc_id": "d1"}
            ])),
        )];
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

    fn sample_citation(id: i64, chunk_id: &str) -> Citation {
        Citation {
            citation_id: id,
            doc_id: "doc-1".to_string(),
            chunk_id: Some(chunk_id.to_string()),
            page: None,
            doc_name: "doc-1".to_string(),
            preview: Some("preview".to_string()),
            content: Some("content".to_string()),
            score: 1.0,
            layer: Some("dense_retrieval".to_string()),
            chunk_type: Some("text".to_string()),
            asset_id: None,
            caption: None,
            image_url: None,
            parser_backend: None,
            source_locator: None,
            parse_run_id: None,
        }
    }

    #[test]
    fn filter_citations_keeps_only_referenced_chunk_ids() {
        let citations = vec![sample_citation(1, "chunk-a"), sample_citation(2, "chunk-b")];
        let filtered = filter_citations_by_answer_references("Answer [[cite:chunk-a]]", citations);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].citation_id, 1);
        assert_eq!(filtered[0].chunk_id.as_deref(), Some("chunk-a"));
    }

    #[test]
    fn filter_citations_returns_empty_when_answer_has_no_markers_and_no_evidence_layer() {
        let citations = vec![sample_citation(1, "chunk-a")];
        let filtered =
            filter_citations_for_mode("chat", "Answer without explicit cite markers", citations);
        assert!(filtered.is_empty());
    }

    #[test]
    fn filter_citations_strict_empty_when_rag_markers_missing() {
        let citations = vec![sample_citation(1, "chunk-a")];
        let filtered =
            filter_citations_for_mode("rag", "Answer without explicit cite markers", citations);
        assert!(filtered.is_empty());
    }

    fn sample_search_citation(id: i64, url: &str) -> Citation {
        Citation {
            citation_id: id,
            doc_id: url.to_string(),
            chunk_id: None,
            page: None,
            doc_name: "title".to_string(),
            preview: Some("snippet".to_string()),
            content: Some("snippet".to_string()),
            score: 1.0,
            layer: Some("search".to_string()),
            chunk_type: Some("web".to_string()),
            asset_id: None,
            caption: None,
            image_url: None,
            parser_backend: None,
            source_locator: None,
            parse_run_id: None,
        }
    }

    #[test]
    fn no_marker_returns_empty_citations() {
        let citations = vec![sample_search_citation(1, "https://a.example")];
        let filtered = filter_citations_for_mode("search", "Answer without markers", citations);
        assert!(filtered.is_empty());
    }

    #[test]
    fn search_filter_keeps_observation_index_two() {
        let citations = vec![
            sample_search_citation(1, "https://a.example"),
            sample_search_citation(2, "https://b.example"),
            sample_search_citation(3, "https://c.example"),
        ];
        let filtered = filter_citations_for_mode("search", "Answer [[2]] here", citations);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].citation_id, 2);
        assert_eq!(filtered[0].doc_id, "https://b.example");
    }

    #[test]
    fn search_filter_expands_combined_index_marker() {
        let citations = vec![
            sample_search_citation(1, "https://a.example"),
            sample_search_citation(2, "https://b.example"),
        ];
        let filtered = filter_citations_for_mode("search", "Refs [[1, 2]]", citations);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].doc_id, "https://a.example");
        assert_eq!(filtered[1].doc_id, "https://b.example");
    }

    #[test]
    fn build_all_preserves_search_citation_indices() {
        let results = vec![tr(
            "web_search",
            ToolStatus::Ok,
            Some(serde_json::json!({
                "query_type": "web",
                "sub_queries": ["q"],
                "synthesized_answer": "",
                "results": [
                    {"url": "https://a.example", "title": "A", "snippet": "a", "citation_index": 1},
                    {"url": "https://b.example", "title": "B", "snippet": "b", "citation_index": 2}
                ]
            })),
        )];
        let citations = build_all_citations_from_tool_results(&results);
        let search: Vec<_> = citations
            .iter()
            .filter(|c| c.layer.as_deref() == Some("search"))
            .collect();
        assert_eq!(search.len(), 2);
        assert_eq!(search[0].citation_id, 1);
        assert_eq!(search[1].citation_id, 2);
    }

    #[test]
    fn mixed_rag_and_search_keeps_search_observation_indices() {
        let results = vec![
            tr(
                "dense_retrieval",
                ToolStatus::Ok,
                Some(serde_json::json!([
                    {"chunk_id": "c1", "doc_id": "d1", "text": "t", "score": 0.9}
                ])),
            ),
            tr(
                "web_search",
                ToolStatus::Ok,
                Some(serde_json::json!({
                    "query_type": "web",
                    "sub_queries": ["q"],
                    "synthesized_answer": "",
                    "results": [
                        {"url": "https://a.example", "title": "A", "snippet": "a", "citation_index": 1}
                    ]
                })),
            ),
        ];
        let citations = build_all_citations_from_tool_results(&results);
        let search = citations
            .iter()
            .find(|c| c.layer.as_deref() == Some("search"))
            .expect("search citation");
        assert_eq!(search.citation_id, 1);
        let filtered = filter_citations_for_mode("search", "Web [[1]]", citations);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].doc_id, "https://a.example");
    }
}

pub async fn dispatch_tools_with_history_interception(
    calls: Vec<common::ToolCall>,
    auth: &avrag_auth::AuthContext,
    session_id: Option<uuid::Uuid>,
    repository: Option<&avrag_storage_pg::PgAppRepository>,
    search_provider: Option<&dyn avrag_search::SearchProvider>,
) -> Vec<ToolResult> {
    let mut out = Vec::new();
    let mut normal_calls = Vec::new();
    let mut indices = Vec::new();

    for (idx, call) in calls.into_iter().enumerate() {
        if call.tool == "conversation_history_load" && session_id.is_some() && repository.is_some()
        {
            let session_id = session_id.unwrap();
            let repo = repository.unwrap();

            // Parse arguments
            let tags: Option<Vec<String>> =
                call.args.get("tags").and_then(|v| v.as_array()).map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                });
            let limit = call
                .args
                .get("limit")
                .and_then(|v| v.as_i64())
                .unwrap_or(20);

            let res = repo
                .load_history_by_tags(auth, session_id, tags.clone(), limit)
                .await;
            let result = match res {
                Ok(messages) => {
                    let msg_json: Vec<serde_json::Value> = messages
                        .into_iter()
                        .map(|m| {
                            serde_json::json!({
                                "message_id": m.message_id,
                                "role": m.role,
                                "content": m.content,
                                "tags": m.tags,
                                "created_at": m.created_at.to_rfc3339(),
                            })
                        })
                        .collect();
                    ToolResult {
                        tool: call.tool.clone(),
                        version: call.version.clone(),
                        status: ToolStatus::Ok,
                        data: Some(serde_json::json!({
                            "tags": tags,
                            "limit": limit,
                            "message_count": msg_json.len(),
                            "messages": msg_json,
                        })),
                        trace: None,
                    }
                }
                Err(e) => ToolResult {
                    tool: call.tool.clone(),
                    version: call.version.clone(),
                    status: ToolStatus::Error,
                    data: Some(serde_json::json!({ "error": e.to_string() })),
                    trace: None,
                },
            };
            out.push((idx, result));
        } else if call.tool == "conversation_history_tag" && repository.is_some() {
            let repo = repository.unwrap();

            // Parse arguments
            let ops_val = call.args.get("operations").and_then(|v| v.as_array());
            let result = match ops_val {
                Some(arr) => {
                    let mut tag_ops = Vec::new();
                    let mut valid = true;
                    for op_val in arr {
                        let msg_id = op_val.get("message_id").and_then(|v| v.as_i64());
                        let action = op_val.get("action").and_then(|v| v.as_str());
                        let tags: Option<Vec<String>> =
                            op_val.get("tags").and_then(|v| v.as_array()).map(|a| {
                                a.iter()
                                    .filter_map(|x| x.as_str().map(String::from))
                                    .collect()
                            });

                        if let (Some(mid), Some(act), Some(t)) = (msg_id, action, tags) {
                            match act {
                                "add" => {
                                    for tag in t {
                                        tag_ops.push(avrag_storage_pg::TagOperation::AddTag {
                                            message_id: mid,
                                            tag,
                                        });
                                    }
                                }
                                "remove" => {
                                    for tag in t {
                                        tag_ops.push(avrag_storage_pg::TagOperation::RemoveTag {
                                            message_id: mid,
                                            tag,
                                        });
                                    }
                                }
                                "replace" => {
                                    tag_ops.push(avrag_storage_pg::TagOperation::ReplaceTags {
                                        message_id: mid,
                                        tags: t,
                                    });
                                }
                                _ => {
                                    valid = false;
                                    break;
                                }
                            }
                        } else {
                            valid = false;
                            break;
                        }
                    }

                    if valid {
                        match repo.apply_tag_operations(auth, tag_ops).await {
                            Ok(()) => ToolResult {
                                tool: call.tool.clone(),
                                version: call.version.clone(),
                                status: ToolStatus::Ok,
                                data: Some(serde_json::json!({
                                    "operation_count": arr.len(),
                                })),
                                trace: None,
                            },
                            Err(e) => ToolResult {
                                tool: call.tool.clone(),
                                version: call.version.clone(),
                                status: ToolStatus::Error,
                                data: Some(serde_json::json!({ "error": e.to_string() })),
                                trace: None,
                            },
                        }
                    } else {
                        ToolResult {
                            tool: call.tool.clone(),
                            version: call.version.clone(),
                            status: ToolStatus::Error,
                            data: Some(serde_json::json!({ "error": "Invalid operations schema" })),
                            trace: None,
                        }
                    }
                }
                None => ToolResult {
                    tool: call.tool.clone(),
                    version: call.version.clone(),
                    status: ToolStatus::Error,
                    data: Some(serde_json::json!({ "error": "Missing operations argument" })),
                    trace: None,
                },
            };
            out.push((idx, result));
        } else {
            normal_calls.push(call);
            indices.push(idx);
        }
    }

    if !normal_calls.is_empty() {
        let normal_results =
            crate::agents::unified::atomic_tools::dispatch_atomic_tools_with_enforcement(
                normal_calls,
                search_provider,
                Some(auth),
            )
            .await;

        for (idx, res) in indices.into_iter().zip(normal_results.into_iter()) {
            out.push((idx, res));
        }
    }

    out.sort_by_key(|(idx, _)| *idx);
    out.into_iter().map(|(_, res)| res).collect()
}
