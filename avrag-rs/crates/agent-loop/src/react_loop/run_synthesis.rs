use avrag_llm::{ChatMessage, LlmUsage};
use common::AppError;
use contracts::ToolResult;

use super::assembler::{ContextAssembler, DisclosedState};
use super::config::{LoopExitConfig, ModeConfig};
use super::exit_policy::{SynthesisGate, decide_synthesis_gate, has_retrieval_observation};
use super::reasoning_emit;
use super::run_result::{build_run_result, RunContext};
use super::synthesis::SynthesisPhase;
use super::telemetry::ReActIterationRecord;
use super::{ReActLoop, truncate_preview};
use crate::events::{AgentEvent, AgentEventSink};
use crate::runtime::{AgentRequest, AgentRunResult, FinalDecision};

impl ReActLoop {
    pub(super) async fn resolve_synthesis_gate(
        &self,
        mode: &ModeConfig,
        loop_exit: &LoopExitConfig,
        request: &AgentRequest,
        auth: &contracts::auth_runtime::AuthContext,
        retrieval_query: &str,
        direct_answer: Option<&str>,
        messages: &mut Vec<ChatMessage>,
        collected_tool_results: &mut Vec<ToolResult>,
        disclosed_state: &DisclosedState,
        sink: &dyn AgentEventSink,
        iteration: u8,
        max_iterations: u8,
        total_tool_calls: u32,
        telemetry_records: &[ReActIterationRecord],
        total_usage: &LlmUsage,
        reasoning_summary_acc: &str,
        start_time: std::time::Instant,
        answer_deltas_streamed: bool,
    ) -> Result<Option<AgentRunResult>, AppError> {
        let mut has_evidence = has_retrieval_observation(messages, collected_tool_results, mode);

        match decide_synthesis_gate(
            loop_exit,
            has_evidence,
            direct_answer,
            collected_tool_results,
            retrieval_query,
        ) {
            SynthesisGate::SkipSynthesisUseDirect(answer) => {
                // Streaming request that did not live-stream retrieve tokens must
                // not dump a single MessageDelta — fall through to synthesis
                // (`run_prose_stream`) for true progressive tokens.
                if request.stream && !answer_deltas_streamed {
                    tracing::info!(
                        "stream request: skip direct dump, enter synthesis for live tokens"
                    );
                    // Drop the retrieve-phase prose so synthesis is not polluted.
                    if messages
                        .last()
                        .is_some_and(|m| m.role == "assistant" && m.content == answer)
                    {
                        messages.pop();
                    }
                } else {
                    return Ok(Some(
                        self.finish_direct_answer_run(
                            answer,
                            request,
                            disclosed_state,
                            collected_tool_results,
                            sink,
                            iteration,
                            max_iterations,
                            total_tool_calls,
                            telemetry_records,
                            total_usage,
                            reasoning_summary_acc,
                            start_time,
                            "skip_synthesis_direct",
                            FinalDecision::DirectAnswer,
                            answer_deltas_streamed,
                        )
                        .await?,
                    ));
                }
            }
            SynthesisGate::RunFallbackThenCheck => {
                if let Some(result) = self
                    .trigger_auto_fallback_and_check_degraded(
                        mode,
                        loop_exit,
                        request,
                        auth,
                        retrieval_query,
                        messages,
                        collected_tool_results,
                        disclosed_state,
                        sink,
                        iteration,
                        max_iterations,
                        total_tool_calls,
                        telemetry_records,
                        total_usage,
                        reasoning_summary_acc,
                        start_time,
                    )
                    .await?
                {
                    return Ok(Some(result));
                }
                has_evidence = has_retrieval_observation(messages, collected_tool_results, mode);
            }
            SynthesisGate::EnterSynthesis => {}
        }

        let _ = has_evidence;
        Ok(None)
    }

