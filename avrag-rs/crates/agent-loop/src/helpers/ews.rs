//! Evidence Working Set (EWS) — KEEP line protocol + active set (Tier A).
//!
//! Design: `docs/engineering/2026-08-07-kb-skill-hardening-and-evidence-working-set-design.md` §4.
//! Model emits `KEEP: #n, #m`; host keeps those aliases active for next-round
//! priority injection. Empty/missing KEEP → sticky (retain prior active).

use contracts::{ToolResult, ToolStatus};
use serde::{Deserialize, Serialize};

use super::selected::alias_chunk_ids_in_order;

/// Soft cap on snippet chars injected per active item.
const SNIPPET_MAX_CHARS: usize = 200;
/// Cap items in observability / injection lists.
const EWS_ITEMS_MAX: usize = 24;

/// One active EWS entry (alias-keyed working set).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EwsItem {
    /// Numeric alias (1-based), as in `#3`.
    pub alias_num: u64,
    /// Display form `#3`.
    pub alias: String,
    pub chunk_id: String,
    /// Truncated body for priority injection.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub snippet: String,
}

/// Run-scoped EWS ledger (one question / agent turn).
#[derive(Debug, Clone, Default)]
pub struct EwsState {
    active: Vec<EwsItem>,
    /// How many times the host injected `[evidence_reread]` this run (W2).
    reread_injections: u32,
}

/// White-box snapshot for `mode_debug.general.ews`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EwsObservability {
    pub active_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_aliases: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_chunk_ids: Vec<String>,
    /// Host synthesis-time reread injections this run.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub reread_injections: u32,
    /// Items present in the last reread block (0 if never injected).
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub last_reread_item_count: usize,
}

fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}
fn is_zero_usize(v: &usize) -> bool {
    *v == 0
}

impl EwsState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn active(&self) -> &[EwsItem] {
        &self.active
    }

    pub fn observability_snapshot(&self) -> EwsObservability {
        let n = self.active.len().min(EWS_ITEMS_MAX);
        EwsObservability {
            active_count: self.active.len(),
            active_aliases: self.active.iter().take(n).map(|i| i.alias.clone()).collect(),
            active_chunk_ids: self
                .active
                .iter()
                .take(n)
                .map(|i| i.chunk_id.clone())
                .collect(),
            reread_injections: self.reread_injections,
            last_reread_item_count: if self.reread_injections > 0 {
                self.active.len().min(EWS_ITEMS_MAX)
            } else {
                0
            },
        }
    }

    /// Record that synthesis (or resynthesis) received an evidence_reread block.
    pub fn note_reread_injected(&mut self) {
        self.reread_injections = self.reread_injections.saturating_add(1);
    }

    /// Apply KEEP / KEEP_DROP lines from model text.
    ///
    /// - No KEEP line → **sticky** (no change to active).
    /// - KEEP line with zero resolvable aliases → **sticky**.
    /// - KEEP with ≥1 resolved alias → replace active with those items (order preserved).
    /// - KEEP_DROP removes listed aliases from active after KEEP apply.
    pub fn apply_from_model_text(
        &mut self,
        text: &str,
        tool_results: &[ToolResult],
        body_for_chunk: impl Fn(&str) -> Option<String>,
    ) {
        let keep_parse = parse_keep_line(text);
        let drop_aliases = parse_keep_drop_aliases(text);

        if keep_parse.line_present {
            let resolved = resolve_aliases_to_items(
                &keep_parse.aliases,
                tool_results,
                &body_for_chunk,
            );
            if !resolved.is_empty() {
                self.active = resolved;
            }
            // else sticky
        }

        if !drop_aliases.is_empty() {
            self.active
                .retain(|i| !drop_aliases.contains(&i.alias_num));
        }

        if self.active.len() > EWS_ITEMS_MAX {
            self.active.truncate(EWS_ITEMS_MAX);
        }
    }
}

/// Result of scanning model text for a KEEP line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeepLineParse {
    /// True if at least one `KEEP:` / `保留:` line appeared.
    pub line_present: bool,
    pub aliases: Vec<u64>,
}

/// Parse `KEEP: #1, #3` / `保留：#2` lines (dedupe, order preserved).
pub fn parse_keep_line(text: &str) -> KeepLineParse {
    let mut out = KeepLineParse::default();
    for line in text.lines() {
        let trimmed = line.trim();
        let line = trimmed
            .trim_start_matches(|c: char| c == '>' || c == '-' || c == '*' || c == '`' || c == '|')
            .trim_start();
        let line = line.trim_end_matches('`').trim_end();
        // KEEP_DROP handled separately — skip lines that start with KEEP_DROP / 保留删除
        if line_starts_keep_drop(line) {
            continue;
        }
        let Some(rest) = keep_line_body(line) else {
            continue;
        };
        out.line_present = true;
        for n in parse_hash_aliases(rest) {
            if !out.aliases.contains(&n) {
                out.aliases.push(n);
            }
        }
    }
    out
}

