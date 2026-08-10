use avrag_llm::LlmUsage;
use common::AppError;

use super::ReActLoop;
use super::assembler::LoopPhase;
use super::config::{LoopExitConfig, ModeConfig};
use super::exit_policy::has_retrieval_observation;
use super::hooks::{LoopContext, LoopHooks};
use super::iteration::{IterationControl, IterationOutcome, IterationState};
use super::reasoning_emit;
use super::telemetry::ReActIterationRecord;
use crate::events::{AgentEvent, AgentEventSink};
use crate::runtime::AgentRequest;

/// Which budget ceiling ended the retrieval loop. Tokens = primary cost
/// budget; rounds = safety ceiling. Both false → the loop ended on a model
/// decision (DirectAnswer / BreakToSynthesis), not on budget.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct BudgetExhaustion {
    pub rounds: bool,
    pub tokens: bool,
}

impl BudgetExhaustion {
    pub(super) fn any(self) -> bool {
        self.rounds || self.tokens
    }
}

/// Budget already spent before this retrieve invocation (product-run cumulative).
/// Re-entry from short Judge must pass prior usage so token ceilings are not reset.
/// `max_additional_rounds` caps this invocation only (Judge re-retrieve uses a small cap).
#[derive(Debug, Clone)]
pub(super) struct RetrievalBudgetSeed {
    pub prior_usage: LlmUsage,
    /// None → use `max_iterations` from the caller. Some(n) → at most n turns this call.
    pub max_additional_rounds: Option<u8>,
}

impl Default for RetrievalBudgetSeed {
    fn default() -> Self {
        Self {
            prior_usage: LlmUsage::zeroed(),
            max_additional_rounds: None,
        }
    }
}

/// Default max retrieve turns when short Judge routes back to retrieve.
pub(super) const VERIFY_RERETRIEVE_MAX_ROUNDS: u8 = 2;

impl ReActLoop {
    pub(super) async fn run_retrieval_loop(
        &self,
        mode: &ModeConfig,
        request: &AgentRequest,
        auth: &contracts::auth_runtime::AuthContext,
        loop_exit: &LoopExitConfig,
        hooks: &dyn LoopHooks,
        base_message_count: usize,
        max_iterations: u8,
        cancel: &tokio_util::sync::CancellationToken,
        state: &mut IterationState,
        sink: &dyn AgentEventSink,
        budget_seed: RetrievalBudgetSeed,
    ) -> Result<
        (
            // iteration index, billable retrieve turns this invocation (shared product round budget)
            u8,
            u8,
            Option<String>,
            Vec<ReActIterationRecord>,
            LlmUsage,
            BudgetExhaustion,
        ),
        AppError,
    > {
        let mut iteration: u8 = 0;
        let mut rounds_completed: u8 = 0;
        let mut telemetry_records: Vec<ReActIterationRecord> = vec![];
        // Only usage *this* invocation; billable ceiling includes prior_usage.
        let mut session_usage = LlmUsage::zeroed();
        let mut direct_answer: Option<String> = None;
        let mut budget_exhaustion = BudgetExhaustion::default();

        let tier = request.metadata.get("user_tier");
        // Tokens = primary cost budget; rounds = safety ceiling for *this* call.
        // `Some(0)` means no further turns (product rounds already exhausted).
        let effective_max_iters = match budget_seed.max_additional_rounds {
            Some(cap) => cap.min(max_iterations),
            None => max_iterations,
        };
        let effective_max_tokens = mode.budget.resolve_max_tokens(tier);
        let prior_billable = budget_seed.prior_usage.billable_tokens();
        // require_evidence is skill-owned: no host no-chunk grace / hard continue.

        loop {
            if cancel.is_cancelled() {
                break;
            }

            // Billable = uncached tokens only: the re-sent system prefix is
            // provider-cached and must not consume the round budget
            // (LlmUsage::billable_tokens). Include prior_usage so Judge re-retrieve
            // cannot reset the product-run token ceiling.
            let tokens_used = prior_billable.saturating_add(session_usage.billable_tokens());
            let rounds_exhausted = iteration >= effective_max_iters;
            let tokens_exhausted = effective_max_tokens > 0 && tokens_used >= effective_max_tokens;

            if rounds_exhausted || tokens_exhausted {
                if self
                    .check_loop_budget_exhausted(
                        iteration,
                        effective_max_iters,
                        tokens_used,
                        effective_max_tokens,
                        rounds_exhausted,
                        tokens_exhausted,
                        state,
                        sink,
                    )
                    .await
                {
                    budget_exhaustion = BudgetExhaustion {
                        rounds: rounds_exhausted,
                        tokens: tokens_exhausted,
                    };
                    break;
                }
            }

            let has_evidence =
                has_retrieval_observation(&state.messages, &state.tool_results, mode);

            let _ = sink
                .emit(AgentEvent::TurnStart {
                    iteration,
                    phase: "retrieve".to_string(),
                })
                .await;

            let outcome = self
                .run_iteration(
                    iteration,
                    effective_max_iters,
                    mode,
                    request,
                    auth,
                    loop_exit,
                    state,
                    &mut session_usage,
                    sink,
                    hooks,
                    effective_max_tokens,
                )
                .await?;

            self.emit_turn_end_telemetry(iteration, &outcome, sink, &mut telemetry_records)
                .await;

            let control_label = match &outcome.control {
                IterationControl::Continue => "continue",
                IterationControl::BreakToSynthesis { .. } => "break",
                IterationControl::DirectAnswer { .. } => "direct",
            };
            hooks.on_turn_end(iteration, control_label);

            if super::iteration::consumes_iteration_budget(&outcome) {
                rounds_completed = rounds_completed.saturating_add(1);
            }

            match outcome.control {
                IterationControl::Continue => {
                    // E4: compile-feedback continuations are free correction
                    // turns — they do not consume the numbered budget.
                    if super::iteration::consumes_iteration_budget(&outcome) {
                        iteration += 1;
                    }
                }
                IterationControl::BreakToSynthesis { .. } => break,
                IterationControl::DirectAnswer { content } => {
                    direct_answer = Some(content);
                    break;
                }
            }

            hooks.transform_context(
                &mut state.messages,
                &LoopContext {
                    mode,
                    request,
                    iteration,
                    phase: LoopPhase::Retrieve,
                    has_retrieval_observation: has_evidence,
                    base_message_count,
                },
            );

            let _ = sink
                .emit(AgentEvent::BudgetTick {
                    current: iteration,
                    max: effective_max_iters,
                })
                .await;
        }

        Ok((
            iteration,
            rounds_completed,
            direct_answer,
            telemetry_records,
            session_usage,
            budget_exhaustion,
        ))
    }

