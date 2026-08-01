//! Pure helpers for the WriteRefine ReAct loop (no ChatContext / agent service).

use std::path;

use contracts::chat::ToolStatus;
use contracts::{ToolCall, ToolResult};
use heavytail::feedforward::fingerprint_workspace;
use heavytail::validator;
use heavytail::StyleParams;

use crate::refine_types::{RefineContext, RefineLoopBudget};

/// 轮次计数模板（LLM 可见文案，见 `prompts/system/hints/round-counter.md`）。
const ROUND_COUNTER_TEMPLATE: &str =
    include_str!("../../../prompts/system/hints/round-counter.md");

/// Chinese round-counter block + machine-readable budget tag for the LLM.
pub fn build_write_refine_round_counter_zh(
    react_iteration: u8,
    max_react: u8,
    revise_used: usize,
    max_revise: usize,
    research_used: usize,
    max_research: usize,
    budget: &RefineLoopBudget,
) -> String {
    let round = react_iteration.saturating_add(1);
    let react_remaining = max_react.saturating_sub(round);
    let rev_rem = max_revise.saturating_sub(revise_used);
    let res_rem = max_research.saturating_sub(research_used);
    let revise_pick = if budget.revise_rounds_capped() { 0 } else { 1 };
    let research_pick = if budget.research_capped() { 0 } else { 1 };
    let final_pick = if round >= max_react {
        0
    } else if react_remaining <= 1 {
        1
    } else {
        2
    };
    let keys = [
        ("round", round.to_string()),
        ("max_react", max_react.to_string()),
        ("react_remaining", react_remaining.to_string()),
        ("revise_used", revise_used.to_string()),
        ("max_revise", max_revise.to_string()),
        ("rev_rem", rev_rem.to_string()),
        ("research_used", research_used.to_string()),
        ("max_research", max_research.to_string()),
        ("res_rem", res_rem.to_string()),
    ];
    let picks = [
        ("revise_pick", revise_pick),
        ("research_pick", research_pick),
        ("final_pick", final_pick),
    ];
    render_round_counter(ROUND_COUNTER_TEMPLATE, &keys, &picks)
}

/// 极简模板渲染：`{key}` 字面替换 + `{name|备选0|备选1}` 按索引选择
/// （备选内只允许 `{key}`，不含 `|`）。
fn render_round_counter(template: &str, keys: &[(&str, String)], picks: &[(&str, usize)]) -> String {
    fn key_of<'a>(keys: &'a [(&str, String)], k: &str) -> Option<&'a str> {
        keys.iter().find(|(n, _)| *n == k).map(|(_, v)| v.as_str())
    }
    fn pick_of(picks: &[(&str, usize)], p: &str) -> Option<usize> {
        picks.iter().find(|(n, _)| *n == p).map(|(_, v)| *v)
    }
    fn scan_close(s: &str) -> Option<usize> {
        let mut depth = 0usize;
        for (i, c) in s.char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    if depth == 0 {
                        return Some(i);
                    }
                    depth -= 1;
                }
                _ => {}
            }
        }
        None
    }
    fn render_keys_only(s: &str, keys: &[(&str, String)]) -> String {
        let mut out = String::new();
        let mut rest = s;
        while let Some(start) = rest.find('{') {
            out.push_str(&rest[..start]);
            let tail = &rest[start + 1..];
            match scan_close(tail) {
                Some(close) => {
                    if let Some(v) = key_of(keys, &tail[..close]) {
                        out.push_str(v);
                    }
                    rest = &tail[close + 1..];
                }
                None => {
                    out.push('{');
                    rest = tail;
                }
            }
        }
        out.push_str(rest);
        out
    }
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let tail = &rest[start + 1..];
        let Some(close) = scan_close(tail) else {
            out.push('{');
            rest = tail;
            continue;
        };
        let seg = &tail[..close];
        if let Some(pipe) = seg.find('|') {
            let name = &seg[..pipe];
            let content = &tail[pipe + 1..close];
            let alts = content.split('|').collect::<Vec<_>>();
            let idx = pick_of(picks, name).unwrap_or(0);
            if let Some(alt) = alts.get(idx.min(alts.len().saturating_sub(1))) {
                out.push_str(&render_keys_only(alt, keys));
            }
        } else if let Some(v) = key_of(keys, seg) {
            out.push_str(v);
        }
        rest = &tail[close + 1..];
    }
    out.push_str(rest);
    out
}

