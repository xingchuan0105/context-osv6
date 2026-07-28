//! K2 (2026-07-28, evidence-plane design §4): the retrieval log — the worker
//! registers chunk ALIASES (`SELECTED: #2, #5, #9`) and code hydrates the
//! full evidence. No JSON schema, no hand-copied chunk ids (those die with
//! E103 in K3; this slice only adds the new path).
//!
//! Alias reconstruction: the sandbox bridge assigns `#1 #2 …` per worker run,
//! incrementing across rounds in call order (bridge.rs `alias_counter`). The
//! run's recorded `tool_results` preserve that exact order, so the alias →
//! chunk map is REPLAYED here deterministically — no new structure crossing
//! the agent-loop / app-chat boundary.

use contracts::ToolResult;

/// One hydrated evidence unit: full chunk text + provenance header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HydratedChunk {
    pub chunk_id: String,
    pub doc_id: Option<String>,
    pub page: Option<i64>,
    pub text: String,
}

/// Parse `SELECTED:` / `SELECTED：` / `选择:` / `选择：` lines into alias
/// numbers (dedupe, order preserved). Only `#n` tokens count — chunk-id
/// shapes, prose, and bare numbers are ignored (non-contract, best-effort).
pub fn parse_selected_aliases(text: &str) -> Vec<u64> {
    let mut out: Vec<u64> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let Some(rest) = selected_line_body(trimmed) else {
            continue;
        };
        for token in rest.split(|c: char| !(c.is_ascii_digit() || c == '#')) {
            let Some(digits) = token.strip_prefix('#') else {
                continue;
            };
            if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            if let Ok(n) = digits.parse::<u64>() {
                if !out.contains(&n) {
                    out.push(n);
                }
            }
        }
    }
    out
}

/// `SELECTED: …` / `SELECTED：…` / `选择: …` / `选择：…` → body after the colon.
fn selected_line_body(line: &str) -> Option<&str> {
    for prefix in ["SELECTED", "选择"] {
        if let Some(rest) = line.strip_prefix(prefix) {
            let rest = rest.trim_start();
            let rest = rest.strip_prefix(':').or_else(|| rest.strip_prefix('：'))?;
            return Some(rest);
        }
    }
    None
}

