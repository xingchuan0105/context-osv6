//! Final-answer format-level contract rules (data-driven 质检台 registry) and
//! refusal/fallback cues. Split out of `answer_contract/mod.rs` (C5-S4).


/// Strong refusal cues only. Avoid mid-sentence phrases like「未提及…」in analytical prose
/// (that false-positive aborted hybrid salvage and leaked raw synthesis JSON).
pub(crate) const DRAFT_REFUSAL_CUES: &[&str] = &[
    "未在文档中找到",
    "文档中未找到",
    "资料中未找到",
    "资料不足以",
    "无法回答",
    "暂无相关",
    "无相关内容",
    "没有找到相关",
];

pub fn contract_violation_fallback(mode_id: &str) -> String {
    super::super::prompt_assets::contract_violation_fallback(mode_id).to_string()
}

/// Host observation tag shells a final answer must never reproduce. These are
/// the exact tag shapes the host emits as observations in the retrieve phase:
/// `<code_execution_result>`, `<loop_budget …>`, `[retrieval_summary]` (also
/// the angle-bracket variant the model has produced when imitating the shell),
/// `<docscope_metadata>`, and the per-phase cluster index blocks
/// (`<retrieve_cluster_index>` / `<synthesis_skill_index>`). Pure format
/// check — a final prose answer quoting one of these tags means the model
/// pasted a host observation shell instead of writing grounded prose; same
/// class as a code-only answer, routed through the same one-repair-round flow
/// (AGENTS.md stop-decision: no semantic keyword bars, format-level detection
/// only). Tags are matched as prefixes (like `<loop_budget`) so truncated /
/// reworded-closing variants still trip.
pub fn contains_host_observation_shell(text: &str) -> bool {
    // 检测集从备案表派生（host_markers.rs 单一事实源）：所有
    // `forbidden_in_final = true` 的标签。新增宿主观察标签只登记表、
    // 发射端引用常量，检测自动覆盖（parity 测试防漏登记）。
    super::super::host_markers::forbidden_in_final_tags()
        .any(|tag| text.contains(tag))
}

/// Chat-template artifact tokens a final answer must never surface. q018
/// regression (run v2_20260802-045319): the whole final answer was a 12-char
/// `` `</response> `` leak with retrieval recall 1.0 behind it. These tokens
/// are never legitimate user-facing prose.
pub fn contains_template_artifact(text: &str) -> bool {
    template_artifact_matched(text).is_some()
}

/// The specific template token that tripped, if any.
pub(crate) fn template_artifact_matched(text: &str) -> Option<&'static str> {
    const TEMPLATE_ARTIFACTS: &[&str] = &["</response>", "<response>", "<|im_end|>", "<|im_start|>"];
    TEMPLATE_ARTIFACTS
        .iter()
        .find(|t| text.contains(**t))
        .copied()
}

/// Executable-form code span (`<code language="…">`) anywhere in the closing
/// message. Markdown fences are the legitimate way to quote code in prose;
/// the `<code language=…>` shape is the retrieve-phase execution protocol, so
/// its presence means a working draft leaked as the final answer — q095/q102
/// (run v2_20260802-045319): debug narration + an unexecuted (even misspelled)
/// code block shipped as the answer, slipping past `is_code_only_answer`
/// because narration prose surrounded the block.
pub fn contains_executable_code_form(text: &str) -> bool {
    executable_code_matched(text).is_some()
}

/// Rule-level hit marker for the executable-code-form detector (no single
/// specific tag; the shape itself is the marker).
pub(crate) fn executable_code_matched(text: &str) -> Option<&'static str> {
    text.contains("<code language=")
        .then_some("<code language=…> executable-form span")
}

/// Working-draft tail: the answer *ends* with a markdown code fence — nothing
/// but whitespace after the closing marker — while prose exists before it
/// (the no-prose case is `code_only`, earlier in rule order). q017
/// (run v2_20260803-030014): retrieve-phase debug narration + an unexecuted
/// ```python block shipped as the final answer; it slipped past `code_only`
/// (narration prose present) and `executable_code` (markdown fence, not the
/// `<code language=…>` form). A fence with no prose tail is the retrieve-phase
/// codegen shape — the code never executes in the answer phase — so a grounded
/// answer closes on prose (or SELECTED citations), not on a fence.
pub(crate) fn trailing_code_fence_matched(text: &str) -> Option<&'static str> {
    if !text.trim_end().ends_with("```") {
        return None;
    }
    let (saw_code, prose) = split_code_spans(text);
    (saw_code && !prose.trim().is_empty())
        .then_some("markdown 代码围栏收尾：最后一个代码块之后没有正文")
}

/// The specific host observation tag that tripped (from the host_markers
/// single source of truth), if any.
pub(crate) fn host_shell_matched(text: &str) -> Option<&'static str> {
    super::super::host_markers::forbidden_in_final_tags()
        .find(|tag| text.contains(tag))
}

