//! Worker handoff compiler (K3 slimmed rule table, design 2026-07-28 §4.3).
//!
//! The handoff contract is now "分析散文 + 可选 SELECTED 行" — prose or a
//! SELECTED-only final message is a VALID worker delivery; JSON remains the
//! way to carry structured fields (summary/gaps/coverage/premise_mismatch).
//! Evidence pointers are code-hydrated (K2), never model-written, so the
//! pointer-validation rules are retired:
//!
//! | code | severity | rule |
//! |------|----------|------|
//! | E101 | RETIRED | envelope check — prose is a legal handoff now |
//! | E102 | RETIRED | key_facts presence — key_facts replaced by SELECTED + hydration |
//! | E103 | RETIRED | evidence pointer authenticity — pointers are hydrated, not model-supplied |
//! | E104 | Warning | `<code_execution_result>` fabrication block — stripped (transformation) |
//! | E105 | Error   | coverage=insufficient declared with ZERO retrieval calls |
//! | W101 | Warning | hedge markers (推断/推测/大概率/可能/未明确) in summary/claim — advisory |
//! | W102 | Warning | fenced JSON — tolerated, advisory only |
//!
//! Only E105 can still trigger the loop's single compile-feedback
//! continuation; E104/W101/W102 never block.

use super::types::{CompileOutcome, Diagnostic};
use crate::react_loop::json_fence::strip_json_fence;

/// Hedge markers that should be labeled as inference rather than asserted
/// (advisory only; the `basis` field lives on the legacy key_facts schema).
const HEDGE_MARKERS: &[&str] = &["推断", "推测", "大概率", "可能", "未明确"];

/// Input for one handoff compile. Pure data so rules stay unit-testable.
pub struct HandoffCompileInput<'a> {
    /// The worker's raw final message.
    pub raw: &'a str,
    /// Whether the loop recorded any tool results at all (drives E105).
    /// At the call site this means ANY tool call happened — a zero-chunk Ok
    /// result still counts as having retrieved (a legitimate 查无).
    pub has_tool_results: bool,
}

/// Compile a worker handoff final message.
///
/// `value` is the parsed JSON when the message is JSON (with E104 stripping
/// applied); `None` for prose / SELECTED-only / unparseable output — which
/// is NOT an error (K3). Error diagnostics exist only for E105.
pub fn compile_handoff(input: &HandoffCompileInput) -> CompileOutcome<serde_json::Value> {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let trimmed = input.raw.trim();

    if trimmed.is_empty() {
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
            "推荐直接输出无围栏的裸 JSON 对象或纯散文",
        ));
    }

    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&body) else {
        // K3: prose / SELECTED-only final message — a legal handoff.
        // E107: fenced code block as the WHOLE final + zero tool calls =
        // unexecuted code passed off as delivery (handover doc §5.2/§6 P0).
        if trimmed.starts_with("```") && !input.has_tool_results {
            diagnostics.push(Diagnostic::error(
                "E107",
                None,
                "整条 handoff 是代码块且本轮零工具调用（未执行代码当交货）",
                "先执行代码（产生工具结果），再以分析散文交付；纯未执行代码不接受",
            ));
        }
        return CompileOutcome {
            value: None,
            diagnostics,
        };
    };

    // E104: strip fabricated <code_execution_result> blocks (transformation;
    // the handoff may still compile — warning only).
    strip_fabricated_blocks(&mut value, &mut diagnostics);

    // E105: coverage=insufficient declared with ZERO retrieval calls this
    // loop — a fabricated "已检索" narrative (see input.has_tool_results).
    check_insufficient_has_retrieval(input.has_tool_results, &value, &mut diagnostics);
    // E106: coverage=full declared with ZERO retrieval calls — fabricated
    // "全覆盖" with no evidence (handover doc §5.2/§6 P0: 假 full 机检).
    check_full_without_evidence(input.has_tool_results, &value, &mut diagnostics);

    // W101: hedge markers — advisory only, never blocks.
    check_hedge_markers(&value, &mut diagnostics);

    CompileOutcome {
        value: Some(value),
        diagnostics,
    }
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

fn check_insufficient_has_retrieval(
    has_tool_results: bool,
    v: &serde_json::Value,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if has_tool_results {
        return;
    }
    let coverage = v
        .get("coverage")
        .and_then(|c| c.as_str())
        .unwrap_or("");
    if !coverage.eq_ignore_ascii_case("insufficient") {
        return;
    }
    diagnostics.push(Diagnostic::error(
        "E105",
        Some("coverage".to_string()),
        "声明 coverage=insufficient（查无），但本轮循环零检索调用",
        "先执行至少一次检索（dense/lexical/graph/doc_scan），再决定是否查无——零检索调用的查无不接受",
    ));
}

/// E106: coverage=full declared with ZERO retrieval calls — a fabricated
/// "全覆盖" with no evidence to back it (handover doc §5.2/§6 P0).
/// Mirror of E105 (insufficient): both catch "conclusion without retrieval".
fn check_full_without_evidence(
    has_tool_results: bool,
    v: &serde_json::Value,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if has_tool_results {
        return;
    }
    let coverage = v
        .get("coverage")
        .and_then(|c| c.as_str())
        .unwrap_or("");
    if !coverage.eq_ignore_ascii_case("full") {
        return;
    }
    diagnostics.push(Diagnostic::error(
        "E106",
        Some("coverage".to_string()),
        "声明 coverage=full（全覆盖），但本轮循环零检索调用",
        "零检索调用无法支撑 full——先执行检索再判定覆盖度，否则降级为 partial/insufficient",
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
        "建议把对应条目以 \"basis\":\"inferred\" 标注为推断而非断言（推断条目的 evidence 可为空）；当前仅提示，不影响接收",
    ));
}