/// Replay the run's tool results in order, enumerating chunk items exactly
/// as the bridge emitted them — position i in the returned Vec IS alias
/// `#{i+1}`. Only sandbox-bridged retrieval tools participate (native
/// web_search/web_fetch results are outside the alias namespace).
pub fn alias_chunks_in_order(tool_results: &[ToolResult]) -> Vec<HydratedChunk> {
    const ALIASED_TOOLS: &[&str] = &[
        "dense_retrieval",
        "lexical_retrieval",
        "graph_retrieval",
        "index_lookup",
        "doc_scan",
    ];
    let mut out = Vec::new();
    for tr in tool_results {
        if tr.status != contracts::ToolStatus::Ok || !ALIASED_TOOLS.contains(&tr.tool.as_str()) {
            continue;
        }
        let Some(data) = tr.data.as_ref() else {
            continue;
        };
        let list = data
            .as_array()
            .or_else(|| data.get("chunks").and_then(|v| v.as_array()));
        let Some(list) = list else {
            continue;
        };
        for item in list {
            let Some(chunk_id) = item.get("chunk_id").and_then(|v| v.as_str()) else {
                continue;
            };
            if chunk_id.is_empty() {
                continue;
            }
            let text = item
                .get("text")
                .or_else(|| item.get("content"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            out.push(HydratedChunk {
                chunk_id: chunk_id.to_string(),
                doc_id: item
                    .get("doc_id")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                page: item.get("page").and_then(|v| v.as_i64()),
                text,
            });
        }
    }
    out
}

/// Hydrate a worker's SELECTED log against its run's tool results: aliases
/// resolve to chunks in order, deduped; unresolvable aliases are skipped
/// (logged — best-effort, never fatal).
pub fn hydrate_selected(final_message: &str, tool_results: &[ToolResult]) -> Vec<HydratedChunk> {
    let aliases = parse_selected_aliases(final_message);
    if aliases.is_empty() {
        return Vec::new();
    }
    let stream = alias_chunks_in_order(tool_results);
    let mut out: Vec<HydratedChunk> = Vec::new();
    for alias in aliases {
        let Some(chunk) = (alias as usize)
            .checked_sub(1)
            .and_then(|idx| stream.get(idx))
        else {
            tracing::warn!(alias, "SELECTED alias did not resolve to a retrieved chunk");
            continue;
        };
        if out.iter().any(|c| c.chunk_id == chunk.chunk_id) {
            continue;
        }
        out.push(chunk.clone());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::ToolStatus;

    fn chunk_result(tool: &str, chunks: serde_json::Value) -> ToolResult {
        ToolResult {
            tool: tool.to_string(),
            version: "1.0".to_string(),
            status: ToolStatus::Ok,
            data: Some(chunks),
            trace: None,
        }
    }

    fn three_chunks() -> Vec<ToolResult> {
        vec![
            chunk_result(
                "dense_retrieval",
                serde_json::json!({"chunks": [
                    {"chunk_id": "c1", "doc_id": "d1", "text": "第一条证据", "page": 1},
                    {"chunk_id": "c2", "doc_id": "d1", "text": "第二条证据", "page": 2},
                ]}),
            ),
            chunk_result(
                "lexical_retrieval",
                serde_json::json!([{"chunk_id": "c3", "doc_id": "d2", "content": "第三条证据"}]),
            ),
        ]
    }

    #[test]
    fn parse_variants_fullwidth_spaces_and_prose() {
        assert_eq!(parse_selected_aliases("SELECTED: #2, #5, #9"), vec![2, 5, 9]);
        assert_eq!(parse_selected_aliases("SELECTED：#3"), vec![3]);
        assert_eq!(parse_selected_aliases("选择: #1"), vec![1]);
        assert_eq!(parse_selected_aliases("选择： #4、#6"), vec![4, 6]);
        // Dedupe preserving order; prose and non-alias tokens ignored.
        assert_eq!(
            parse_selected_aliases("SELECTED: #2 #2 #5 某chunk-uuid #x"),
            vec![2, 5]
        );
        assert!(parse_selected_aliases("分析正文，没有圈选行").is_empty());
        // Chunk-id-shaped tokens never parse as aliases.
        assert!(parse_selected_aliases("SELECTED: 6c16ac99-e934").is_empty());
    }

    #[test]
    fn hydration_resolves_in_alias_order_with_dedupe() {
        let h = hydrate_selected("SELECTED: #3, #1, #3", &three_chunks());
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].chunk_id, "c3");
        assert_eq!(h[1].chunk_id, "c1");
        assert_eq!(h[1].text, "第一条证据");
        assert_eq!(h[1].page, Some(1));
    }

    #[test]
    fn unresolvable_alias_is_skipped() {
        let h = hydrate_selected("SELECTED: #2, #99", &three_chunks());
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].chunk_id, "c2");
    }

    #[test]
    fn empty_log_hydrates_to_empty() {
        assert!(hydrate_selected("没有圈选", &three_chunks()).is_empty());
        assert!(hydrate_selected("SELECTED:", &three_chunks()).is_empty());
    }

    #[test]
    fn non_aliased_tools_do_not_count() {
        let mut results = three_chunks();
        results.push(chunk_result(
            "web_search",
            serde_json::json!({"results": [{"url": "https://a", "title": "t"}]}),
        ));
        // Only 3 aliased chunks exist; alias #4 must NOT resolve to web data.
        let h = hydrate_selected("SELECTED: #4", &results);
        assert!(h.is_empty());
    }
}
