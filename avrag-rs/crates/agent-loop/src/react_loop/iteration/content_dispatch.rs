use avrag_llm::{ChatMessage, LlmResponse};
use common::AppError;

use super::super::config::{LoopExitConfig, ModeConfig};
use super::super::skill_request::is_skill_request_message;
use super::super::telemetry::ReActIterationRecord;
use super::super::{ReActLoop, truncate_preview};
use super::state::{
    COMPILE_FEEDBACK_EXIT_REASON, IterationControl, IterationOutcome, IterationState,
    disclosed_skill_ids,
};
use crate::events::AgentEventSink;
use crate::runtime::AgentRunUsage;

impl ReActLoop {
    pub(super) async fn dispatch_content(
        &self,
        iteration: u8,
        mode: &ModeConfig,
        _request: &crate::runtime::AgentRequest,
        _loop_exit: &LoopExitConfig,
        state: &mut IterationState,
        _sink: &dyn AgentEventSink,
        llm_response: &LlmResponse,
        iter_start: std::time::Instant,
        content: String,
    ) -> Result<IterationOutcome, AppError> {
        let llm_usage = iteration_llm_usage(llm_response);
        state.messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: content.clone(),
            name: None,
            tool_call_id: None,
            tool_calls: None,
            multimodal_content: None,
            reasoning_content: llm_response.reasoning_content.clone(),
        });

        if is_skill_request_message(&content) {
            let exit_reason = "skill_request".to_string();
            return Ok(IterationOutcome {
                control: IterationControl::Continue,
                record: Some(ReActIterationRecord {
                    iteration,
                    disclosed_skills: disclosed_skill_ids(&state.disclosed),
                    action_type: exit_reason.clone(),
                    observation_preview: truncate_preview(&content, 200),
                    llm_usage: Some(llm_usage),
                    elapsed_ms: iter_start.elapsed().as_millis() as u64,
                    exit_reason,
                }),
                sandbox_break: false,
            });
        }

        // Stop / DirectAnswer is model+skill owned (including whether to answer
        // without retrieval). Host does not run semantic coverage gates.
        // Since 2026-08-03 (runtime QC) the host DOES run two STRUCTURAL count
        // gates before accepting a DirectAnswer (see below): zero Ok retrieval
        // returns while the mode requires evidence, and query-card required
        // actions with no Ok ToolResult. `require_evidence` (skill prose) stays
        // model+skill owned — these gates only count Ok returns, they never
        // judge answer quality or coverage.

        // S2: worker loops compile the candidate final output at this
        // decision point (design 2026-07-27 §4.3). Error diagnostics reject
        // the output: it is NOT final, the rendered feedback becomes the next
        // observation, and the loop continues — at most once per run. The
        // compiler stays generic: its input comes only from loop state
        // (messages/tool_results), no app-chat types leak in.
        if mode.worker_handoff {
            let outcome = crate::output_compiler::compile_handoff(
                &crate::output_compiler::HandoffCompileInput {
                    raw: &content,
                    has_tool_results: !state.tool_results.is_empty(),
                },
            );
            if outcome.has_errors() && state.compile_continuations < MAX_COMPILE_CONTINUATIONS {
                state.compile_continuations += 1;
                let feedback = outcome.render_feedback();
                state.messages.push(ChatMessage::user(feedback.clone()));
                let exit_reason = COMPILE_FEEDBACK_EXIT_REASON.to_string();
                return Ok(IterationOutcome {
                    control: IterationControl::Continue,
                    record: Some(ReActIterationRecord {
                        iteration,
                        disclosed_skills: disclosed_skill_ids(&state.disclosed),
                        action_type: exit_reason.clone(),
                        observation_preview: truncate_preview(&feedback, 200),
                        llm_usage: Some(llm_usage),
                        elapsed_ms: iter_start.elapsed().as_millis() as u64,
                        exit_reason,
                    }),
                    sandbox_break: false,
                });
            }
        }

        // L2 / L2.5 structural gates (2026-08-03, runtime QC). Both fire ONLY
        // when the numbered-iteration budget is not about to be exhausted —
        // on exhaustion both gates release (the loop breaks at the next
        // top-of-loop budget check anyway, then C5 synthesis / disclosure
        // handles the final turn). Each rejection is a `Continue` that eats a
        // normal numbered iteration (`consumes_iteration_budget`), so the
        // rounds ceiling guarantees termination.
        // Budget-exhaustion check uses the run's resolved iteration budget
        // (same value the loop's top-of-loop check uses), not a re-resolution.
        let budget_exhausted = iteration.saturating_add(1) >= state.max_iterations;

        if !budget_exhausted {
            // L2 evidence gate: mode requires retrieval evidence (rag/search
            // primitives mounted) but zero Ok retrieval returns so far. The
            // third-person observation states the runtime fact; the model
            // decides the next action (AGENTS.md stop-decision).
            let evidence_required = super::super::policy::exit_policy::requires_evidence(mode);
            if evidence_required
                && !super::super::policy::exit_policy::has_retrieval_observation(
                    &state.messages,
                    &state.tool_results,
                    mode,
                )
            {
                let observation = super::super::prompt_assets::evidence_missing_nudge();
                state.messages.push(ChatMessage::user(observation.to_string()));
                let exit_reason = "evidence_missing_continue".to_string();
                return Ok(IterationOutcome {
                    control: IterationControl::Continue,
                    record: Some(ReActIterationRecord {
                        iteration,
                        disclosed_skills: disclosed_skill_ids(&state.disclosed),
                        action_type: exit_reason.clone(),
                        observation_preview: truncate_preview(&observation, 200),
                        llm_usage: Some(llm_usage),
                        elapsed_ms: iter_start.elapsed().as_millis() as u64,
                        exit_reason,
                    }),
                    sandbox_break: false,
                });
            }

            // L2.5 required-action gate: query card declared actions; any
            // required action without an Ok ToolResult blocks the DirectAnswer
            // once per round (same budget mechanics as above).
            if let Some(card) = state.query_card.as_ref() {
                if let Some(missing) = card.required_actions.iter().find(|action| {
                    !super::super::query_card::required_action_satisfied(action, &state.tool_results)
                }) {
                    let observation =
                        super::super::prompt_assets::required_action_missing(missing);
                    state.messages.push(ChatMessage::user(observation.clone()));
                    let exit_reason = "required_action_missing_continue".to_string();
                    return Ok(IterationOutcome {
                        control: IterationControl::Continue,
                        record: Some(ReActIterationRecord {
                            iteration,
                            disclosed_skills: disclosed_skill_ids(&state.disclosed),
                            action_type: exit_reason.clone(),
                            observation_preview: truncate_preview(&observation, 200),
                            llm_usage: Some(llm_usage),
                            elapsed_ms: iter_start.elapsed().as_millis() as u64,
                            exit_reason,
                        }),
                        sandbox_break: false,
                    });
                }
            }
        }

        // Stop decision: model-owned for chat/write paths. Three-loop modes
        // (`forbid_retrieve_direct_answer`) never ship retrieve prose as the
        // user answer — leave retrieve for synthesis (design 2026-08-07).
        // L2/L2.5 above remain the host structural count gates.

        // Channel workers still ship internal handoff JSON as DirectAnswer;
        // three-loop only applies to user-facing product modes.
        if mode.loop_exit.forbid_retrieve_direct_answer && !mode.worker_handoff {
            let exit_reason = "retrieve_handoff_synthesis".to_string();
            return Ok(IterationOutcome {
                control: IterationControl::BreakToSynthesis {
                    reason: exit_reason.clone(),
                },
                record: Some(ReActIterationRecord {
                    iteration,
                    disclosed_skills: disclosed_skill_ids(&state.disclosed),
                    action_type: exit_reason.clone(),
                    observation_preview: truncate_preview(&content, 200),
                    llm_usage: Some(llm_usage),
                    elapsed_ms: iter_start.elapsed().as_millis() as u64,
                    exit_reason,
                }),
                sandbox_break: false,
            });
        }

        let exit_reason = "direct_content".to_string();
        Ok(IterationOutcome {
            control: IterationControl::DirectAnswer {
                content: content.clone(),
            },
            record: Some(ReActIterationRecord {
                iteration,
                disclosed_skills: disclosed_skill_ids(&state.disclosed),
                action_type: exit_reason.clone(),
                observation_preview: truncate_preview(&content, 200),
                llm_usage: Some(llm_usage),
                elapsed_ms: iter_start.elapsed().as_millis() as u64,
                exit_reason,
            }),
            sandbox_break: false,
        })
    }
}

/// S2/E4: worker loops get at most ONE compile-feedback continuation per
/// run. The continuation is a free correction turn — it does NOT consume the
/// numbered iteration budget (`consumes_iteration_budget`), so a compile
/// failure at the last numbered iteration still gets its retry; when the
/// allowance is spent the flow falls through to the existing paths (direct
/// answer / C5 budget-exhausted final turn) and the post-loop compile
/// attaches diagnostic codes to the degraded handoff.
pub(super) const MAX_COMPILE_CONTINUATIONS: u8 = 1;

pub(crate) fn iteration_llm_usage(llm_response: &LlmResponse) -> AgentRunUsage {
    AgentRunUsage {
        provider: llm_response.usage.provider.clone(),
        model: llm_response.model.clone(),
        prompt_tokens: llm_response.usage.prompt_tokens as u64,
        completion_tokens: llm_response.usage.completion_tokens as u64,
        total_tokens: llm_response.usage.total_tokens as u64,
        request_count: 1,
        cached_tokens: llm_response.usage.cached_tokens as u64,
        reasoning_tokens: llm_response.usage.reasoning_tokens as u64,
    }
}
