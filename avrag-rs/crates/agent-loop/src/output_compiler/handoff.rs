//! Worker handoff compiler (v1): parse + structurally validate the
//! `internal_worker_handoff_v1` final output against the loop's real tool
//! observations.
//!
//! Rule table (design 2026-07-27 §4.2; C4 structural validation migrated
//! here from app-chat `workers::sanitize_worker_handoff`):
//!
//! | code | severity | rule |
//! |------|----------|------|
//! | E101 | Error   | non-contract envelope / missing handoff structure (`task_result` wrapper, raw code blocks, prose) |
//! | E102 | Error   | loop tool_results non-empty but key_facts missing/empty (and coverage != insufficient) |
//! | E103 | Error   | key_facts[].evidence pointer absent from observed chunk ids (fact dropped, C4 semantics) |
//! | E104 | Warning | `<code_execution_result>` fabrication block — stripped (transformation) |
//! | W101 | Warning | hedge markers (推断/推测/大概率/可能/未明确) in summary/claim — advisory |
//! | W102 | Warning | fenced JSON — tolerated, advisory only |
//!
//! E102 carve-out: `coverage=insufficient + key_facts=[] + gaps=[查无说明]` is
//! the legal "查无即成功" delivery (design §3.1), so E102 does not fire when
//! coverage is insufficient.

use std::collections::HashSet;

use super::types::{CompileOutcome, Diagnostic};
use crate::react_loop::json_fence::strip_json_fence;

const HANDOFF_SCHEMA: &str = "internal_worker_handoff_v1";
const LEGACY_ANSWER_SCHEMA: &str = "internal_answer_v1";

/// Hedge markers that should be labeled as inference rather than asserted
/// (advisory only in v1; the `basis` field arrives with S3).
const HEDGE_MARKERS: &[&str] = &["推断", "推测", "大概率", "可能", "未明确"];

/// Cap on legal-pointer lists inside suggestions (feedback stays compact).
const MAX_POINTERS_IN_SUGGESTION: usize = 8;

/// Input for one handoff compile. Pure data so rules stay unit-testable.
pub struct HandoffCompileInput<'a> {
    /// The worker's raw final message.
    pub raw: &'a str,
    /// Chunk ids the loop actually observed (harvested from Ok tool results).
    /// `None` = run context unknown (pure parse) → E103 pointer checks skipped.
    pub observed_chunk_ids: Option<&'a HashSet<String>>,
    /// Whether the loop recorded any tool results at all (drives E102).
    pub has_tool_results: bool,
}

/// Compile a worker handoff final message.
///
/// The returned value is the (possibly transformed) JSON: E104 strips
/// fabricated execution blocks from summary/claims, E103 drops facts citing
/// unobserved pointers (coverage downgraded to `insufficient` when every fact
/// is dropped) — both semantics-identical to the migrated C4 sanitize.
pub fn compile_handoff(input: &HandoffCompileInput) -> CompileOutcome<serde_json::Value> {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let trimmed = input.raw.trim();

    if trimmed.is_empty() {
        diagnostics.push(e101("empty output — no handoff structure at all"));
        return CompileOutcome {
            value: None,
            diagnostics,
        };
    }

    // W102: fenced JSON is tolerated (C3/C4) but noted.
    let fenced = trimmed.starts_with("```");
    let body = strip_json_fence(trimmed);
    if fenced {
        diagnostics.push(Diagnostic::warning(
            "W102",
            None,
            "输出被 markdown 围栏包裹（已容忍剥离）",
            "契约允许但推荐直接输出无围栏的裸 JSON 对象",
        ));
    }

    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&body) else {
        // Raw code blocks (q087) / prose / anything non-JSON.
        diagnostics.push(e101(
            "output is not parseable JSON (prose or raw code block), not a handoff envelope",
        ));
        return CompileOutcome {
            value: None,
            diagnostics,
        };
    };

    if !is_contract_envelope(&value) {
        // e.g. q045's {"task_result": …} wrapper: correct content, wrong box.
        diagnostics.push(e101(
            "非契约外壳（缺 schema_version=internal_worker_handoff_v1 / summary；如 task_result 包装）",
        ));
        return CompileOutcome {
            value: None,
            diagnostics,
        };
    }

    // E104: strip fabricated <code_execution_result> blocks (transformation;
    // the handoff may still compile — warning only).
    strip_fabricated_blocks(&mut value, &mut diagnostics);

    // E103: drop facts whose evidence pointers were never observed (C4
    // pointer-truthfulness migration). Facts with NO pointers survive.
    if let Some(observed) = input.observed_chunk_ids {
        check_pointer_truthfulness(&mut value, observed, &mut diagnostics);
    }

    // E102: loop observed things but the handoff lists no facts (and the
    // worker is not declaring a legal "查无" via coverage=insufficient).
    check_facts_present(input.has_tool_results, &value, input.observed_chunk_ids, &mut diagnostics);

    // W101: hedge markers — advisory only, never blocks.
    check_hedge_markers(&value, &mut diagnostics);

    CompileOutcome {
        value: Some(value),
        diagnostics,
    }
}