    pub(super) async fn finish_direct_answer_run(
        &self,
        answer: String,
        request: &AgentRequest,
        disclosed_state: &DisclosedState,
        collected_tool_results: &[ToolResult],
        sink: &dyn AgentEventSink,
        iteration: u8,
        max_iterations: u8,
        total_tool_calls: u32,
        telemetry_records: &[ReActIterationRecord],
        total_usage: &LlmUsage,
        reasoning_summary_acc: &str,
        start_time: std::time::Instant,
        telemetry_label: &str,
        final_decision: FinalDecision,
        content_already_streamed: bool,
    ) -> Result<AgentRunResult, AppError> {
        let disclosed_skills: Vec<String> = disclosed_state
            .disclosed_skill_ids
            .iter()
            .cloned()
            .collect();
        let observation_preview = truncate_preview(&answer, 200);
        reasoning_emit::emit_evaluation_telemetry(
            sink,
            iteration,
            telemetry_label,
            &observation_preview,
            &disclosed_skills,
            telemetry_label,
        )
        .await;
        // Avoid a second full-answer MessageDelta when retrieve already streamed
        // live tokens (acceptance A4: one compose-stage blob felt frozen).
        if !content_already_streamed {
            let _ = sink
                .emit(AgentEvent::MessageDelta {
                    text: answer.clone(),
                })
                .await;
        }
        let _ = sink
            .emit(AgentEvent::Done {
                final_message: Some(answer.clone()),
                usage: None,
            })
            .await;
        self.finish_run(
            sink,
            answer,
            request,
            collected_tool_results,
            telemetry_records,
            total_usage,
            reasoning_summary_acc,
            iteration,
            max_iterations,
            total_tool_calls,
            start_time,
            Some(final_decision),
        )
        .await
    }

    pub(super) async fn run_synthesis_phase(
        &self,
        mode: &ModeConfig,
        request: &AgentRequest,
        disclosed_state: &mut DisclosedState,
        messages: &[ChatMessage],
        collected_tool_results: &[ToolResult],
        sink: &dyn AgentEventSink,
        cancel: &tokio_util::sync::CancellationToken,
        iteration: u8,
        max_iterations: u8,
        total_tool_calls: u32,
        telemetry_records: &[ReActIterationRecord],
        total_usage: &LlmUsage,
        reasoning_summary_acc: &str,
        start_time: std::time::Instant,
    ) -> Result<AgentRunResult, AppError> {
        let synthesis_ctx = ContextAssembler::assemble_synthesis(
            mode,
            request,
            &self.skill_registry,
            disclosed_state,
        );
        reasoning_emit::emit_prompt_snapshot(
            sink,
            "synthesis",
            iteration,
            &synthesis_ctx,
            disclosed_state,
        )
        .await;
        reasoning_emit::emit_plan_decision_telemetry(
            sink,
            "synthesis",
            iteration,
            &synthesis_ctx,
            disclosed_state,
        )
        .await;

        let synthesis = SynthesisPhase;
        // C5: a loop that exhausted its iteration budget gets ONE explicit
        // final turn before synthesis: stop emitting code/retrieval and
        // produce the final output the task brief requires (the worker's
        // internal handoff JSON). Without it, exhausted workers reach the
        // prose-only stream with the retrieve-phase "output one <code> block"
        // framing still dominant and emit raw code or fabrications.
        let exhausted = budget_exhausted_messages(messages, iteration, max_iterations);
        let messages = exhausted.as_deref().unwrap_or(messages);
        let final_answer = synthesis
            .run(
                &self.llm,
                &synthesis_ctx,
                mode,
                messages,
                collected_tool_results,
                sink,
                cancel,
            )
            .await?;

        let disclosed_skills: Vec<String> = disclosed_state
            .disclosed_skill_ids
            .iter()
            .cloned()
            .collect();
        let observation_preview = truncate_preview(&final_answer, 200);
        reasoning_emit::emit_evaluation_telemetry(
            sink,
            iteration,
            "synthesized",
            &observation_preview,
            &disclosed_skills,
            "synthesized",
        )
        .await;

        self.finish_run(
            sink,
            final_answer,
            request,
            collected_tool_results,
            telemetry_records,
            total_usage,
            reasoning_summary_acc,
            iteration,
            max_iterations,
            total_tool_calls,
            start_time,
            Some(FinalDecision::Synthesized),
        )
        .await
    }
    pub(super) async fn emit_run_citations(
        &self,
        sink: &dyn AgentEventSink,
        citations: &[contracts::chat::Citation],
    ) {
        if !citations.is_empty() {
            let _ = sink
                .emit(AgentEvent::Citations {
                    citations: citations.to_vec(),
                })
                .await;
        }
    }

