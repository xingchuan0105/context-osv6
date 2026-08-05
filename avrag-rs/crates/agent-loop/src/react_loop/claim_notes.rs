//! P1″ multi-round claim notes board.
//!
//! Host accumulates one-line fact excerpts from newly **expanded** retrieval
//! hits into a durable run-scoped board. Model-visible via `[claim_notes]`
//! observation (third-person; not a model-authored notes tool).

use serde_json::Value;

/// Cap on cumulative claim lines retained per agent run.
pub const MAX_CLAIM_NOTES: usize = 48;
/// Max chars of each excerpt (one visual line).
pub const CLAIM_EXCERPT_CHARS: usize = 140;

/// One host-extracted fact line tied to an alias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimNoteLine {
    pub alias: String,
    pub excerpt: String,
}

/// Extract claim lines from a single tool `data` payload (chunks/hits/array).
pub fn extract_claim_lines(data: &Value) -> Vec<ClaimNoteLine> {
    let mut out = Vec::new();
    visit_items(data, &mut out);
    out
}

fn visit_items(data: &Value, out: &mut Vec<ClaimNoteLine>) {
    match data {
        Value::Array(arr) => {
            for item in arr {
                push_from_item(item, out);
            }
        }
        Value::Object(_) => {
            if let Some(arr) = data.get("chunks").and_then(|v| v.as_array()) {
                for item in arr {
                    push_from_item(item, out);
                }
            }
            if let Some(arr) = data.get("hits").and_then(|v| v.as_array()) {
                for item in arr {
                    push_from_item(item, out);
                }
            }
        }
        _ => {}
    }
}

fn push_from_item(item: &Value, out: &mut Vec<ClaimNoteLine>) {
    // Reseen / stub: no new fact.
    if item.get("reseen").is_some() {
        return;
    }
    if item.get("visibility").and_then(|v| v.as_str()) == Some("stub") {
        return;
    }
    // Prefer expanded full bodies only (never card/stub residual).
    let vis = item.get("visibility").and_then(|v| v.as_str());
    let body_omitted = item.get("body_omitted").and_then(|v| v.as_bool()) == Some(true);
    if body_omitted {
        return;
    }
    let text = item
        .get("text")
        .and_then(|v| v.as_str())
        .or_else(|| item.get("content").and_then(|v| v.as_str()))
        .unwrap_or("")
        .trim();
    if text.is_empty() {
        return;
    }
    let is_expanded = vis == Some("expanded")
        || (vis.is_none() && text.chars().count() > 40);
    if !is_expanded {
        return;
    }
    let alias = item
        .get("alias")
        .and_then(|v| v.as_str())
        .unwrap_or("?")
        .to_string();
    let excerpt = one_line_excerpt(text, CLAIM_EXCERPT_CHARS);
    if excerpt.is_empty() {
        return;
    }
    out.push(ClaimNoteLine { alias, excerpt });
}

fn one_line_excerpt(text: &str, max_chars: usize) -> String {
    // Prefer first non-empty line; collapse whitespace.
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or(text.trim());
    let collapsed: String = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max_chars {
        return collapsed;
    }
    let head: String = collapsed.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{head}…")
}

/// Merge newly extracted lines into the durable board (alias-keyed upsert;
/// append-only for new aliases; drop oldest when over cap).
pub fn accumulate_claim_notes(board: &mut Vec<ClaimNoteLine>, fresh: &[ClaimNoteLine]) {
    for line in fresh {
        if let Some(existing) = board.iter_mut().find(|n| n.alias == line.alias) {
            // Prefer longer / newer excerpt for same alias.
            if line.excerpt.chars().count() >= existing.excerpt.chars().count() {
                existing.excerpt = line.excerpt.clone();
            }
            continue;
        }
        // Exact excerpt dedupe across aliases (same one-line fact, different #n).
        if board.iter().any(|n| n.excerpt == line.excerpt) {
            continue;
        }
        board.push(line.clone());
    }
    while board.len() > MAX_CLAIM_NOTES {
        board.remove(0);
    }
}

/// Accumulate from a list of Ok retrieval tool data values.
pub fn accumulate_from_tool_datas<'a, I>(board: &mut Vec<ClaimNoteLine>, datas: I)
where
    I: IntoIterator<Item = &'a Value>,
{
    for data in datas {
        let fresh = extract_claim_lines(data);
        accumulate_claim_notes(board, &fresh);
    }
}

/// Render body lines for the claim_notes template (`{lines}` / `{n}` / `{max}`).
pub fn format_claim_note_lines(board: &[ClaimNoteLine]) -> String {
    if board.is_empty() {
        return String::new();
    }
    board
        .iter()
        .map(|n| format!("- {}: {}", n.alias, n.excerpt))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_expanded_only() {
        // Synthetic fixtures only — no realistic corpus / product policy prose.
        let data = json!({
            "chunks": [
                {"chunk_id":"a","alias":"#1","visibility":"expanded","text":"tok_expanded marker_alpha"},
                {"chunk_id":"b","alias":"#2","visibility":"card","text":"tok_card","body_omitted":true},
                {"chunk_id":"c","alias":"#3","reseen":"#1","text":""},
                {"chunk_id":"d","alias":"#4","visibility":"expanded","body_omitted":true,"text":"tok_omitted_residual"},
            ]
        });
        let lines = extract_claim_lines(&data);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].alias, "#1");
        assert!(lines[0].excerpt.contains("marker_alpha"));
    }

    #[test]
    fn accumulate_upserts_and_caps() {
        let mut board = Vec::new();
        accumulate_claim_notes(
            &mut board,
            &[ClaimNoteLine {
                alias: "#1".into(),
                excerpt: "short".into(),
            }],
        );
        accumulate_claim_notes(
            &mut board,
            &[ClaimNoteLine {
                alias: "#1".into(),
                excerpt: "longer excerpt for same alias".into(),
            }],
        );
        assert_eq!(board.len(), 1);
        assert!(board[0].excerpt.starts_with("longer"));

        for i in 0..MAX_CLAIM_NOTES + 5 {
            accumulate_claim_notes(
                &mut board,
                &[ClaimNoteLine {
                    alias: format!("#{i}"),
                    excerpt: format!("fact {i}"),
                }],
            );
        }
        assert_eq!(board.len(), MAX_CLAIM_NOTES);
    }

    #[test]
    fn format_lines_bullet_list() {
        let board = vec![
            ClaimNoteLine {
                alias: "#1".into(),
                excerpt: "alpha".into(),
            },
            ClaimNoteLine {
                alias: "#2".into(),
                excerpt: "beta".into(),
            },
        ];
        let s = format_claim_note_lines(&board);
        assert!(s.contains("- #1: alpha"));
        assert!(s.contains("- #2: beta"));
    }
}
