//! Channel workers: worker digests + post-run evidence finalization.
//!
//! `finalize_answer_evidence` is the single point where the chat exit's
//! `[[E:id]]` markers become product output: valid E-ids are rewritten to
//! product markers (`[[cite:chunk_id]]` / `[[web:n]]`) and mapped 1:1 to
//! `contracts::Citation` from the store; dangling or off-protocol markers
//! (`[[E99]]`, raw `[[web:1]]`…) are stripped with a warning — an empty
//! channel can never fabricate citations (2026-07-17 incident). Markers
//! pointing at targeted (DocProfile, orientation-only) entries are stripped
//! silently.

use agent_loop::runtime::AgentRunResult;
use contracts::chat::{AnswerBlock, Citation, SourceRef};

use super::store::{EvidenceKind, EvidenceStore};

const MAX_NOTE_CHARS: usize = 2000;

/// Non-Ok tool outcomes from a worker run, as short descriptions
/// (`web_search: Timeout (detail)`). Used to distinguish "检索失败" (Error)
/// from "未命中" (Empty) instead of silently collapsing both to Empty.
pub fn tool_failures(results: &[contracts::ToolResult]) -> Vec<String> {
    results
        .iter()
        .filter(|tr| tr.status != contracts::ToolStatus::Ok)
        .map(|tr| {
            let detail = tr
                .data
                .as_ref()
                .and_then(|d| d.get("error").and_then(|e| e.as_str()))
                .map(|s| format!(" ({s})"))
                .unwrap_or_default();
            format!("{}: {:?}{detail}", tr.tool, tr.status)
        })
        .collect()
}

/// Worker channel summary handed to the chat exit (its digested
/// understanding of the channel — not raw chunks).
pub fn channel_note_from_run(result: &AgentRunResult) -> Option<String> {
    let t = result.answer.trim();
    if t.is_empty() {
        return None;
    }
    Some(t.chars().take(MAX_NOTE_CHARS).collect())
}

/// Rewrite E-markers to product markers and rebuild citations/sources.
pub fn finalize_answer_evidence(answer_result: &mut AgentRunResult, store: &EvidenceStore) {
    let original_answer = answer_result.answer.clone();
    let (rewritten, citations, stripped) = rewrite_markers(&original_answer, store);
    if stripped > 0 {
        tracing::warn!(
            stripped,
            "orchestrator chat exit emitted dangling/off-protocol citation markers"
        );
    }
    answer_result.answer = rewritten.clone();
    for block in &mut answer_result.answer_blocks {
        if let AnswerBlock::Text { text, .. } = block {
            if *text == original_answer {
                *text = rewritten.clone();
            } else if text.contains("[[") {
                // Divergent block text: rewrite standalone (numbering recomputed).
                let (t, _, _) = rewrite_markers(text, store);
                *text = t;
            }
        }
    }
    answer_result.citations = citations;
    answer_result.sources = store
        .entries()
        .iter()
        .filter(|e| e.kind == EvidenceKind::WebPage)
        .map(|e| SourceRef {
            id: e.url.clone().unwrap_or_else(|| e.eid.clone()),
            title: e.title.clone().unwrap_or_default(),
            snippet: Some(e.preview.clone()),
            doc_id: None,
            page: None,
        })
        .collect();
}