    pub(super) async fn finish_run(
        &self,
        sink: &dyn AgentEventSink,
        final_answer: String,
        request: &AgentRequest,
        collected_tool_results: &[ToolResult],
        telemetry_records: &[ReActIterationRecord],
        total_usage: &LlmUsage,
        reasoning_summary_acc: &str,
        iteration: u8,
        max_iterations: u8,
        total_tool_calls: u32,
        start_time: std::time::Instant,
        final_decision: Option<FinalDecision>,
    ) -> Result<AgentRunResult, AppError> {
        let ctx = RunContext {
            iteration,
            max_iterations,
            total_tool_calls,
            telemetry_records,
            total_usage,
            reasoning_summary_acc,
            start_time,
        };
        let result = build_run_result(
            &self.llm,
            final_answer,
            request,
            collected_tool_results,
            &ctx,
            final_decision,
        );
        self.emit_run_citations(sink, &result.citations).await;
        Ok(result)
    }
}

/// Final-turn instruction appended when the retrieval loop exhausts its
/// iteration budget (C5): stop emitting code blocks / new retrieval and
/// produce the final output the task brief requires. Generic on purpose —
/// the worker brief (app-chat) requires the internal_worker_handoff_v1 JSON,
/// chat modes require prose; this turn re-asserts "wrap up per brief" either
/// way.
pub(crate) const BUDGET_EXHAUSTED_FINAL_TURN: &str = "\
迭代预算已用尽。不要再输出任何 <code> 代码块，也不要发起新的检索或工具调用。\n\
立即按任务简报（Task brief）要求的最终输出格式给出最终答复：\n\
- 若简报要求输出内部交接 JSON（schema_version=internal_worker_handoff_v1，\n\
  含 summary / key_facts / coverage / gaps），请直接输出该 JSON 对象本身\n\
  （不要 <code> 块，不要 markdown 围栏）；\n\
- 否则直接给出最终结论散文，并如实说明已找到/未找到的证据。";

/// C5: when the retrieval loop exhausted its iteration budget, append ONE
/// final user turn carrying `BUDGET_EXHAUSTED_FINAL_TURN` so the synthesis
/// call starts from an explicit handoff instruction instead of the
/// retrieve-phase framing. Returns `Some(new_messages)` when appended.
fn budget_exhausted_messages(
    messages: &[ChatMessage],
    iteration: u8,
    max_iterations: u8,
) -> Option<Vec<ChatMessage>> {
    if iteration < max_iterations {
        return None;
    }
    let mut out = messages.to_vec();
    out.push(ChatMessage::user(BUDGET_EXHAUSTED_FINAL_TURN));
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_exhaustion_appends_handoff_turn() {
        let history = vec![
            ChatMessage::system("retrieve with <code> blocks"),
            ChatMessage::user("question"),
        ];
        let out = budget_exhausted_messages(&history, 6, 6).expect("exhausted → appended");
        assert_eq!(out.len(), history.len() + 1);
        let last = out.last().unwrap();
        assert_eq!(last.role, "user");
        assert!(last.content.contains("internal_worker_handoff_v1"));
        assert!(last.content.contains("不要再输出任何 <code> 代码块"));
        assert!(last.content.contains("不要发起新的检索"));
        assert!(last.content.contains("summary / key_facts / coverage / gaps"));
    }

    #[test]
    fn no_final_turn_before_budget_exhaustion() {
        let history = vec![ChatMessage::user("question")];
        assert!(budget_exhausted_messages(&history, 5, 6).is_none());
        assert!(budget_exhausted_messages(&history, 0, 6).is_none());
    }
}