    pub(super) async fn check_loop_budget_exhausted(
        &self,
        iteration: u8,
        max_iterations: u8,
        tokens_used: u32,
        tokens_max: u32,
        rounds_exhausted: bool,
        tokens_exhausted: bool,
        state: &IterationState,
        sink: &dyn AgentEventSink,
    ) -> bool {
        if !rounds_exhausted && !tokens_exhausted {
            return false;
        }
        let reason = if tokens_exhausted && rounds_exhausted {
            "token_and_round_budget_exhausted"
        } else if tokens_exhausted {
            "token_budget_exhausted"
        } else {
            "iteration_budget_exhausted"
        };
        let disclosed_skills: Vec<String> = state
            .disclosed
            .disclosed_skill_ids
            .iter()
            .cloned()
            .collect();
        let msg = format!(
            "{reason}: rounds={iteration}/{max_iterations} tokens={tokens_used}/{tokens_max}"
        );
        reasoning_emit::emit_evaluation_telemetry(
            sink,
            iteration,
            "budget_exhausted",
            &msg,
            &disclosed_skills,
            "budget_exhausted",
        )
        .await;
        let _ = sink
            .emit(AgentEvent::Activity {
                stage: "budget_exhausted".to_string(),
                message: msg.clone(),
                detail: Some(msg),
                counts: Default::default(),
                sources_preview: Vec::new(),
            })
            .await;
        true
    }

    pub(super) async fn emit_turn_end_telemetry(
        &self,
        iteration: u8,
        outcome: &IterationOutcome,
        sink: &dyn AgentEventSink,
        telemetry_records: &mut Vec<ReActIterationRecord>,
    ) {
        if outcome.sandbox_break {
            return;
        }
        let Some(record) = outcome.record.clone() else {
            return;
        };
        let exit_reason = record.exit_reason.clone();
        let observation_preview = record.observation_preview.clone();
        let disclosed_skills = record.disclosed_skills.clone();
        reasoning_emit::emit_evaluation_telemetry(
            sink,
            iteration,
            &exit_reason,
            &observation_preview,
            &disclosed_skills,
            &exit_reason,
        )
        .await;
        let _ = sink
            .emit(AgentEvent::TurnEnd {
                iteration,
                exit_reason,
            })
            .await;
        telemetry_records.push(record);
    }
}