/// Scan `[[...]]` tokens; returns (rewritten text, citations, stripped count).
fn rewrite_markers(
    answer: &str,
    store: &EvidenceStore,
) -> (String, Vec<Citation>, usize) {
    let mut out = String::with_capacity(answer.len());
    let mut citations: Vec<Citation> = Vec::new();
    let mut seen_eids = std::collections::HashSet::new();
    let mut stripped = 0usize;
    // Single global appearance-order counter: citation_id must be unique
    // across doc AND web citations — the lookup endpoint finds by citation_id
    // alone, and the frontend resolves `[[web:n]]` by array position (which
    // equals this counter). Two per-kind counters collided (2026-07-18).
    let mut next_index: i64 = 1;

    let mut rest = answer;
    while let Some(start) = rest.find("[[") {
        let (head, tail) = rest.split_at(start);
        out.push_str(head);
        let Some(end) = tail.find("]]") else {
            out.push_str(tail);
            rest = "";
            break;
        };
        let token = tail[2..end].trim();

        if let Some(eid) = parse_e_marker(token) {
            match store.get(&eid) {
                Some(entry) if entry.kind == EvidenceKind::DocProfile => {
                    // Targeted (orientation) entry: not citable — strip silently
                    // (the model was told not to cite it; no warning needed).
                }
                Some(entry) => {
                    if seen_eids.insert(eid.clone()) {
                        match entry.kind {
                            EvidenceKind::DocChunk => {
                                let chunk = entry.chunk_id.clone().unwrap_or_default();
                                out.push_str(&format!("[[cite:{chunk}]]"));
                                citations.push(Citation {
                                    citation_id: next_index,
                                    doc_id: entry.doc_id.clone().unwrap_or_default(),
                                    chunk_id: Some(chunk),
                                    page: entry.page,
                                    doc_name: entry
                                        .doc_name
                                        .clone()
                                        .or_else(|| entry.doc_id.clone())
                                        .unwrap_or_default(),
                                    preview: Some(entry.preview.clone()),
                                    content: Some(entry.full_text.clone()),
                                    score: entry.score.unwrap_or(0.0) as f32,
                                    layer: Some("dense_retrieval".to_string()),
                                    chunk_type: Some("text".to_string()),
                                    asset_id: None,
                                    caption: None,
                                    image_url: None,
                                    parser_backend: None,
                                    source_locator: None,
                                    parse_run_id: None,
                                });
                                next_index += 1;
                            }
                            EvidenceKind::WebPage => {
                                let url = entry.url.clone().unwrap_or_default();
                                out.push_str(&format!("[[web:{next_index}]]"));
                                citations.push(Citation {
                                    citation_id: next_index,
                                    doc_id: url.clone(),
                                    chunk_id: None,
                                    page: None,
                                    doc_name: entry.title.clone().unwrap_or_default(),
                                    preview: Some(entry.preview.clone()),
                                    content: Some(entry.full_text.clone()),
                                    score: 1.0,
                                    layer: Some("search".to_string()),
                                    chunk_type: Some("web".to_string()),
                                    asset_id: None,
                                    caption: None,
                                    image_url: None,
                                    parser_backend: None,
                                    source_locator: Some(serde_json::json!({
                                        "url": url,
                                        "title": entry.title.clone().unwrap_or_default(),
                                    })),
                                    parse_run_id: None,
                                });
                                next_index += 1;
                            }
                            EvidenceKind::DocProfile => {
                                // Unreachable: handled by the match guard above.
                            }
                        }
                    } else {
                        // Repeat citation of the same entry: reuse its product marker.
                        let existing = citations
                            .iter()
                            .find(|c| marker_source_matches(c, &eid, store));
                        match existing {
                            Some(c) if c.chunk_id.is_some() => {
                                out.push_str(&format!(
                                    "[[cite:{}]]",
                                    c.chunk_id.as_deref().unwrap_or_default()
                                ));
                            }
                            Some(c) => {
                                out.push_str(&format!("[[web:{}]]", c.citation_id));
                            }
                            None => {}
                        }
                    }
                }
                None => stripped += 1, // dangling E-id: drop the marker
            }
        } else if is_raw_citation_marker(token) {
            // Off-protocol [[cite:…]] / [[web:n]] / [[n]]: ungrounded here → drop.
            stripped += 1;
        } else {
            // Not a citation token (e.g. [[image:…]]) — pass through untouched.
            out.push_str(&rest_after_token_prefix(tail));
        }
        rest = &tail[end + 2..];
    }
    out.push_str(rest);
    (out, citations, stripped)
}

/// `E7` / `E:7` → "E7" (store id form).
fn parse_e_marker(token: &str) -> Option<String> {
    let t = token.strip_prefix('E').or_else(|| token.strip_prefix('e'))?;
    let t = t.strip_prefix(':').unwrap_or(t);
    if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) {
        Some(format!("E{t}"))
    } else {
        None
    }
}

fn is_raw_citation_marker(token: &str) -> bool {
    if token.starts_with("cite:") {
        return true;
    }
    let t = token.strip_prefix("web:").unwrap_or(token);
    !t.is_empty()
        && t.split(',')
            .all(|p| p.trim().chars().all(|c| c.is_ascii_digit()) && !p.trim().is_empty())
}

/// Whether an existing citation came from this store entry (for repeat refs).
fn marker_source_matches(
    citation: &Citation,
    eid: &str,
    store: &EvidenceStore,
) -> bool {
    let Some(entry) = store.get(eid) else {
        return false;
    };
    match entry.kind {
        EvidenceKind::DocChunk => citation.chunk_id == entry.chunk_id,
        EvidenceKind::WebPage => citation.doc_id == entry.url.clone().unwrap_or_default(),
        EvidenceKind::DocProfile => false,
    }
}