/// Chunk ids actually observed, harvested from recorded tool results
/// (retrieval arrays in both `data: [...]` and `data: {"chunks": [...]}`
/// shapes). Non-Ok results never count as observations. Migrated from app-chat
/// `workers::observed_chunk_ids`.
pub fn observed_chunk_ids(tool_results: &[contracts::ToolResult]) -> HashSet<String> {
    let mut ids = HashSet::new();
    for tr in tool_results {
        if tr.status != contracts::ToolStatus::Ok {
            continue;
        }
        let Some(data) = tr.data.as_ref() else {
            continue;
        };
        let arr = data
            .as_array()
            .or_else(|| data.get("chunks").and_then(|v| v.as_array()));
        let Some(arr) = arr else {
            continue;
        };
        for item in arr {
            if let Some(id) = item.get("chunk_id").and_then(|v| v.as_str()) {
                ids.insert(id.to_string());
            }
        }
    }
    ids
}

/// Remove `<code_execution_result …>…</code_execution_result>` spans (the
/// q039 fabrication vector). An unterminated opening tag strips to the end of
/// the string. Opening tags may carry attributes (e.g. `untrusted="true"`).
/// Migrated verbatim from app-chat `workers::strip_code_execution_blocks`.
pub fn strip_code_execution_blocks(text: &str) -> String {
    const OPEN: &str = "<code_execution_result";
    const CLOSE: &str = "</code_execution_result>";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(OPEN) {
        out.push_str(&rest[..start]);
        let after_open = &rest[start..];
        if let Some(end) = after_open.find(CLOSE) {
            rest = &after_open[end + CLOSE.len()..];
        } else {
            rest = "";
            break;
        }
    }
    out.push_str(rest);
    out.trim().to_string()
}

fn e101(detail: &str) -> Diagnostic {
    Diagnostic::error(
        "E101",
        None,
        format!("missing handoff structure: {detail}"),
        format!(
            "按契约骨架直接输出完整 JSON 对象本身（不要任何外层包装）：\
             {{\"schema_version\":\"{HANDOFF_SCHEMA}\",\"summary\":\"…\",\
             \"key_facts\":[{{\"claim\":\"…\",\"evidence\":[\"chunk-id\"]}}],\
             \"coverage\":\"full|partial|insufficient\",\"gaps\":[\"…\"]}}"
        ),
    )
}

/// Contract envelope check mirrors app-chat `handoff_from_value`: preferred
/// `internal_worker_handoff_v1` (or bare `summary`), legacy
/// `internal_answer_v1` (or `answer_text`) accepted silently.
fn is_contract_envelope(v: &serde_json::Value) -> bool {
    let schema = v
        .get("schema_version")
        .and_then(|s| s.as_str())
        .unwrap_or("");
    schema == HANDOFF_SCHEMA
        || schema == LEGACY_ANSWER_SCHEMA
        || v.get("summary").and_then(|s| s.as_str()).is_some()
        || v.get("answer_text").is_some()
}

fn key_facts_array_mut(v: &mut serde_json::Value) -> Option<&mut Vec<serde_json::Value>> {
    v.get_mut("key_facts").and_then(|k| k.as_array_mut())
}

fn strip_fabricated_blocks(v: &mut serde_json::Value, diagnostics: &mut Vec<Diagnostic>) {
    if let Some(summary) = v.get_mut("summary").and_then(|s| s.as_str()) {
        let stripped = strip_code_execution_blocks(summary);
        if stripped != summary {
            diagnostics.push(Diagnostic::warning(
                "E104",
                Some("summary".to_string()),
                "检出伪造的 <code_execution_result> 块，已剥离",
                "该块一律剥离；请只在 summary 中陈述结论本身",
            ));
            let stripped = stripped.to_string();
            if let Some(slot) = v.get_mut("summary") {
                *slot = serde_json::Value::String(stripped);
            }
        }
    }
    let Some(facts) = key_facts_array_mut(v) else {
        return;
    };
    for (i, fact) in facts.iter_mut().enumerate() {
        let Some(claim) = fact.get("claim").and_then(|c| c.as_str()) else {
            continue;
        };
        let stripped = strip_code_execution_blocks(claim);
        if stripped != claim {
            diagnostics.push(Diagnostic::warning(
                "E104",
                Some(format!("key_facts[{i}].claim")),
                "检出伪造的 <code_execution_result> 块，已剥离",
                "该块一律剥离；claim 只写归纳出的结论",
            ));
            let stripped = stripped.to_string();
            if let Some(slot) = fact.get_mut("claim") {
                *slot = serde_json::Value::String(stripped);
            }
        }
    }
}

