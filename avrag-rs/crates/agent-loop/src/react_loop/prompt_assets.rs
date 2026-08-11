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

/// C5 when rounds exhausted and no retrieval-side tool attempt this run.
pub fn budget_exhausted_final_turn_no_attempt() -> &'static str {
    trim_body(loop_prompt!("budget-exhausted-final-no-attempt.nudge.md"))
}

/// C5 token ceiling + no retrieval-side tool attempt.
pub fn budget_exhausted_final_turn_tokens_no_attempt() -> &'static str {
    trim_body(loop_prompt!("budget-exhausted-final-tokens-no-attempt.nudge.md"))
}

/// C5 body: budget kind × whether any retrieval tool was attempted.
pub fn budget_exhausted_final_turn_for(
    exhaustion: super::run_retrieval::BudgetExhaustion,
    had_retrieval_attempt: bool,
) -> &'static str {
    let token_only = exhaustion.tokens && !exhaustion.rounds;
    match (token_only, had_retrieval_attempt) {
        (true, false) => budget_exhausted_final_turn_tokens_no_attempt(),
        (true, true) => budget_exhausted_final_turn_tokens(),
        (false, false) => budget_exhausted_final_turn_no_attempt(),
        (false, true) => budget_exhausted_final_turn(),
    }
}

/// Shared-workspace visitor mode: answer only from shared KB observations.
pub fn share_grounded_only_nudge() -> &'static str {
    trim_body(loop_prompt!("share-grounded-only.nudge.md"))
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

/// Working-set char budget demotion fact (LLM-boundary, third-person).
pub fn working_set_trimmed() -> &'static str {
    trim_body(loop_prompt!("working-set-trimmed.nudge.md"))
}

/// Older retrieval observation bodies stubbed (history clear).
pub fn history_cleared() -> &'static str {
    trim_body(loop_prompt!("history-cleared.nudge.md"))
}

pub fn codegen_untrusted_prefix() -> &'static str {
    trim_body(loop_prompt!("codegen-untrusted-prefix.nudge.md"))
}

// --- L2 evidence / L2.5 required-action structural gates (2026-08-03) ---

/// Structural evidence gate: zero answer-grade hits, but retrieval tools **were**
/// attempted (empty Ok / 0 hits). Third-person runtime fact.
pub fn evidence_missing_nudge() -> &'static str {
    trim_body(loop_prompt!("evidence-missing.nudge.md"))
}

/// Structural evidence gate: **no** sandbox retrieval-side tool entries yet
/// (model has not produced a client.* capture). Distinct from zero-hit after call.
pub fn evidence_missing_no_client_nudge() -> &'static str {
    trim_body(loop_prompt!("evidence-missing-no-client.nudge.md"))
}

/// Pick L2 observation by whether any retrieval-layer tool result exists.
pub fn evidence_missing_nudge_for(had_retrieval_attempt: bool) -> &'static str {
    if had_retrieval_attempt {
        evidence_missing_nudge()
    } else {
        evidence_missing_no_client_nudge()
    }
}

/// Required-action: action never appeared in tool_results at all.
pub fn required_action_missing_never(action: &str) -> String {
    subst(
        trim_body(loop_prompt!("required-action-missing-never.tmpl.md")),
        &[("action", action)],
    )
}

/// Required-action: matching tool name seen but no Ok status.
pub fn required_action_missing_error(action: &str) -> String {
    subst(
        trim_body(loop_prompt!("required-action-missing-error.tmpl.md")),
        &[("action", action)],
    )
}

/// Required-action gate: pick never-attempted vs attempted-non-Ok.
pub fn required_action_missing(action: &str, attempted_non_ok: bool) -> String {
    if attempted_non_ok {
        required_action_missing_error(action)
    } else {
        required_action_missing_never(action)
    }
}

/// Synthesis hint when answer-grade aliases exist (SELECTED protocol recency).
pub fn selected_protocol_nudge() -> &'static str {
    trim_body(loop_prompt!("selected-protocol.nudge.md"))
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

