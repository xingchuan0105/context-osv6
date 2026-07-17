//! Channel workers: map retrieval runs → [`EvidencePack`].

use contracts::{ToolResult, ToolStatus};
use uuid::Uuid;

use super::types::{
    Channel, EvidenceItem, EvidencePack, PackStatus, TaskBrief,
};
use agent_loop::runtime::AgentRunResult;

const MAX_ITEMS: usize = 12;
const MAX_TEXT_CHARS: usize = 1200;

/// Build an EvidencePack from a finished worker run (any status).
pub fn pack_from_run(
    channel: Channel,
    brief: TaskBrief,
    result: &AgentRunResult,
    run_error: Option<String>,
) -> EvidencePack {
    let dispatch_id = Uuid::new_v4().to_string();
    if let Some(err) = run_error {
        return EvidencePack {
            channel,
            status: PackStatus::Error,
            dispatch_id,
            task_brief: brief,
            items: vec![],
            notes: truncate_notes(&result.answer),
            error: Some(err),
        };
    }

    let mut items = extract_items(channel, &result.tool_results);
    if items.is_empty() {
        // Fallback: scrape cite markers from answer as weak ids (optional)
        items.extend(items_from_answer_cites(&result.answer));
    }
    items.truncate(MAX_ITEMS);
    for it in &mut items {
        if it.text.chars().count() > MAX_TEXT_CHARS {
            it.text = it
                .text
                .chars()
                .take(MAX_TEXT_CHARS)
                .collect::<String>()
                + "…";
        }
    }

    let status = if items.is_empty() {
        PackStatus::Empty
    } else {
        PackStatus::Ok
    };

    EvidencePack {
        channel,
        status,
        dispatch_id,
        task_brief: brief,
        items,
        notes: truncate_notes(&result.answer),
        error: None,
    }
}

fn truncate_notes(answer: &str) -> Option<String> {
    let t = answer.trim();
    if t.is_empty() {
        return None;
    }
    let s: String = t.chars().take(800).collect();
    Some(s)
}

fn extract_items(channel: Channel, tools: &[ToolResult]) -> Vec<EvidenceItem> {
    let mut items = Vec::new();
    for tr in tools {
        if tr.status != ToolStatus::Ok {
            continue;
        }
        let Some(data) = tr.data.as_ref() else {
            continue;
        };
        match channel {
            Channel::Rag => push_rag_items(&mut items, data),
            Channel::Search => push_search_items(&mut items, data),
        }
    }
    items
}

