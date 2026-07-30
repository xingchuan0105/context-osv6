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
    ) -> Result<(u8, Option<String>, Vec<ReActIterationRecord>, LlmUsage), AppError> {
        let mut iteration: u8 = 0;
        let mut telemetry_records: Vec<ReActIterationRecord> = vec![];
        let mut total_usage = LlmUsage::zeroed();
        let mut direct_answer: Option<String> = None;

        let tier = request.metadata.get("user_tier");
        // Tokens = primary cost budget; rounds = safety ceiling.
        let mut effective_max_iters = max_iterations;
        let mut effective_max_tokens = mode.budget.resolve_max_tokens(tier);
        let grace_tokens = mode.budget.resolve_no_chunk_grace_tokens();
        let mut no_chunk_grace_used = false;

        loop {
            if cancel.is_cancelled() {
                break;
            }

            let tokens_used = total_usage.total_tokens;
            let rounds_exhausted = iteration >= effective_max_iters;
            let tokens_exhausted =
                effective_max_tokens > 0 && tokens_used >= effective_max_tokens;

            if rounds_exhausted || tokens_exhausted {
                let has_chunks =
                    has_retrieval_observation(&state.messages, &state.tool_results, mode);
                if loop_exit.require_evidence && !has_chunks && !no_chunk_grace_used {
                    no_chunk_grace_used = true;
                    // Token grace + ensure at least 2 more completes can run.
                    if grace_tokens > 0 {
                        effective_max_tokens = if effective_max_tokens == 0 {
                            grace_tokens
                        } else {
                            effective_max_tokens.saturating_add(grace_tokens)
                        };
                    }
                    effective_max_iters = effective_max_iters
                        .max(iteration.saturating_add(super::exit_policy::NO_CHUNK_BUDGET_GRACE_ROUNDS));
                    state.messages.push(avrag_llm::ChatMessage::user(
                        super::exit_policy::NO_CHUNK_BUDGET_GRACE_NUDGE.to_string(),
                    ));
                    let detail = format!(
                        "no answer-grade chunk at budget; +{grace_tokens} tokens, rounds_cap={effective_max_iters}"
                    );
                    reasoning_emit::emit_evaluation_telemetry(
                        sink,
                        iteration,
                        "budget_grace_no_chunk",
                        &detail,
                        &state
                            .disclosed
                            .disclosed_skill_ids
                            .iter()
                            .cloned()
                            .collect::<Vec<_>>(),
                        "budget_grace_no_chunk",
                    )
                    .await;
                    let _ = sink
                        .emit(AgentEvent::Activity {
                            stage: "budget_grace_no_chunk".to_string(),
                            message: super::exit_policy::NO_CHUNK_BUDGET_GRACE_NUDGE.to_string(),
                            detail: Some(format!(
                                "tokens_used={tokens_used} tokens_max={effective_max_tokens} \
                                 grace_tokens={grace_tokens} max_iterations={effective_max_iters}"
                            )),
                            counts: Default::default(),
                            sources_preview: Vec::new(),
                        })
                        .await;
                    // Continue with raised caps.
                } else if self
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
                    &mut total_usage,
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

        Ok((iteration, direct_answer, telemetry_records, total_usage))
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
