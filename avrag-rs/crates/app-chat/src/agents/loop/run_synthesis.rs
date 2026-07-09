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
use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::runtime::{AgentRequest, AgentRunResult, FinalDecision};

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
                    )
                    .await?,
                ));
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
        let _ = sink
            .emit(AgentEvent::MessageDelta {
                text: answer.clone(),
            })
            .await;
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
