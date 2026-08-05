//! LLM-facing loop messages loaded from `avrag-rs/prompts/loop/*.md`
//! (and a few retired bodies under `prompts/deprecated/loop-legacy/`).
//!
//! **Hard rule:** instruction / nudge / user-facing fallback prose for the
//! model or final answer lives only under `prompts/`. This module may
//! `include_str!`, trim, and substitute simple `{placeholders}` — never
//! invent Chinese/English prompt bodies in Rust.
//!
//! Exception: sandbox/SDK observation *data* (tool stdout, retrieval JSON)
//! is runtime feedback, not authored system instructions.

/// Paths relative to `crates/agent-loop` → `avrag-rs/prompts/loop/`.
macro_rules! loop_prompt {
    ($file:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../prompts/loop/",
            $file
        ))
    };
}

/// Retired host observations (no longer injected on SaC skill-owned grounding).
macro_rules! loop_prompt_legacy {
    ($file:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../prompts/deprecated/loop-legacy/",
            $file
        ))
    };
}

/// Paths relative to `crates/agent-loop` → `avrag-rs/prompts/synthesis/`.
macro_rules! synthesis_prompt {
    ($file:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../prompts/synthesis/",
            $file
        ))
    };
}

fn trim_body(raw: &str) -> &str {
    raw.trim()
}

fn subst(template: &str, pairs: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (k, v) in pairs {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

// --- legacy no-chunk (not injected on product SaC path) ---

pub fn no_chunk_continue_nudge() -> &'static str {
    trim_body(loop_prompt_legacy!("no-chunk-continue.nudge.md"))
}

pub fn no_chunk_budget_grace_nudge() -> &'static str {
    trim_body(loop_prompt_legacy!("no-chunk-budget-grace.nudge.md"))
}

pub fn retrieval_failed_final_turn() -> &'static str {
    trim_body(loop_prompt!("retrieval-failed-final.nudge.md"))
}

pub fn budget_exhausted_final_turn() -> &'static str {
    trim_body(loop_prompt!("budget-exhausted-final.nudge.md"))
}

/// Token-budget variant of the C5 closing observation: same wrap-up body,
/// but the stated fact is the token ceiling, not the rounds ceiling.
pub fn budget_exhausted_final_turn_tokens() -> &'static str {
    trim_body(loop_prompt!("budget-exhausted-final-tokens.nudge.md"))
}

pub fn budget_exhausted_carryover(tool: &str, body: &str) -> String {
    subst(
        trim_body(loop_prompt!("budget-exhausted-carryover.tmpl.md")),
        &[("tool", tool), ("body", body)],
    )
}

// --- codegen observation wrappers ---

pub fn blocks_skipped_nudge(n_blocks: usize, n_skipped: usize) -> String {
    subst(
        trim_body(loop_prompt!("blocks-skipped.nudge.md")),
        &[
            ("n_blocks", &n_blocks.to_string()),
            ("n_skipped", &n_skipped.to_string()),
        ],
    )
}

pub fn codegen_no_output_nudge() -> &'static str {
    trim_body(loop_prompt!("codegen-no-output.nudge.md"))
}

/// Sandbox error observation. `{n_fail}` / `{n_max}` = consecutive failure
/// count and host break threshold (third-person environment facts).
pub fn codegen_sandbox_error_nudge(n_fail: u8, n_max: u8) -> String {
    subst(
        trim_body(loop_prompt!("codegen-sandbox-error.nudge.md")),
        &[
            ("n_fail", &n_fail.to_string()),
            ("n_max", &n_max.to_string()),
        ],
    )
}

/// Per-round evidence visibility facts (S+L × P1+).
pub fn evidence_index(
    expanded: usize,
    cards: usize,
    stubs: usize,
    expand_chars: usize,
    pool_aliases: usize,
) -> String {
    subst(
        trim_body(loop_prompt!("evidence-index.tmpl.md")),
        &[
            ("expanded", &expanded.to_string()),
            ("cards", &cards.to_string()),
            ("stubs", &stubs.to_string()),
            ("expand_chars", &expand_chars.to_string()),
            ("pool_aliases", &pool_aliases.to_string()),
        ],
    )
}