// --- 规则卡注册表（D1：数据驱动的终答质检台） ---

/// One final-answer contract rule. `check` returns the specific matched
/// marker/description when the rule fires; `None` means pass. Rules run in
/// table order. Adding a detector = one table row + one check fn + one test.
pub struct FinalAnswerRule {
    pub id: &'static str,
    pub check: fn(&str) -> Option<&'static str>,
    pub feedback_hint: &'static str,
}

/// The format-level final-answer rules, in detection order. Hint bodies
/// live in `prompts/loop/final-answer-feedback-*.md` (P2-2 verbatim move,
/// loaded via `prompt_assets` — hence `LazyLock` instead of `const`).
pub static FINAL_ANSWER_RULES: std::sync::LazyLock<Vec<FinalAnswerRule>> =
    std::sync::LazyLock::new(|| {
        vec![
            FinalAnswerRule {
                id: "code_only",
                check: |t| {
                    is_code_only_answer(t)
                        .then_some("代码块 / markdown 围栏构成全部内容，没有围栏之外的散文正文")
                },
                feedback_hint: super::super::prompt_assets::final_answer_feedback_code_only(),
            },
            FinalAnswerRule {
                id: "host_shell",
                check: host_shell_matched,
                feedback_hint: super::super::prompt_assets::final_answer_feedback_host_shell(),
            },
            FinalAnswerRule {
                id: "template_artifact",
                check: template_artifact_matched,
                feedback_hint: super::super::prompt_assets::final_answer_feedback_template_artifact(),
            },
            FinalAnswerRule {
                id: "executable_code",
                check: executable_code_matched,
                feedback_hint: super::super::prompt_assets::final_answer_feedback_executable_code(),
            },
            FinalAnswerRule {
                id: "trailing_code_fence",
                check: trailing_code_fence_matched,
                feedback_hint: super::super::prompt_assets::final_answer_feedback_trailing_code_fence(),
            },
        ]
    });

/// A concrete violation found by the quality gate: which rule fired, which
/// specific marker tripped, and the third-person feedback hint for the nudge.
pub struct FinalAnswerViolation {
    pub rule_id: &'static str,
    pub matched: &'static str,
    pub feedback_hint: &'static str,
}

/// The single quality-gate entry point. Every call site (DirectAnswer
/// routing, synthesis pre-repair, synthesis post-repair re-check) uses this;
/// rules are data-driven in [`FINAL_ANSWER_RULES`], so adding a rule never
/// touches engine control flow.
pub fn check_final_answer(text: &str) -> Option<FinalAnswerViolation> {
    FINAL_ANSWER_RULES.iter().find_map(|rule| {
        (rule.check)(text).map(|matched| FinalAnswerViolation {
            rule_id: rule.id,
            matched,
            feedback_hint: rule.feedback_hint,
        })
    })
}

/// Every format-level final-answer contract violation, routed to the same
/// one-repair-round flow (exit-policy direct-answer routing + synthesis
/// gate). Format detection only — no semantic judgment (AGENTS.md
/// stop-decision). Thin wrapper over [`check_final_answer`].
pub fn final_answer_contract_violation(text: &str) -> bool {
    check_final_answer(text).is_some()
}

/// prose_only-contract detector: true when `text` is code-shaped with no
/// surrounding prose — either fenced / `<code>` spans only, or bare
/// unfenced retrieve-phase draft (assignments, `if`/`else:`, `await`,
/// `print(…)`…) shipped as the entire DirectAnswer. Detector only (host
/// structural check); the repair observation lives in
/// `prompts/loop/synthesis-prose-repair.nudge.md` /
/// `final-answer-feedback-code-only.md`.
///
/// Stricter than `parse::parse_llm_output`'s CodeBlocks classification on
/// purpose: a prose answer that *quotes* one fenced query is a valid answer
/// and must not trigger a repair round.
pub fn is_code_only_answer(text: &str) -> bool {
    let (saw_code, prose) = split_code_spans(text);
    let residual = prose.trim();
    if saw_code && residual.is_empty() {
        return true;
    }
    // Fences with residual "prose" that is itself code-shaped, or a fully
    // unfenced multi-line sandbox draft (no fence markers at all).
    let candidate = if residual.is_empty() { text } else { residual };
    is_unfenced_code_shaped(candidate)
}

