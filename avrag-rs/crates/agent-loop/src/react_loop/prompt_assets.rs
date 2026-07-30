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

pub fn codegen_sandbox_error_nudge() -> &'static str {
    trim_body(loop_prompt!("codegen-sandbox-error.nudge.md"))
}

pub fn codegen_untrusted_prefix() -> &'static str {
    trim_body(loop_prompt!("codegen-untrusted-prefix.nudge.md"))
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
        assert!(!synthesis_repair_nudge().is_empty());
        assert!(!partial_evidence_insufficient().is_empty());
        assert!(!codegen_no_output_nudge().is_empty());
        assert!(!codegen_sandbox_error_nudge().is_empty());
        let b = blocks_skipped_nudge(3, 2);
        assert!(b.contains('3') && b.contains('2'));
        assert!(!b.contains("{n_blocks}"));
        let s = retrieval_summary(2, 5, "。可见 alias: #1, #2。");
        assert!(s.contains('2') && s.contains('5'));
        assert!(s.contains("#1"));
        assert!(!s.contains("{detail}"));
        assert!(!contract_violation_fallback("rag").is_empty());
        assert!(!degraded_no_evidence_answer("search").is_empty());
    }
}