/// Parse `KEEP_DROP: #5` / `保留删除: #5` demote lines.
pub fn parse_keep_drop_aliases(text: &str) -> Vec<u64> {
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let line = trimmed
            .trim_start_matches(|c: char| c == '>' || c == '-' || c == '*' || c == '`' || c == '|')
            .trim_start();
        let line = line.trim_end_matches('`').trim_end();
        let Some(rest) = keep_drop_line_body(line) else {
            continue;
        };
        for n in parse_hash_aliases(rest) {
            if !out.contains(&n) {
                out.push(n);
            }
        }
    }
    out
}

fn line_starts_keep_drop(line: &str) -> bool {
    keep_drop_line_body(line).is_some()
}

fn keep_line_body(line: &str) -> Option<&str> {
    for prefix in ["KEEP", "保留"] {
        if let Some(rest) = line.strip_prefix(prefix) {
            // Avoid matching KEEP_DROP
            if rest.starts_with('_') || rest.starts_with("DROP") || rest.starts_with("删除") {
                return None;
            }
            let rest = rest.trim_start();
            let rest = rest.strip_prefix(':').or_else(|| rest.strip_prefix('：'))?;
            return Some(rest.trim());
        }
    }
    None
}

fn keep_drop_line_body(line: &str) -> Option<&str> {
    for prefix in ["KEEP_DROP", "Keep_Drop", "保留删除"] {
        if let Some(rest) = line.strip_prefix(prefix) {
            let rest = rest.trim_start();
            let rest = rest.strip_prefix(':').or_else(|| rest.strip_prefix('：'))?;
            return Some(rest.trim());
        }
    }
    None
}

fn parse_hash_aliases(rest: &str) -> Vec<u64> {
    let mut out = Vec::new();
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
    out
}

fn resolve_aliases_to_items(
    aliases: &[u64],
    tool_results: &[ToolResult],
    body_for_chunk: &impl Fn(&str) -> Option<String>,
) -> Vec<EwsItem> {
    let ordered = alias_chunk_ids_in_order(tool_results);
    let mut items = Vec::new();
    for &n in aliases {
        if n == 0 {
            continue;
        }
        let idx = (n as usize).saturating_sub(1);
        let Some(chunk_id) = ordered.get(idx).cloned() else {
            continue;
        };
        let snippet = body_for_chunk(&chunk_id)
            .or_else(|| snippet_from_tool_results(tool_results, &chunk_id))
            .map(|s| truncate_chars(&s, SNIPPET_MAX_CHARS))
            .unwrap_or_default();
        items.push(EwsItem {
            alias_num: n,
            alias: format!("#{n}"),
            chunk_id,
            snippet,
        });
    }
    items
}