/// Lead planning context observation (`[lead_plan_context]`).
pub fn lead_plan_context_observation(
    caps_rag: bool,
    caps_search: bool,
    workspace_note: &str,
    doc_scope_note: &str,
    doc_lines: &str,
) -> String {
    subst(
        trim_body(loop_prompt!("lead-plan-context.tmpl.md")),
        &[
            ("caps_rag", if caps_rag { "是" } else { "否" }),
            ("caps_search", if caps_search { "是" } else { "否" }),
            ("workspace_note", workspace_note),
            ("doc_scope_note", doc_scope_note),
            ("doc_lines", doc_lines),
        ],
    )
}

/// Task Brief observation for a Worker (`[task_brief]`).
pub fn task_brief_observation(brief_json: &str) -> String {
    subst(
        trim_body(loop_prompt!("task-brief.tmpl.md")),
        &[("brief_json", brief_json)],
    )
}

/// EvidencePack observation after PackGate (`[evidence_pack]`).
pub fn evidence_pack_observation(pack_json: &str) -> String {
    subst(
        trim_body(loop_prompt!("evidence-pack.tmpl.md")),
        &[("pack_json", pack_json)],
    )
}

/// Aggregated coverage observation for Lead synthesize (`[coverage_aggregate]`).
pub fn coverage_aggregate_observation(
    n_packs: usize,
    coverage_summary: &str,
    gaps_summary: &str,
    rebrief_used: u8,
) -> String {
    subst(
        trim_body(loop_prompt!("coverage-aggregate.tmpl.md")),
        &[
            ("n_packs", &n_packs.to_string()),
            ("coverage_summary", coverage_summary),
            ("gaps_summary", gaps_summary),
            ("rebrief_used", &rebrief_used.to_string()),
        ],
    )
}

/// After Lead+Workers retrieve: environment fact before product synthesis.
pub fn lead_workers_handoff_to_synthesis(n_packs: usize, coverage_summary: &str) -> String {
    subst(
        trim_body(loop_prompt!("lead-workers-handoff-synthesis.tmpl.md")),
        &[
            ("n_packs", &n_packs.to_string()),
            ("coverage_summary", coverage_summary),
        ],
    )
}

/// Host structural re-brief wave observation (`[rebrief_wave]`).
pub fn rebrief_wave_observation(rebrief_used: u8, channels: &str) -> String {
    subst(
        trim_body(loop_prompt!("rebrief-wave.tmpl.md")),
        &[
            ("rebrief_used", &rebrief_used.to_string()),
            ("channels", channels),
        ],
    )
}

/// RAG Worker short SaC environment fact (`[rag_worker_sac]`).
pub fn rag_worker_sac_observation() -> String {
    trim_body(loop_prompt!("rag-worker-sac.tmpl.md")).to_string()
}

/// Briefs rejected by PlanGate / start gate (`[brief_gate_rejects]`).
pub fn brief_gate_rejects_observation(reject_lines: &str) -> String {
    subst(
        trim_body(loop_prompt!("brief-gate-rejects.tmpl.md")),
        &[("reject_lines", reject_lines)],
    )
}

/// Host BASE tool result observation (`[base_tools_result]`).
pub fn base_tools_result_observation(tool: &str, status: &str, payload: &str) -> String {
    subst(
        trim_body(loop_prompt!("base-tools-result.tmpl.md")),
        &[
            ("tool", tool),
            ("status", status),
            ("payload", payload),
        ],
    )
}

