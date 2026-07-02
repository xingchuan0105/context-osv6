use avrag_llm::LlmUsage;
use common::AppError;

use super::ReActLoop;
use super::assembler::LoopPhase;
use super::config::{LoopExitConfig, ModeConfig};
use super::exit_policy::has_retrieval_observation;
use super::hooks::{LoopContext, LoopHooks, StandardLoopHooks};
use super::iteration::{IterationControl, IterationOutcome, IterationState};
use super::reasoning_emit;
use super::telemetry::ReActIterationRecord;
use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::runtime::AgentRequest;

impl ReActLoop {
    pub(super) async fn run_retrieval_loop(
        &self,
        mode: &ModeConfig,
        request: &AgentRequest,
        auth: &avrag_auth::AuthContext,
        loop_exit: &LoopExitConfig,
        hooks: &StandardLoopHooks,
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

        loop {
            if cancel.is_cancelled() {
                break;
            }

            if self
                .check_iteration_budget_exhausted(iteration, max_iterations, state, sink)
                .await
            {
                break;
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
                    max_iterations,
                    mode,
                    request,
                    auth,
                    loop_exit,
                    state,
                    &mut total_usage,
                    sink,
                )
                .await?;

            self.emit_turn_end_telemetry(iteration, &outcome, sink, &mut telemetry_records)
                .await;

            match outcome.control {
                IterationControl::Continue => {
                    iteration += 1;
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
                    max: max_iterations,
                })
                .await;
        }

        Ok((iteration, direct_answer, telemetry_records, total_usage))
    }
    pub(super) async fn check_iteration_budget_exhausted(
        &self,
        iteration: u8,
        max_iterations: u8,
        state: &IterationState,
        sink: &dyn AgentEventSink,
    ) -> bool {
        if iteration < max_iterations {
            return false;
        }
        let disclosed_skills: Vec<String> = state
            .disclosed
            .disclosed_skill_ids
            .iter()
            .cloned()
            .collect();
        reasoning_emit::emit_evaluation_telemetry(
            sink,
            iteration,
            "budget_exhausted",
            "iteration budget exhausted, evaluating exit policy",
            &disclosed_skills,
            "budget_exhausted",
        )
        .await;
        let _ = sink
            .emit(AgentEvent::Activity {
                stage: "budget_exhausted".to_string(),
                message: "iteration budget exhausted, evaluating exit policy".to_string(),
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