pub fn strip_task_section(brief: &str) -> String {
    if let Some(idx) = brief.find("## 你的任务") {
        brief[..idx].to_string()
    } else {
        brief.to_string()
    }
}

pub fn core_lexical_bands_unmet(validation: &validator::ValidationReport) -> bool {
    validation.metric_results.iter().any(|m| {
        (m.metric == "hapax_ratio" || m.metric == "zipf_exponent") && !m.passed
    })
}

pub fn core_lexical_bands_met(validation: &validator::ValidationReport) -> bool {
    !core_lexical_bands_unmet(validation)
}

pub fn should_prefer_current_workspace(ctx: &RefineContext, style: &StyleParams) -> bool {
    let cur_fp = fingerprint_workspace(&ctx.workspace);
    let cur_v = validator::validate(&cur_fp, style);
    let cur_core = core_lexical_bands_met(&cur_v);
    let Some(best) = ctx.best_snapshot.as_ref() else {
        return false;
    };
    let best_fp = fingerprint_workspace(&best.workspace);
    let best_v = validator::validate(&best_fp, style);
    let best_core = core_lexical_bands_met(&best_v);
    if cur_core && !best_core {
        return true;
    }
    cur_v.passed && !best_v.passed
}

pub fn synthesize_force_lexical_call(
    ctx: &RefineContext,
    reservoir: &[String],
) -> Option<ToolCall> {
    let hapax_fail = ctx
        .diagnosis
        .validation
        .metric_results
        .iter()
        .any(|m| m.metric == "hapax_ratio" && !m.passed);
    let zipf_fail = ctx
        .diagnosis
        .validation
        .metric_results
        .iter()
        .any(|m| m.metric == "zipf_exponent" && !m.passed);

    if hapax_fail {
        let check = ctx
            .diagnosis
            .validation
            .metric_results
            .iter()
            .find(|m| m.metric == "hapax_ratio")?;
        if check.actual < check.target.0 {
            let term = reservoir
                .iter()
                .find(|t| t.chars().count() >= 2)
                .cloned()?;
            return Some(ToolCall {
                tool: "write_refine_lexical".into(),
                version: "1".into(),
                args: serde_json::json!({
                    "op": "repeat_term",
                    "term": term,
                    "max_edits": 5
                }),
            });
        }
        if check.actual > check.target.1 {
            let from = ctx
                .diagnosis
                .fingerprint
                .word_freq
                .iter()
                .filter(|(_, count)| **count == 1)
                .map(|(word, _): (&String, &usize)| word.clone())
                .next()?;
            let to = reservoir
                .iter()
                .find(|t| t.chars().count() >= 2)
                .cloned()?;
            return Some(ToolCall {
                tool: "write_refine_lexical".into(),
                version: "1".into(),
                args: serde_json::json!({
                    "op": "replace_term",
                    "from": from,
                    "to": to,
                    "max_replacements": 6
                }),
            });
        }
    }
    if zipf_fail {
        let from = ctx
            .diagnosis
            .word_hints
            .iter()
            .find(|h| h.reason.contains("Zipf") || h.action.contains("减到"))
            .map(|h| h.word.clone())
            .or_else(|| {
                ctx.diagnosis
                    .fingerprint
                    .word_freq
                    .iter()
                    .max_by_key(|(_, count)| *count)
                    .map(|(word, _): (&String, &usize)| word.clone())
            })?;
        let to = reservoir
            .iter()
            .find(|t| t.as_str() != from.as_str() && t.chars().count() >= 2)
            .or_else(|| reservoir.first())?
            .clone();
        return Some(ToolCall {
            tool: "write_refine_lexical".into(),
            version: "1".into(),
            args: serde_json::json!({
                "op": "replace_term",
                "from": from,
                "to": to,
                "max_replacements": 8
            }),
        });
    }
    None
}