fn fact_evidence_ids(fact: &serde_json::Value) -> Vec<&str> {
    fact.get("evidence")
        .and_then(|e| e.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
        .unwrap_or_default()
}

fn check_pointer_truthfulness(
    v: &mut serde_json::Value,
    observed: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(facts) = v.get("key_facts").and_then(|k| k.as_array()) else {
        return;
    };
    // Collect verdicts first (immutable), then mutate.
    let verdicts: Vec<(usize, Vec<String>)> = facts
        .iter()
        .enumerate()
        .filter_map(|(i, fact)| {
            let bogus: Vec<String> = fact_evidence_ids(fact)
                .into_iter()
                .filter(|id| !observed.contains(*id))
                .map(str::to_string)
                .collect();
            (!bogus.is_empty()).then_some((i, bogus))
        })
        .collect();
    if verdicts.is_empty() {
        return;
    }
    let legal = legal_pointer_list(observed);
    let drop_count = verdicts.len();
    for (i, bogus) in &verdicts {
        diagnostics.push(Diagnostic::error(
            "E103",
            Some(format!("key_facts[{i}].evidence")),
            format!("evidence 指针 {:?} 不存在于本次循环的真实观察", bogus),
            format!("只引用真实观察到的 chunk id。合法指针集合：{legal}"),
        ));
    }
    // C4 semantics: drop the tainted facts; when every fact is dropped,
    // coverage downgrades to insufficient.
    let had_facts = !facts.is_empty();
    let dropped: HashSet<usize> = verdicts.iter().map(|(i, _)| *i).collect();
    if let Some(facts) = key_facts_array_mut(v) {
        let mut idx = 0usize;
        facts.retain(|_| {
            let keep = !dropped.contains(&idx);
            idx += 1;
            keep
        });
        if facts.is_empty() && had_facts && drop_count > 0 {
            if let Some(slot) = v.get_mut("coverage") {
                *slot = serde_json::Value::String("insufficient".to_string());
            } else {
                v["coverage"] = serde_json::Value::String("insufficient".to_string());
            }
        }
    }
}

fn check_facts_present(
    has_tool_results: bool,
    v: &serde_json::Value,
    observed: Option<&HashSet<String>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !has_tool_results {
        return;
    }
    let coverage = v
        .get("coverage")
        .and_then(|c| c.as_str())
        .unwrap_or("");
    // 查无即成功 (design §3.1): insufficient + no facts + gaps is a full
    // delivery, not a failure.
    if coverage.eq_ignore_ascii_case("insufficient") {
        return;
    }
    let has_facts = v
        .get("key_facts")
        .and_then(|k| k.as_array())
        .is_some_and(|a| !a.is_empty());
    if has_facts {
        return;
    }
    let observed_count = observed.map(|s| s.len()).unwrap_or(0);
    let pointers = observed.map(legal_pointer_list).unwrap_or_default();
    diagnostics.push(Diagnostic::error(
        "E102",
        Some("key_facts".to_string()),
        format!(
            "循环内工具结果非空（已观察 {observed_count} 个 chunk），但 key_facts 缺失/为空"
        ),
        format!(
            "把已观察到的证据逐条归纳为 key_facts（claim + evidence 指针）。\
             可用指针：{pointers}。若确实查无，请将 coverage 置为 insufficient 并在 gaps 说明查无内容"
        ),
    ));
}

fn check_hedge_markers(v: &serde_json::Value, diagnostics: &mut Vec<Diagnostic>) {
    let hit = |text: &str| -> Option<&'static str> {
        HEDGE_MARKERS.iter().copied().find(|m| text.contains(m))
    };
    let mut fields: Vec<String> = Vec::new();
    if let Some(summary) = v.get("summary").and_then(|s| s.as_str()) {
        if hit(summary).is_some() {
            fields.push("summary".to_string());
        }
    }
    if let Some(facts) = v.get("key_facts").and_then(|k| k.as_array()) {
        for (i, fact) in facts.iter().enumerate() {
            let text = fact
                .get("claim")
                .and_then(|c| c.as_str())
                .or_else(|| fact.as_str())
                .unwrap_or("");
            if hit(text).is_some() {
                fields.push(format!("key_facts[{i}].claim"));
            }
        }
    }
    if fields.is_empty() {
        return;
    }
    diagnostics.push(Diagnostic::warning(
        "W101",
        None,
        format!(
            "{} 含不确定性表述（{}）",
            fields.join(" / "),
            HEDGE_MARKERS.join("/")
        ),
        "建议把对应条目标注为推断（basis: inferred，schema 升级后）而非断言；当前仅提示，不影响接收",
    ));
}

fn legal_pointer_list(observed: &HashSet<String>) -> String {
    if observed.is_empty() {
        return "（空——本次循环没有观察到任何 chunk，不要引用任何 evidence 指针）".to_string();
    }
    let mut ids: Vec<&str> = observed.iter().map(String::as_str).collect();
    ids.sort_unstable();
    let shown: Vec<&str> = ids.iter().take(MAX_POINTERS_IN_SUGGESTION).copied().collect();
    let mut out = shown.join(", ");
    if ids.len() > MAX_POINTERS_IN_SUGGESTION {
        out.push_str(&format!(" …（共 {} 个）", ids.len()));
    }
    out
}
