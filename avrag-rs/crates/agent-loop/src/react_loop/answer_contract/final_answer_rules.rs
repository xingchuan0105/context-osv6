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
pub fn template_artifact_matched(text: &str) -> Option<&'static str> {
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
pub fn executable_code_matched(text: &str) -> Option<&'static str> {
    text.contains("<code language=")
        .then_some("<code language=…> executable-form span")
}

/// The specific host observation tag that tripped (from the host_markers
/// single source of truth), if any.
pub fn host_shell_matched(text: &str) -> Option<&'static str> {
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

/// The four format-level final-answer rules, in detection order.
pub const FINAL_ANSWER_RULES: &[FinalAnswerRule] = &[
    FinalAnswerRule {
        id: "code_only",
        check: |t| {
            is_code_only_answer(t)
                .then_some("代码块 / markdown 围栏构成全部内容，没有围栏之外的散文正文")
        },
        feedback_hint: "候选答复是代码块形态：围栏之外没有散文正文；代码只在检索轮经沙箱执行，终答是回传证据之上的普通文字。",
    },
    FinalAnswerRule {
        id: "host_shell",
        check: host_shell_matched,
        feedback_hint: "候选答复中含有宿主观察标签外壳；该标签只由宿主注入，外壳内容不是回传证据。",
    },
    FinalAnswerRule {
        id: "template_artifact",
        check: template_artifact_matched,
        feedback_hint: "候选答复中含有模板残留标记；该标记是模型侧输出残片，不是答复内容。",
    },
    FinalAnswerRule {
        id: "executable_code",
        check: executable_code_matched,
        feedback_hint: "候选答复中含有可执行形态的代码 span（<code language=…>）；该形态只在检索轮经沙箱执行，出现在终答里是过程稿泄漏。",
    },
];

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

/// prose_only-contract detector: true when `text` carries code spans
/// (`<code>…</code>` or markdown fences) but no prose outside them — the
/// retrieve-phase "output one code block" framing leaked into the final
/// answer. Detector only (host structural check); the repair observation
/// lives in `prompts/loop/synthesis-prose-repair.nudge.md`.
///
/// Stricter than `parse::parse_llm_output`'s CodeBlocks classification on
/// purpose: a prose answer that *quotes* one fenced query is a valid answer
/// and must not trigger a repair round.
pub fn is_code_only_answer(text: &str) -> bool {
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
    saw_code && prose.trim().is_empty()
}