/// P1″ cumulative claim notes board (host-extracted fact lines).
pub fn claim_notes(lines: &str, n: usize, max: usize) -> String {
    subst(
        trim_body(loop_prompt!("claim-notes.tmpl.md")),
        &[
            ("lines", lines),
            ("n", &n.to_string()),
            ("max", &max.to_string()),
        ],
    )
}

pub fn codegen_untrusted_prefix() -> &'static str {
    trim_body(loop_prompt!("codegen-untrusted-prefix.nudge.md"))
}

// --- L2 evidence / L2.5 required-action structural gates (2026-08-03) ---

/// Structural evidence gate observation: zero Ok retrieval returns so far,
/// yet the mode requires evidence. Third-person statement of the runtime
/// fact; the model decides the next action (AGENTS.md stop-decision).
pub fn evidence_missing_nudge() -> &'static str {
    trim_body(loop_prompt!("evidence-missing.nudge.md"))
}

/// Required-action gate observation: the query card declared `{action}` but
/// no Ok ToolResult for it has been collected yet.
pub fn required_action_missing(action: &str) -> String {
    subst(
        trim_body(loop_prompt!("required-action-missing.tmpl.md")),
        &[("action", action)],
    )
}

pub fn format_hint_no_space_pipe() -> &'static str {
    trim_body(loop_prompt!("format-hint-no-space-pipe.nudge.md"))
}

pub fn format_hint_key_value() -> &'static str {
    trim_body(loop_prompt!("format-hint-key-value.nudge.md"))
}

/// `detail` is free prose after the counts (leading punctuation included by caller, or empty).
pub fn retrieval_summary(call_count: usize, total_chunks: usize, detail: &str) -> String {
    subst(
        trim_body(loop_prompt!("retrieval-summary.tmpl.md")),
        &[
            ("call_count", &call_count.to_string()),
            ("total_chunks", &total_chunks.to_string()),
            ("detail", detail),
        ],
    )
}

// --- synthesis / answer fallbacks ---

pub fn synthesis_repair_nudge() -> &'static str {
    trim_body(loop_prompt!("synthesis-repair.nudge.md"))
}

// --- synthesis contract blocks (P2-2: verbatim move out of answer_contract) ---

/// JSON-envelope contract appended to the synthesis system prompt
/// (`InternalSearchAnswerV1` modes). Body: `prompts/synthesis/`.
pub fn synthesis_contract_internal_search_answer_v1() -> &'static str {
    trim_body(synthesis_prompt!("contract-internal-search-answer-v1.md"))
}

/// JSON-envelope contract for `InternalAnswerV1` modes.
pub fn synthesis_contract_internal_answer_v1() -> &'static str {
    trim_body(synthesis_prompt!("contract-internal-answer-v1.md"))
}

/// JSON-envelope contract for `InternalAnswerUnifiedV1` /
/// `InternalHybridAnswerV1` modes (thin contract).
pub fn synthesis_contract_internal_answer_unified_v1() -> &'static str {
    trim_body(synthesis_prompt!("contract-internal-answer-unified-v1.md"))
}

// --- final-answer rule feedback hints (P2-2: verbatim move out of
//     final_answer_rules; substituted as `{violation_detail}` into
//     synthesis-prose-repair.tmpl.md) ---

pub fn final_answer_feedback_code_only() -> &'static str {
    trim_body(loop_prompt!("final-answer-feedback-code-only.md"))
}

pub fn final_answer_feedback_host_shell() -> &'static str {
    trim_body(loop_prompt!("final-answer-feedback-host-shell.md"))
}

pub fn final_answer_feedback_template_artifact() -> &'static str {
    trim_body(loop_prompt!("final-answer-feedback-template-artifact.md"))
}

pub fn final_answer_feedback_executable_code() -> &'static str {
    trim_body(loop_prompt!("final-answer-feedback-executable-code.md"))
}

pub fn final_answer_feedback_trailing_code_fence() -> &'static str {
    trim_body(loop_prompt!("final-answer-feedback-trailing-code-fence.md"))
}

