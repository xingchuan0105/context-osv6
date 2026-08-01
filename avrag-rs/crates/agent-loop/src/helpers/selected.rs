//! SELECTED alias hydration for the single-agent (and any) run path.
//!
//! Agents circle evidence with `SELECTED: #n` (sandbox bridge aliases).
//! Product citations still filter on `[[cite:chunk_id]]` (ADR-0008). This
//! module replays tool_results in bridge order to resolve `#n` → chunk_id
//! so `filter_citations_for_mode` can keep those chunks.
//!
//! Mirrors app-chat `orchestrator/selected.rs` (keep logic aligned).

use contracts::{ToolResult, ToolStatus};

/// Tools whose chunk lists participate in the `#1 #2 …` alias namespace.
///
/// SaC sandbox injects aliases only for dense / lexical / grep (bridge method
/// names). Host tool_result names below are the capture tags for those paths
/// plus index/grep/read_lines. `graph_retrieval` is intentionally **not**
/// listed: force-augment telemetry side-cars use that tool id and must not
/// enter the SELECTED alias stream (see `codegen_bridge` / citations).
const ALIASED_TOOLS: &[&str] = &[
    "dense_retrieval",
    "lexical_retrieval",
    "index_lookup",
    "doc_grep",
    "doc_read_lines",
    "struct_query",
];

/// Parse `SELECTED:` / `SELECTED：` / `选择:` / `选择：` lines into alias
/// numbers (dedupe, order preserved). Only `#n` tokens count.
pub fn parse_selected_aliases(text: &str) -> Vec<u64> {
    let mut out: Vec<u64> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        // Allow blockquote / list / markdown fence ticks before SELECTED
        // (models often write `SELECTED: #1, #2`).
        let line = trimmed
            .trim_start_matches(|c: char| c == '>' || c == '-' || c == '*' || c == '`' || c == '|')
            .trim_start();
        let line = line.trim_end_matches('`').trim_end();
        let Some(rest) = selected_line_body(line) else {
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

/// Chunk ids in alias order: position `i` is `#{i+1}`.
pub fn alias_chunk_ids_in_order(tool_results: &[ToolResult]) -> Vec<String> {
    let mut out = Vec::new();
    for tr in tool_results {
        if tr.status != ToolStatus::Ok || !ALIASED_TOOLS.contains(&tr.tool.as_str()) {
            continue;
        }
        let Some(data) = tr.data.as_ref() else {
            continue;
        };
        // struct_query carries `chunks` (table-level evidence md) like other
        // aliased tools; dense/lexical/grep carry a chunk list too.
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
            out.push(chunk_id.to_string());
        }
    }
    out
}

/// Resolve SELECTED aliases in `answer` to chunk_ids via tool_results order.
pub fn resolve_selected_chunk_ids(answer: &str, tool_results: &[ToolResult]) -> Vec<String> {
    let aliases = parse_selected_aliases(answer);
    if aliases.is_empty() {
        return Vec::new();
    }
    let stream = alias_chunk_ids_in_order(tool_results);
    let mut out = Vec::new();
    for alias in aliases {
        let Some(id) = (alias as usize)
            .checked_sub(1)
            .and_then(|idx| stream.get(idx))
            .cloned()
        else {
            tracing::warn!(alias, "SELECTED alias did not resolve to a retrieved chunk");
            continue;
        };
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

/// Append synthetic `[[cite:id]]` markers for SELECTED-resolved chunks so
/// ADR-0008 filter keeps them without rewriting the user-visible answer.
pub fn answer_with_selected_cite_markers(answer: &str, tool_results: &[ToolResult]) -> String {
    let ids = resolve_selected_chunk_ids(answer, tool_results);
    if ids.is_empty() {
        return answer.to_string();
    }
    let mut out = answer.to_string();
    // Only append ids not already present as [[cite:…]]
    let existing = crate::cite_extract::extract_referenced_chunk_ids(answer);
    let mut appended = false;
    for id in ids {
        if existing.contains(&id) {
            continue;
        }
        if !appended {
            out.push('\n');
            appended = true;
        }
        out.push_str("[[cite:");
        out.push_str(&id);
        out.push_str("]]");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tr(tool: &str, chunks: serde_json::Value) -> ToolResult {
        ToolResult {
            tool: tool.to_string(),
            version: "1.0".to_string(),
            status: ToolStatus::Ok,
            data: Some(chunks),
            trace: None,
        }
    }

    #[test]
    fn parse_selected_and_blockquote() {
        assert_eq!(parse_selected_aliases("SELECTED: #2, #5\nbody"), vec![2, 5]);
        assert_eq!(parse_selected_aliases("> SELECTED: #1（示例）"), vec![1]);
        assert_eq!(parse_selected_aliases("选择： #4、#6"), vec![4, 6]);
        assert_eq!(parse_selected_aliases("`SELECTED: #1, #2`"), vec![1, 2]);
    }

    #[test]
    fn resolve_aliases_in_order() {
        let results = vec![
            tr(
                "dense_retrieval",
                serde_json::json!({"chunks": [
                    {"chunk_id": "c1", "text": "a"},
                    {"chunk_id": "c2", "text": "b"},
                ]}),
            ),
            tr(
                "doc_grep",
                serde_json::json!({"chunks": [{"chunk_id": "c3", "text": "g"}]}),
            ),
        ];
        assert_eq!(
            resolve_selected_chunk_ids("SELECTED: #3, #1", &results),
            vec!["c3".to_string(), "c1".to_string()]
        );
    }

    #[test]
    fn struct_query_chunks_join_alias_namespace() {
        // struct_query returns `chunks` (table-level evidence md) like dense/grep;
        // bridge aliases it (ALIASED_METHODS), host replays in tool-result order.
        let results = vec![
            tr(
                "dense_retrieval",
                serde_json::json!({"chunks": [{"chunk_id": "c1"}]}),
            ),
            tr(
                "struct_query",
                serde_json::json!({
                    "columns": ["阶段", "cnt"],
                    "rows": [["验证", "59"], ["发布", "30"]],
                    "chunks": [{"chunk_id": "c4", "text": "| 阶段 | cnt |\n| 验证 | 59 |"}],
                }),
            ),
        ];
        assert_eq!(
            resolve_selected_chunk_ids("SELECTED: #2", &results),
            vec!["c4".to_string()]
        );
        // Non-aliased struct_catalog must not enter the namespace.
        let with_catalog = vec![tr(
            "struct_catalog",
            serde_json::json!({"relations": [{"table_name": "t0"}]}),
        )];
        assert!(alias_chunk_ids_in_order(&with_catalog).is_empty());
    }

    #[test]
    fn filter_markers_append_without_duplicating() {
        let results = vec![tr(
            "dense_retrieval",
            serde_json::json!([{"chunk_id": "c1", "text": "t"}]),
        )];
        let ans = "答案正文\nSELECTED: #1";
        let with = answer_with_selected_cite_markers(ans, &results);
        assert!(with.contains("[[cite:c1]]"));
        assert!(with.contains("SELECTED: #1"));
        // Already has cite — no duplicate append of same id
        let already = "x [[cite:c1]]\nSELECTED: #1";
        let with2 = answer_with_selected_cite_markers(already, &results);
        assert_eq!(with2.matches("[[cite:c1]]").count(), 1);
    }
}