/// Assemble the `{detail}` clause for [`retrieval_summary`] from fragment prompts.
/// Runtime only fills numbers / alias lists; Chinese observation prose lives in
/// `prompts/loop/retrieval-summary-detail-*.md`.
pub fn retrieval_summary_detail(
    aliases: &[String],
    any_truncated: bool,
    any_grep_zero: bool,
    new_aliases: usize,
    seen_aliases: usize,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !aliases.is_empty() {
        // Cap list length so observation stays small.
        let shown: Vec<&str> = aliases.iter().take(24).map(String::as_str).collect();
        let mut list = shown.join(", ");
        if aliases.len() > 24 {
            list.push_str(" …");
        }
        parts.push(subst(
            trim_body(loop_prompt!("retrieval-summary-detail-aliases.tmpl.md")),
            &[("aliases", &list)],
        ));
    }
    parts.push(subst(
        trim_body(loop_prompt!("retrieval-summary-detail-saturation.tmpl.md")),
        &[
            ("n_aliases", &aliases.len().to_string()),
            ("new_aliases", &new_aliases.to_string()),
            ("seen_aliases", &seen_aliases.to_string()),
        ],
    ));
    if any_truncated {
        parts.push(
            trim_body(loop_prompt!("retrieval-summary-detail-truncated.nudge.md")).to_string(),
        );
    }
    if any_grep_zero {
        parts.push(
            trim_body(loop_prompt!("retrieval-summary-detail-grep-zero.nudge.md")).to_string(),
        );
    }
    parts.push(trim_body(loop_prompt!("retrieval-summary-detail-selected.nudge.md")).to_string());
    // Structural join only (semicolon); wrapping punctuation is in the wrap tmpl.
    let joined = parts.join("；");
    subst(
        trim_body(loop_prompt!("retrieval-summary-detail-wrap.tmpl.md")),
        &[("parts", &joined)],
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

// --- short Judge (three-loop, 2026-08-07) ---

macro_rules! pipeline_prompt {
    ($file:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../prompts/pipeline/",
            $file
        ))
    };
}

macro_rules! cluster_skill {
    ($file:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../prompts/clusters/",
            $file
        ))
    };
}

pub fn verify_system() -> &'static str {
    trim_body(pipeline_prompt!("verify.system.md"))
}

pub fn verify_skill_body() -> &'static str {
    trim_body(cluster_skill!("verify/SKILL.md"))
}

/// User turn for one-shot verify (`{question}`, `{final_answer}`, `{evidence}`).
pub fn verify_user(question: &str, final_answer: &str, evidence: &str) -> String {
    subst(
        trim_body(pipeline_prompt!("verify.user.tmpl.md")),
        &[
            ("question", question),
            ("final_answer", final_answer),
            ("evidence", evidence),
        ],
    )
}

/// Filler when verify has no tool/retrieval excerpt to show.
pub fn verify_empty_evidence() -> &'static str {
    trim_body(loop_prompt!("verify-empty-evidence.md"))
}

pub fn verify_fail_synthesis_observation(advice: &str) -> String {
    subst(
        trim_body(loop_prompt!("verify-fail-synthesis.tmpl.md")),
        &[("advice", advice)],
    )
}

pub fn verify_fail_retrieve_observation(advice: &str) -> String {
    subst(
        trim_body(loop_prompt!("verify-fail-retrieve.tmpl.md")),
        &[("advice", advice)],
    )
}

/// Model-only observation when verify fail rounds are exhausted but product
/// token budget remains: one user-facing closeout synthesis turn.
pub fn user_facing_closeout_observation() -> &'static str {
    trim_body(loop_prompt!("user-facing-closeout.nudge.md"))
}

/// Fallback advice when verify returns fail without usable `advice` text.
pub fn verify_empty_advice() -> &'static str {
    trim_body(loop_prompt!("verify-empty-advice.md"))
}

/// Prior synthesis draft for resynthesis after verify fail (revision, not rewrite-from-scratch).
pub fn verify_draft_under_revision(draft: &str) -> String {
    subst(
        trim_body(loop_prompt!("verify-draft-under-revision.tmpl.md")),
        &[("draft", draft)],
    )
}