/// prose_only synthesis returned a code-only answer (retrieve framing leaked
/// into the final turn): observation that precedes the one repair round.
/// `detail` names the specific violated form (from the final-answer quality
/// gate) so the model sees exactly which shape tripped.
pub fn synthesis_prose_repair_nudge(detail: &str) -> String {
    subst(
        trim_body(loop_prompt!("synthesis-prose-repair.tmpl.md")),
        &[("violation_detail", detail)],
    )
}

/// Evidence-pool rerender observation for prose synthesis: the repair pass
/// still violated the final-form contract, but grounded evidence exists, so
/// the evidence pool is replayed once more for a third synthesis pass.
pub fn synthesis_rerender_nudge() -> &'static str {
    trim_body(loop_prompt!("synthesis-rerender.tmpl.md"))
}

/// User-visible disclosure line deterministically appended by the host when
/// a final answer is released without any retrieval evidence (budget
/// exhaustion or no-evidence synthesis). Not model-authored.
pub fn evidence_missing_disclosure() -> &'static str {
    trim_body(loop_prompt!("evidence-missing-disclosure.md"))
}

pub fn partial_evidence_insufficient() -> &'static str {
    trim_body(loop_prompt!("partial-evidence-insufficient.md"))
}

pub fn contract_violation_fallback(mode_id: &str) -> &'static str {
    match mode_id {
        "rag" => trim_body(loop_prompt!("contract-violation-rag.md")),
        "search" => trim_body(loop_prompt!("contract-violation-search.md")),
        "rag+search" => trim_body(loop_prompt!("contract-violation-dual.md")),
        _ => trim_body(loop_prompt!("contract-violation-default.md")),
    }
}

pub fn degraded_no_evidence_answer(mode_id: &str) -> &'static str {
    match mode_id {
        "rag" => trim_body(loop_prompt!("degraded-no-evidence-rag.md")),
        "search" => trim_body(loop_prompt!("degraded-no-evidence-search.md")),
        _ => trim_body(loop_prompt!("degraded-no-evidence-default.md")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_prompts_are_nonempty() {
        assert!(!no_chunk_continue_nudge().is_empty());
        assert!(!no_chunk_budget_grace_nudge().is_empty());
        assert!(!retrieval_failed_final_turn().is_empty());
        assert!(!budget_exhausted_final_turn().is_empty());
        assert!(!budget_exhausted_final_turn_tokens().is_empty());
        assert!(!synthesis_repair_nudge().is_empty());
        let r = synthesis_prose_repair_nudge("候选答复是代码块形态：围栏之外没有散文正文");
        assert!(!r.is_empty());
        assert!(r.contains("代码块形态"));
        assert!(!r.contains("{violation_detail}"));
        assert!(!partial_evidence_insufficient().is_empty());
        assert!(!codegen_no_output_nudge().is_empty());
        let se = codegen_sandbox_error_nudge(2, 4);
        assert!(!se.is_empty());
        assert!(se.contains("2/4") && !se.contains("{n_fail}"));
        let b = blocks_skipped_nudge(3, 2);
        assert!(b.contains('3') && b.contains('2'));
        assert!(!b.contains("{n_blocks}"));
        let s = retrieval_summary(2, 5, "。可见 alias: #1, #2。");
        assert!(s.contains('2') && s.contains('5'));
        assert!(s.contains("#1"));
        assert!(!s.contains("{detail}"));
        assert!(!contract_violation_fallback("rag").is_empty());
        assert!(!degraded_no_evidence_answer("search").is_empty());
        // P2-2: synthesis contract blocks + final-answer feedback hints.
        assert!(synthesis_contract_internal_search_answer_v1().contains("internal_search_answer_v1"));
        assert!(synthesis_contract_internal_answer_v1().contains("internal_answer_v1"));
        assert!(synthesis_contract_internal_answer_unified_v1().contains("internal_answer_unified_v1"));
        assert!(!final_answer_feedback_code_only().is_empty());
        assert!(!final_answer_feedback_host_shell().is_empty());
        assert!(!final_answer_feedback_template_artifact().is_empty());
        assert!(final_answer_feedback_executable_code().contains("<code language="));
        assert!(final_answer_feedback_trailing_code_fence().contains("代码围栏"));
    }
}
