use avrag_llm::{ChatMessage, LlmResponse, LlmUsage};
use common::AppError;

use super::super::assembler::ContextAssembler;
use super::super::config::ModeConfig;
use super::super::reasoning_emit::{self, record_reasoning};
use super::super::ReActLoop;
use super::state::IterationState;
use crate::agents::events::AgentEventSink;
use crate::agents::runtime::AgentRequest;

impl ReActLoop {
    pub(super) async fn assemble_retrieve_context(
        &self,
        iteration: u8,
        mode: &ModeConfig,
        request: &AgentRequest,
        state: &mut IterationState,
        sink: &dyn AgentEventSink,
    ) -> super::super::assembler::AssembledContext {
        let last_assistant_content = state
            .messages
            .iter()
            .rev()
            .find(|m| m.role == "assistant")
            .map(|m| m.content.as_str());

        let assembled = ContextAssembler::assemble_retrieve(
            iteration,
            mode,
            request,
            &self.skill_registry,
            &mut state.disclosed,
            last_assistant_content,
        );
        reasoning_emit::emit_prompt_snapshot(
            sink,
            "retrieve",
            iteration,
            &assembled,
            &state.disclosed,
        )
        .await;
        reasoning_emit::emit_plan_decision_telemetry(
            sink,
            "retrieve",
            iteration,
            &assembled,
            &state.disclosed,
        )
        .await;
        assembled
    }

    pub(super) async fn call_retrieve_llm(
        &self,
        mode: &ModeConfig,
        state: &mut IterationState,
        total_usage: &mut LlmUsage,
        assembled: &super::super::assembler::AssembledContext,
        sink: &dyn AgentEventSink,
    ) -> Result<LlmResponse, AppError> {
        let mut round_messages = vec![ChatMessage::system(assembled.system_content.clone())];
        for msg in &state.messages {
            if msg.role != "system" {
                round_messages.push(msg.clone());
            }
        }

        let temperature = mode.temperature.unwrap_or(0.7);
        let llm_response = self
            .llm
            .complete_with_tools(&round_messages, &assembled.tools, Some(temperature))
            .await
            .map_err(|e| AppError::internal(format!("llm completion failed: {e}")))?;

        total_usage.accumulate(&llm_response.usage);
        record_reasoning(
            sink,
            &mut state.reasoning_acc,
            llm_response.reasoning_content.as_deref(),
        )
        .await;
        Ok(llm_response)
    }
}