/// Knockout reexpose observation (`{chunk_ids}`).
pub fn knockout_reexposed_observation(chunk_ids: &str) -> String {
    subst(
        trim_body(loop_prompt!("knockout-reexposed.tmpl.md")),
        &[("chunk_ids", chunk_ids)],
    )
}

/// Synthesis-time EWS recency reread (`{items}` = host-formatted item lines).
pub fn evidence_reread_block(items: &str) -> String {
    if items.trim().is_empty() {
        return String::new();
    }
    subst(
        trim_body(loop_prompt!("evidence-reread.tmpl.md")),
        &[("items", items.trim_end())],
    )
}

pub fn partial_evidence_insufficient() -> &'static str {
    trim_body(loop_prompt!("partial-evidence-insufficient.md"))
}

/// Disaster-only user prose when format gate is exhausted (has evidence path).
/// Not a host footnote on a model draft — full replacement of illegal out-bound text.
pub fn disaster_format_exhausted() -> &'static str {
    trim_body(loop_prompt!("disaster/format-exhausted.md"))
}

/// Disaster-only user prose when there is no retrieval evidence to write from.
pub fn disaster_no_evidence_answer(mode_id: &str) -> &'static str {
    match mode_id {
        "search" => trim_body(loop_prompt!("disaster/search-no-evidence.md")),
        "rag" | "rag+search" => trim_body(loop_prompt!("disaster/no-evidence.md")),
        _ => trim_body(loop_prompt!("disaster/default.md")),
    }
}

/// @deprecated name — use [`disaster_format_exhausted`] / mode-aware helpers.
pub fn contract_violation_fallback(_mode_id: &str) -> &'static str {
    disaster_format_exhausted()
}

/// @deprecated name — use [`disaster_no_evidence_answer`].
pub fn degraded_no_evidence_answer(mode_id: &str) -> &'static str {
    disaster_no_evidence_answer(mode_id)
}

pub fn final_answer_feedback_provider_protocol() -> &'static str {
    trim_body(loop_prompt!("final-answer-feedback-provider-protocol.md"))
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
        assert!(!evidence_missing_nudge().is_empty());
        assert!(!evidence_missing_no_client_nudge().is_empty());
        assert!(evidence_missing_nudge().contains("[evidence_missing]"));
        assert!(evidence_missing_no_client_nudge().contains("[evidence_missing]"));
        assert!(evidence_missing_nudge().contains("已有检索侧调用") || evidence_missing_nudge().contains("已调用"));
        assert!(
            evidence_missing_no_client_nudge().contains("尚未")
                || evidence_missing_no_client_nudge().contains("尚未发生")
        );
        assert_eq!(
            evidence_missing_nudge_for(true),
            evidence_missing_nudge()
        );
        assert_eq!(
            evidence_missing_nudge_for(false),
            evidence_missing_no_client_nudge()
        );
        assert!(!budget_exhausted_final_turn_no_attempt().is_empty());
        assert!(!budget_exhausted_final_turn_tokens_no_attempt().is_empty());
        assert!(required_action_missing("dense", false).contains("尚未出现"));
        assert!(required_action_missing("dense", true).contains("Status=Ok") || required_action_missing("dense", true).contains("成功回传"));
        assert!(selected_protocol_nudge().contains("[selected_protocol]"));
        assert!(user_facing_closeout_observation().contains("[user_facing_closeout]"));
        assert!(!disaster_format_exhausted().is_empty());
        assert!(!disaster_no_evidence_answer("rag").is_empty());
        assert!(!disaster_no_evidence_answer("search").is_empty());
        assert!(!codegen_no_output_nudge().is_empty());
        assert!(!final_answer_feedback_provider_protocol().is_empty());
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
        let rr = evidence_reread_block("- #1 chunk_id=c1 | snip");
        assert!(rr.contains("[evidence_reread]"));
        assert!(rr.contains("#1"));
        assert!(!rr.contains("{items}"));
        assert!(evidence_reread_block("").is_empty());
    }
}