fn snippet_from_tool_results(tool_results: &[ToolResult], chunk_id: &str) -> Option<String> {
    for tr in tool_results {
        if tr.status != ToolStatus::Ok {
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
            let id = item.get("chunk_id").and_then(|v| v.as_str())?;
            if id != chunk_id {
                continue;
            }
            let text = item
                .get("text")
                .or_else(|| item.get("content"))
                .and_then(|v| v.as_str())?;
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }
    None
}

fn truncate_chars(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

/// Format one EWS item line (shared by ews_active + evidence_reread).
fn format_ews_item_line(it: &EwsItem) -> String {
    format!(
        "- {} chunk_id={} {}",
        it.alias,
        it.chunk_id,
        if it.snippet.is_empty() {
            String::new()
        } else {
            format!("| {}", it.snippet.replace('\n', " "))
        }
    )
}

/// Body lines for active items (no host markers); used by prompt templates.
pub fn format_ews_item_lines(items: &[EwsItem]) -> String {
    let mut body = String::new();
    for (i, it) in items.iter().enumerate() {
        if i >= EWS_ITEMS_MAX {
            body.push_str(&format!("…(+{} more)\n", items.len() - EWS_ITEMS_MAX));
            break;
        }
        body.push_str(&format_ews_item_line(it));
        body.push('\n');
    }
    body
}

/// Format active EWS for model-visible injection (third-person host block).
pub fn format_ews_active_block(items: &[EwsItem]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut body = String::from("[ews_active]\n");
    body.push_str("本 run 当前证据工作集（宿主优先注入；未列入集的历史命中可能仅作折叠占位）：\n");
    body.push_str(&format_ews_item_lines(items));
    body.push_str("[/ews_active]");
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ok_chunk(tool: &str, id: &str, alias: &str, text: &str) -> ToolResult {
        ToolResult {
            tool: tool.into(),
            version: "1".into(),
            status: ToolStatus::Ok,
            data: Some(json!([{
                "chunk_id": id,
                "alias": alias,
                "content": text,
            }])),
            trace: None,
        }
    }

    #[test]
    fn parse_keep_and_chinese_prefix() {
        let p = parse_keep_line("noise\nKEEP: #2, #5\nok");
        assert!(p.line_present);
        assert_eq!(p.aliases, vec![2, 5]);
        let p2 = parse_keep_line("> 保留： `#1`");
        assert!(p2.line_present);
        assert_eq!(p2.aliases, vec![1]);
        assert!(!parse_keep_line("SELECTED: #1").line_present);
    }

    #[test]
    fn keep_drop_does_not_count_as_keep_line() {
        let p = parse_keep_line("KEEP_DROP: #3\n");
        assert!(!p.line_present);
        assert_eq!(parse_keep_drop_aliases("KEEP_DROP: #3, #4"), vec![3, 4]);
        assert_eq!(parse_keep_drop_aliases("保留删除：#2"), vec![2]);
    }

    #[test]
    fn sticky_when_no_keep_line() {
        let mut ews = EwsState::new();
        let tr = vec![ok_chunk("dense_retrieval", "c1", "#1", "body one long enough")];
        ews.apply_from_model_text("KEEP: #1\n", &tr, |_| None);
        assert_eq!(ews.active().len(), 1);
        ews.apply_from_model_text("just code, no keep\n", &tr, |_| None);
        assert_eq!(ews.active().len(), 1);
        assert_eq!(ews.active()[0].alias, "#1");
    }

    #[test]
    fn replace_active_on_new_keep() {
        let mut ews = EwsState::new();
        let tr = vec![
            ok_chunk("dense_retrieval", "c1", "#1", "alpha text here for snip"),
            ok_chunk("dense_retrieval", "c2", "#2", "beta text here for snip"),
        ];
        // alias order is stream order: first chunk #1, second #2
        ews.apply_from_model_text("KEEP: #1\n", &tr, |_| None);
        assert_eq!(ews.active()[0].chunk_id, "c1");
        ews.apply_from_model_text("KEEP: #2\n", &tr, |_| None);
        assert_eq!(ews.active().len(), 1);
        assert_eq!(ews.active()[0].chunk_id, "c2");
    }

    #[test]
    fn keep_drop_removes() {
        let mut ews = EwsState::new();
        let a = "a ".repeat(20);
        let b = "b ".repeat(20);
        let tr = vec![
            ok_chunk("dense_retrieval", "c1", "#1", &a),
            ok_chunk("dense_retrieval", "c2", "#2", &b),
        ];
        ews.apply_from_model_text("KEEP: #1, #2\n", &tr, |_| None);
        assert_eq!(ews.active().len(), 2);
        ews.apply_from_model_text("KEEP_DROP: #1\n", &tr, |_| None);
        assert_eq!(ews.active().len(), 1);
        assert_eq!(ews.active()[0].alias_num, 2);
    }

    #[test]
    fn reread_item_lines_include_snippet() {
        let items = vec![EwsItem {
            alias_num: 3,
            alias: "#3".into(),
            chunk_id: "cid-3".into(),
            snippet: "保修两年".into(),
        }];
        let lines = format_ews_item_lines(&items);
        assert!(lines.contains("#3"));
        assert!(lines.contains("cid-3"));
        assert!(lines.contains("保修两年"));
        let active = format_ews_active_block(&items);
        assert!(active.contains("[ews_active]"));
        assert!(active.contains("保修两年"));
    }

    #[test]
    fn note_reread_updates_obs() {
        let mut ews = EwsState::new();
        let tr = vec![ok_chunk("dense_retrieval", "c1", "#1", "body one long enough")];
        ews.apply_from_model_text("KEEP: #1\n", &tr, |_| None);
        assert_eq!(ews.observability_snapshot().reread_injections, 0);
        ews.note_reread_injected();
        let obs = ews.observability_snapshot();
        assert_eq!(obs.reread_injections, 1);
        assert_eq!(obs.last_reread_item_count, 1);
    }
}
