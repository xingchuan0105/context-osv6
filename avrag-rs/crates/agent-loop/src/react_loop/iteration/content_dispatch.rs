use avrag_llm::{ChatMessage, LlmResponse};
use common::AppError;

use super::super::config::{LoopExitConfig, ModeConfig};
use super::super::exit_policy::{has_retrieval_observation, should_block_content_early_stop};
use super::super::skill_request::is_skill_request_message;
use super::super::telemetry::ReActIterationRecord;
use super::super::{ReActLoop, truncate_preview};
use super::state::{IterationControl, IterationOutcome, IterationState, disclosed_skill_ids};
use crate::events::AgentEventSink;
use crate::runtime::AgentRunUsage;

impl ReActLoop {
    pub(super) async fn dispatch_content(
        &self,
        iteration: u8,
        mode: &ModeConfig,
        loop_exit: &LoopExitConfig,
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

        let has_evidence_now =
            has_retrieval_observation(&state.messages, &state.tool_results, mode);
        if should_block_content_early_stop(loop_exit, has_evidence_now) {
            state.messages.push(ChatMessage::user(
                "You must retrieve evidence (code execution or tools) before answering. \
                 Continue with retrieval — do not answer from memory alone.",
            ));
            let exit_reason = "content_blocked_no_evidence".to_string();
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

        // S2: worker loops compile the candidate final output at this
        // decision point (design 2026-07-27 §4.3). Error diagnostics reject
        // the output: it is NOT final, the rendered feedback becomes the next
        // observation, and the loop continues — at most once per run. The
        // compiler stays generic: its input comes only from loop state
        // (messages/tool_results), no app-chat types leak in.
        if mode.worker_handoff {
            let observed = crate::output_compiler::observed_chunk_ids(&state.tool_results);
            let outcome =
                crate::output_compiler::compile_handoff(&crate::output_compiler::HandoffCompileInput {
                    raw: &content,
                    observed_chunk_ids: Some(&observed),
                    has_tool_results: !state.tool_results.is_empty(),
                });
            if outcome.has_errors() && state.compile_continuations < MAX_COMPILE_CONTINUATIONS {
                state.compile_continuations += 1;
                let feedback = outcome.render_feedback();
                state.messages.push(ChatMessage::user(feedback.clone()));
                let exit_reason = "compile_feedback".to_string();
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

/// S2: worker loops get at most ONE compile-feedback continuation per run —
/// the rejected output consumes one iteration of budget; when that is spent
/// (or the iteration budget runs out first) the flow falls through to the
/// existing paths (direct answer / C5 budget-exhausted final turn) and the
/// post-loop compile attaches diagnostic codes to the degraded handoff.
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
    }
}