/// Re-emit the untouched `[[token]]` text (helper for pass-through).
fn rest_after_token_prefix(tail: &str) -> String {
    let end = tail.find("]]").map(|i| i + 2).unwrap_or(tail.len());
    tail[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::store::EvidenceStore;
    use crate::orchestrator::types::Channel;
    use contracts::{ToolResult, ToolStatus};

    fn store_with_both() -> EvidenceStore {
        let mut store = EvidenceStore::default();
        store.insert_from_tool_results(
            Channel::Rag,
            &[ToolResult {
                tool: "dense_retrieval".into(),
                version: "1".into(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!([
                    {"chunk_id": "chunk-a", "doc_id": "d1", "text": "doc evidence", "score": 0.9, "page": 3}
                ])),
                trace: None,
            }],
        );
        store.insert_from_tool_results(
            Channel::Search,
            &[ToolResult {
                tool: "web_search".into(),
                version: "1".into(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!({
                    "results": [{"url": "https://a.example", "title": "A", "snippet": "web evidence"}]
                })),
                trace: None,
            }],
        );
        store
    }

    #[test]
    fn valid_markers_become_product_markers_and_citations() {
        let store = store_with_both();
        let mut r = AgentRunResult::default();
        r.answer = "文档证据 [[E1]]，网页佐证 [[E2]]。重复 [[E1]]".into();
        finalize_answer_evidence(&mut r, &store);

        assert!(r.answer.contains("[[cite:chunk-a]]"), "{}", r.answer);
        // Single global counter: doc first (1), web second (2).
        assert!(r.answer.contains("[[web:2]]"), "{}", r.answer);
        assert!(!r.answer.contains("[[E"), "E-ids must be gone: {}", r.answer);
        assert_eq!(r.citations.len(), 2, "repeat ref dedupes");
        assert_eq!(r.citations[0].chunk_id.as_deref(), Some("chunk-a"));
        assert_eq!(r.citations[0].page, Some(3));
        assert_eq!(r.citations[1].layer.as_deref(), Some("search"));
        assert_eq!(r.citations[1].doc_id, "https://a.example");
        assert_eq!(r.sources.len(), 1);
    }

    #[test]
    fn citation_ids_are_unique_across_doc_and_web() {
        // 2026-07-18 incident: per-kind counters collided — lookup by
        // citation_id returned the wrong entry (web before doc → 404).
        let store = store_with_both();
        let mut r = AgentRunResult::default();
        r.answer = "网页 [[E2]] 在前，文档 [[E1]] 在后。".into();
        finalize_answer_evidence(&mut r, &store);

        let ids: Vec<i64> = r.citations.iter().map(|c| c.citation_id).collect();
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(ids.len(), unique.len(), "citation ids must be unique: {ids:?}");
        // `[[web:n]]` n == citation_id == array position (frontend resolves by index).
        assert_eq!(r.citations[0].citation_id, 1);
        assert_eq!(r.citations[0].layer.as_deref(), Some("search"));
        assert!(r.answer.contains("[[web:1]]"), "{}", r.answer);
        assert_eq!(r.citations[1].citation_id, 2);
        assert_eq!(r.citations[1].chunk_id.as_deref(), Some("chunk-a"));
    }

    #[test]
    fn dangling_and_raw_markers_are_stripped() {
        let store = store_with_both();
        let mut r = AgentRunResult::default();
        r.answer = "编造的 [[E9]] 和原生 [[web:7]] 与 [[cite:fake]] 都剥离。".into();
        finalize_answer_evidence(&mut r, &store);

        assert!(!r.answer.contains("[["), "{}", r.answer);
        assert!(r.citations.is_empty());
    }

    #[test]
    fn targeted_entry_markers_stripped_silently() {
        let mut store = store_with_both();
        store.insert_from_tool_results(
            Channel::Rag,
            &[ToolResult {
                tool: "doc_profile".into(),
                version: "1".into(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!([
                    {"doc_id": "d1", "genre": "report", "sections": [{"title": "t", "page": 1}]}
                ])),
                trace: None,
            }],
        );
        // E3 = targeted entry; citing it must vanish without a citation.
        let mut r = AgentRunResult::default();
        r.answer = "结构 [[E3]] 证据 [[E1]]".into();
        finalize_answer_evidence(&mut r, &store);

        assert!(!r.answer.contains("[[E3]]"), "{}", r.answer);
        assert!(r.answer.contains("[[cite:chunk-a]]"), "{}", r.answer);
        assert_eq!(r.citations.len(), 1);
        assert!(r.citations.iter().all(|c| c.chunk_id.is_some()));
    }

    #[test]
    fn answer_blocks_text_rewritten() {
        let store = store_with_both();
        let mut r = AgentRunResult::default();
        r.answer = "证据 [[E1]]".into();
        r.answer_blocks = vec![AnswerBlock::Text {
            text: "证据 [[E1]]".into(),
            citations: vec![],
        }];
        finalize_answer_evidence(&mut r, &store);
        let AnswerBlock::Text { text, .. } = &r.answer_blocks[0] else {
            panic!("text block");
        };
        assert!(text.contains("[[cite:chunk-a]]"));
    }
}
