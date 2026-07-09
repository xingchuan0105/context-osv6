use contracts::chat::{Citation, DegradeReason, DegradeTraceItem};
use contracts::{ToolResult, ToolStatus};

use super::retrieval::chunk_text_field;

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
            let text = chunk_text_field(item).unwrap_or_default();
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
                reason: parse_tool_degrade_reason(reason),
                impact: format!("{} degraded", result.tool),
            });
        }
        if result.status != ToolStatus::Ok {
            trace.push(DegradeTraceItem {
                stage: result.tool.clone(),
                reason: DegradeReason::ToolUnavailable,
                impact: format!("{} unavailable", result.tool),
            });
        }
    }
    trace
}

fn parse_tool_degrade_reason(raw: &str) -> DegradeReason {
    for part in raw
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let parsed = DegradeReason::from_str(part);
        if part == parsed.as_str() {
            return parsed;
        }
    }
    DegradeReason::from_str(raw.trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::ToolResult;

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