fn push_rag_items(items: &mut Vec<EvidenceItem>, data: &serde_json::Value) {
    // Common shapes: { chunks: [...] }, { results: [...] }, array of chunks
    let arrays = [
        data.get("chunks"),
        data.get("results"),
        data.get("items"),
        data.as_array().map(|_| data),
    ];
    for arr in arrays.into_iter().flatten() {
        if let Some(list) = arr.as_array() {
            for c in list {
                let id = c
                    .get("chunk_id")
                    .or_else(|| c.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if id.is_empty() {
                    continue;
                }
                let text = c
                    .get("text")
                    .or_else(|| c.get("content"))
                    .or_else(|| c.get("snippet"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let score = c.get("score").and_then(|v| v.as_f64());
                items.push(EvidenceItem {
                    id,
                    title: c
                        .get("title")
                        .or_else(|| c.get("filename"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    text,
                    score,
                    uri: None,
                });
            }
        }
    }
}

fn push_search_items(items: &mut Vec<EvidenceItem>, data: &serde_json::Value) {
    let list = data
        .get("results")
        .or_else(|| data.get("items"))
        .and_then(|v| v.as_array());
    let Some(list) = list else {
        return;
    };
    for (i, r) in list.iter().enumerate() {
        let url = r
            .get("url")
            .or_else(|| r.get("link"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let title = r
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let text = r
            .get("snippet")
            .or_else(|| r.get("content"))
            .or_else(|| r.get("text"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let id = if url.is_empty() {
            format!("{}", i + 1)
        } else {
            url.clone()
        };
        items.push(EvidenceItem {
            id,
            title,
            text,
            score: None,
            uri: if url.is_empty() { None } else { Some(url) },
        });
    }
}

fn items_from_answer_cites(answer: &str) -> Vec<EvidenceItem> {
    // [[cite:uuid]] weak fallback — text empty (chat may still use notes)
    let mut out = Vec::new();
    let mut rest = answer;
    while let Some(start) = rest.find("[[cite:") {
        let after = &rest[start + 7..];
        if let Some(end) = after.find("]]") {
            let id = after[..end].trim().to_string();
            if !id.is_empty() {
                out.push(EvidenceItem {
                    id,
                    title: None,
                    text: String::new(),
                    score: None,
                    uri: None,
                });
            }
            rest = &after[end + 2..];
        } else {
            break;
        }
    }
    out
}

/// Error pack when worker cannot start.
pub fn pack_error(channel: Channel, brief: TaskBrief, error: impl Into<String>) -> EvidencePack {
    EvidencePack {
        channel,
        status: PackStatus::Error,
        dispatch_id: Uuid::new_v4().to_string(),
        task_brief: brief,
        items: vec![],
        notes: None,
        error: Some(error.into()),
    }
}

/// Attach worker evidence to the chat-exit answer (Option B citation path).
///
/// The chat exit runs in pure-chat mode (no retrieval tools), so its own run
/// carries no retrieval evidence. Rebuild citations/sources from the merged
/// worker tool results — filtered to the markers the answer actually emitted
/// (`[[cite:chunk_id]]` / `[[web:n]]`), same as the legacy assembled path
/// (`filter_citations_for_mode` with `"rag"` keeps both doc cites and web
/// indices; there are no doc citations for search-only turns anyway).
pub fn attach_worker_evidence(
    answer_result: &mut AgentRunResult,
    worker_tool_results: Vec<ToolResult>,
) {
    if worker_tool_results.is_empty() {
        return;
    }
    let mut merged = worker_tool_results;
    merged.extend(answer_result.tool_results.iter().cloned());
    let citations = agent_loop::helpers::build_all_citations_from_tool_results(&merged);
    let filtered =
        agent_loop::helpers::filter_citations_for_mode("rag", &answer_result.answer, citations);
    // Multi-dispatch turns (recovery / re-dispatch) merge repeated worker runs:
    // drop duplicate citations of the same evidence (web by url, docs by chunk_id).
    let mut seen = std::collections::HashSet::new();
    answer_result.citations = filtered
        .into_iter()
        .filter(|c| {
            let key = c
                .chunk_id
                .clone()
                .unwrap_or_else(|| format!("web:{}", c.doc_id));
            seen.insert(key)
        })
        .collect();
    answer_result.sources = agent_loop::helpers::build_sources_from_tool_results(&merged);
    answer_result.tool_results = merged;
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::ToolStatus;

    #[test]
    fn extracts_search_results() {
        let tr = ToolResult {
            tool: "web_search".into(),
            version: "1".into(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!({
                "results": [
                    {"url": "https://ex.com", "title": "T", "snippet": "hello"}
                ]
            })),
            trace: None,
        };
        let mut result = AgentRunResult::default();
        result.tool_results = vec![tr];
        let pack = pack_from_run(Channel::Search, TaskBrief::new("g"), &result, None);
        assert_eq!(pack.status, PackStatus::Ok);
        assert_eq!(pack.items.len(), 1);
        assert_eq!(pack.items[0].uri.as_deref(), Some("https://ex.com"));
    }

    #[test]
    fn empty_tools_empty_pack() {
        let result = AgentRunResult::default();
        let pack = pack_from_run(Channel::Rag, TaskBrief::new("g"), &result, None);
        assert_eq!(pack.status, PackStatus::Empty);
    }

    #[test]
    fn run_error_status() {
        let result = AgentRunResult::default();
        let pack = pack_from_run(
            Channel::Rag,
            TaskBrief::new("g"),
            &result,
            Some("boom".into()),
        );
        assert_eq!(pack.status, PackStatus::Error);
        assert_eq!(pack.error.as_deref(), Some("boom"));
    }

    #[test]
    fn attach_evidence_builds_filtered_citations_and_sources() {
        let worker_results = vec![
            ToolResult {
                tool: "dense_retrieval".into(),
                version: "1".into(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!([
                    {"chunk_id": "chunk-a", "doc_id": "doc1", "text": "doc evidence", "score": 0.9}
                ])),
                trace: None,
            },
            ToolResult {
                tool: "web_search".into(),
                version: "1".into(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!({
                    "query_type": "web",
                    "sub_queries": ["q"],
                    "results": [
                        {"url": "https://a.example", "title": "A", "snippet": "web evidence", "citation_index": 1}
                    ],
                    "synthesized_answer": ""
                })),
                trace: None,
            },
        ];
        let mut chat_run = AgentRunResult::default();
        chat_run.answer = "Doc says so [[cite:chunk-a]], web agrees [[web:1]].".into();

        attach_worker_evidence(&mut chat_run, worker_results);

        assert_eq!(chat_run.citations.len(), 2);
        assert!(
            chat_run
                .citations
                .iter()
                .any(|c| c.chunk_id.as_deref() == Some("chunk-a"))
        );
        assert!(
            chat_run
                .citations
                .iter()
                .any(|c| c.layer.as_deref() == Some("search") && c.doc_id == "https://a.example")
        );
        assert!(!chat_run.sources.is_empty());
        assert_eq!(chat_run.tool_results.len(), 2);
    }

    #[test]
    fn attach_evidence_drops_unreferenced_citations() {
        let worker_results = vec![ToolResult {
            tool: "dense_retrieval".into(),
            version: "1".into(),
            status: ToolStatus::Ok,
            data: Some(serde_json::json!([
                {"chunk_id": "chunk-a", "doc_id": "doc1", "text": "t", "score": 0.9}
            ])),
            trace: None,
        }];
        let mut chat_run = AgentRunResult::default();
        chat_run.answer = "Answer with no markers.".into();

        attach_worker_evidence(&mut chat_run, worker_results);

        assert!(chat_run.citations.is_empty());
    }
}