/// Best-effort refine checkpoint: logs a warning on failure but never aborts.
pub fn checkpoint_refine(ctx: &RefineContext, job_dir: &path::Path) {
    if let Err(err) = ctx.checkpoint(job_dir) {
        tracing::warn!(error = %err, "refine checkpoint failed");
    }
}

pub fn tool_error(tool: &str, msg: &str) -> ToolResult {
    ToolResult {
        tool: tool.to_string(),
        version: "1".to_string(),
        status: ToolStatus::Error,
        data: Some(serde_json::json!({ "error": msg })),
        trace: None,
    }
}

pub fn parse_sentence_id_args(
    value: Option<&serde_json::Value>,
) -> Vec<heavytail::workspace::SentenceId> {
    let Some(arr) = value.and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|v| v.as_str())
        .filter(|s| heavytail::workspace::SentenceId::is_valid(s))
        .map(heavytail::workspace::SentenceId::new)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refine_types::{RefineLoopBudget, WRITE_REFINE_GATE_MAX_REVISE};
    use heavytail::state::WriterBudget;

    #[test]
    fn refine_loop_budget_defaults_match_plan() {
        let b = RefineLoopBudget::default();
        assert_eq!(b.max_rounds, 5);
        assert_eq!(b.max_react_iterations, crate::WRITE_REFINE_HARD_REACT_CAP);
        assert_eq!(b.max_on_demand_research, 5);
        assert_eq!(b.per_research_worker_tokens, 4_000);
        assert_eq!(b.max_refine_tokens, 40_000);
    }

    #[test]
    fn refine_loop_budget_from_writer_budget() {
        let writer = WriterBudget::default();
        let b = RefineLoopBudget::from_writer_budget(&writer, crate::WRITE_REFINE_HARD_REACT_CAP);
        assert_eq!(b.max_rounds, writer.max_rounds);
        assert_eq!(b.max_react_iterations, crate::WRITE_REFINE_HARD_REACT_CAP);
    }

    #[test]
    fn unlimited_budget_still_caps_react_iterations() {
        let b = RefineLoopBudget::unlimited();
        assert_eq!(b.max_react_iterations, crate::WRITE_REFINE_HARD_REACT_CAP);
        assert_eq!(b.max_rounds, WRITE_REFINE_GATE_MAX_REVISE);
        assert!(b.react_iterations_capped());
        assert!(b.revise_rounds_capped());
    }

    #[test]
    fn write_refine_round_counter_shows_remaining_and_last_round_hint() {
        let budget = RefineLoopBudget::unlimited();
        let mid = build_write_refine_round_counter_zh(
            2,
            6,
            1,
            WRITE_REFINE_GATE_MAX_REVISE,
            0,
            usize::MAX,
            &budget,
        );
        assert!(mid.contains("第 3 / 6 轮"));
        assert!(mid.contains("剩余 3 轮"));
        assert!(mid.contains("<write_refine_round"));

        let last = build_write_refine_round_counter_zh(
            5,
            6,
            3,
            WRITE_REFINE_GATE_MAX_REVISE,
            1,
            usize::MAX,
            &budget,
        );
        assert!(last.contains("最后一轮"));
        assert!(last.contains("remaining=\"0\""));
    }

    #[test]
    fn strip_task_section_removes_task_heading() {
        let brief = "## 指标说明\n\nstuff\n\n## 你的任务\n\nDo things.";
        let stripped = strip_task_section(brief);
        assert!(stripped.contains("指标说明"));
        assert!(!stripped.contains("你的任务"));
    }

    #[test]
    fn strip_task_section_preserves_when_no_task() {
        let brief = "## 指标说明\n\nstuff";
        let stripped = strip_task_section(brief);
        assert_eq!(stripped, brief);
    }
}
