use contracts::AnswerContextChunk;
use contracts::chat::SourceRef;
use contracts::{ToolResult, ToolStatus};

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
            let text = chunk_text_field(item).unwrap_or_default();
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

pub(crate) fn chunk_text_field(item: &serde_json::Value) -> Option<String> {
    item.get("text")
        .or_else(|| item.get("content"))
        .and_then(|v| v.as_str())
        .map(str::to_owned)
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
            let text = chunk_text_field(item).map(|s| s.chars().take(200).collect::<String>());
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
}