/// Split `text` into code spans vs outside prose. Returns `(saw_code, prose)`
/// where `prose` is everything outside `<code …>…</code>` spans and markdown
/// fences (any language). Shared by `is_code_only_answer` and
/// `trailing_code_fence_matched`.
fn split_code_spans(text: &str) -> (bool, String) {
    let mut saw_code = false;
    let mut outside = String::new();
    let mut rest = text;
    // `<code …>…</code>` spans (inline or block) — same tag shape parse.rs
    // treats as executable.
    while let Some(start) = rest.find("<code") {
        let Some(tag_end) = rest[start..].find('>').map(|o| start + o) else {
            break;
        };
        let Some(close) = rest[tag_end..].find("</code>").map(|o| tag_end + o) else {
            break;
        };
        outside.push_str(&rest[..start]);
        saw_code = true;
        rest = &rest[close + "</code>".len()..];
    }
    outside.push_str(rest);
    // Markdown fences of ANY language: a fence-only answer is not prose no
    // matter the tag (unlike parse.rs, which only executes python fences).
    let mut prose = String::new();
    let mut in_fence = false;
    for line in outside.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            saw_code = true;
            continue;
        }
        if !in_fence {
            prose.push_str(line);
            prose.push('\n');
        }
    }
    (saw_code, prose)
}

/// True when non-empty lines are predominantly code-shaped (unfenced
/// sandbox / retrieve draft). Pure structural line shapes — no product
/// tool catalogue, no Chinese confession keywords.
fn is_unfenced_code_shaped(text: &str) -> bool {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return false;
    }
    let code_n = lines.iter().filter(|l| line_looks_like_code(l)).count();
    let all_code = code_n == lines.len();
    if all_code && lines.len() >= 2 {
        return true;
    }
    // Single strong statement (e.g. sole `await client.foo(...)`) is still
    // a code-only leak when it is the entire answer.
    if all_code && lines.len() == 1 && line_is_strong_code(lines[0]) {
        return true;
    }
    // ≥3 lines, ≥75% code-shaped, at least one strong signal — tolerates a
    // stray non-code fragment without letting real prose through.
    if lines.len() >= 3
        && code_n * 4 >= lines.len() * 3
        && lines.iter().any(|l| line_is_strong_code(l))
    {
        return true;
    }
    false
}

fn line_looks_like_code(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return false;
    }
    if t.starts_with('#') || t.starts_with("//") || t.starts_with("/*") {
        return true;
    }
    if line_is_strong_code(t) {
        return true;
    }
    // Python / JS block headers and control keywords.
    const CONTROL_PREFIXES: &[&str] = &[
        "if ", "elif ", "else:", "else if", "for ", "while ", "try:", "try ", "except",
        "finally:", "with ", "def ", "class ", "async ", "return ", "raise ", "yield ",
        "assert ", "pass", "break", "continue", "match ", "case ", "lambda ",
    ];
    if CONTROL_PREFIXES.iter().any(|p| t.starts_with(p)) {
        return true;
    }
    // Trailing `:` block header (e.g. `if city:`) — prose rarely ends this way.
    if t.ends_with(':') && t.len() > 1 && !t.starts_with("http") {
        return true;
    }
    // Simple assignment: `name = …` (not `==` / comparison-only).
    if looks_like_assignment(t) {
        return true;
    }
    // Statement-shaped call: `foo(...)` or `obj.method(...)` as whole line.
    if looks_like_call_statement(t) {
        return true;
    }
    false
}

fn line_is_strong_code(line: &str) -> bool {
    let t = line.trim();
    // Prefix / whole-statement shapes only — do not treat mid-prose
    // `client.foo` mentions as a code line (FE display still strips tokens).
    t.starts_with("await ")
        || t.starts_with("import ")
        || t.starts_with("from ")
        || t.starts_with("print(")
        || t.starts_with("def ")
        || t.starts_with("async def ")
        || t.starts_with("async ")
        || t.starts_with("class ")
        || t.starts_with("function ")
        || t.starts_with("const ")
        || t.starts_with("let ")
        || t.starts_with("var ")
        || t.contains(" = await ")
        || t.contains("=await ")
}

fn looks_like_assignment(line: &str) -> bool {
    // `ident = value` or `ident: type = value` — reject pure comparisons.
    let Some(eq) = line.find('=') else {
        return false;
    };
    if line.as_bytes().get(eq + 1) == Some(&b'=') {
        return false; // `==`
    }
    if eq > 0 && matches!(line.as_bytes()[eq - 1], b'!' | b'<' | b'>') {
        return false; // `!=` `<=` `>=`
    }
    let lhs = line[..eq].trim();
    if lhs.is_empty() {
        return false;
    }
    // LHS is a simple identifier or dotted/attr target (city, ctx["city"], x.y).
    lhs.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '[' | ']' | '"' | '\''))
}

fn looks_like_call_statement(line: &str) -> bool {
    let t = line.trim();
    if !(t.ends_with(')') && t.contains('(')) {
        return false;
    }
    // No sentence-ending CJK / Latin period before the call (prose).
    if t.contains('。') || t.contains(". ") {
        return false;
    }
    let Some(open) = t.find('(') else {
        return false;
    };
    let callee = t[..open].trim();
    !callee.is_empty()
        && callee
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.'))
}
