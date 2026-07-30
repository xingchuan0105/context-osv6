use avrag_llm::{ChatMessage, LlmResponse, LlmUsage};
use common::AppError;

use super::super::ReActLoop;
use super::super::assembler::ContextAssembler;
use super::super::config::ModeConfig;
use super::super::hooks::LoopHooks;
use super::super::reasoning_emit::{self, record_reasoning};
use super::state::IterationState;
use crate::events::AgentEventSink;
use crate::runtime::AgentRequest;

impl ReActLoop {
    pub(super) async fn assemble_retrieve_context(
        &self,
        iteration: u8,
        max_iterations: u8,
        mode: &ModeConfig,
        request: &AgentRequest,
        state: &mut IterationState,
        sink: &dyn AgentEventSink,
        tokens_used: u32,
        tokens_max: u32,
    ) -> super::super::assembler::AssembledContext {
        let last_assistant_content = state
            .messages
            .iter()
            .rev()
            .find(|m| m.role == "assistant")
            .map(|m| m.content.as_str());

        state.disclosed.tokens_used_hint = Some(tokens_used);
        state.disclosed.tokens_max_hint = Some(tokens_max);

        let assembled = ContextAssembler::assemble_retrieve(
            iteration,
            max_iterations,
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
        request: &AgentRequest,
        state: &mut IterationState,
        total_usage: &mut LlmUsage,
        assembled: &super::super::assembler::AssembledContext,
        sink: &dyn AgentEventSink,
        hooks: &dyn LoopHooks,
    ) -> Result<LlmResponse, AppError> {
        let mut round_messages = vec![ChatMessage::system(assembled.system_content.clone())];
        for msg in &state.messages {
            if msg.role != "system" {
                round_messages.push(msg.clone());
            }
        }
        // B5: LLM boundary transform (default: identity).
        let round_messages = hooks.convert_to_llm(&round_messages);

        let temperature = mode.temperature.unwrap_or(0.7);

        // Live-stream retrieve when the client asked for stream:
        // - no tools this round, or
        // - pure chat / prose_only (orchestrator chat exit): prefer progressive
        //   tokens over tool-calling this turn — otherwise the model answers via
        //   non-stream complete_with_tools and the UI freezes for tens of seconds
        //   (acceptance A4). Memory/user_context can still run on non-stream turns.
        let prefer_prose_stream = request.stream
            && (assembled.tools.is_empty()
                || mode.id == "chat"
                || mode.synthesis_output.contract
                    == super::super::config::AnswerContractKind::ProseOnly);
        let llm_response = if prefer_prose_stream {
            self.call_retrieve_llm_stream(
                &round_messages,
                temperature,
                request,
                state,
                sink,
            )
            .await?
        } else {
            self.llm
                .complete_with_tools(&round_messages, &assembled.tools, Some(temperature))
                .await
                .map_err(|e| AppError::internal(format!("llm completion failed: {e}")))?
        };

        total_usage.accumulate(&llm_response.usage);
        record_reasoning(
            sink,
            &mut state.reasoning_acc,
            llm_response.reasoning_content.as_deref(),
        )
        .await;
        Ok(llm_response)
    }

    async fn call_retrieve_llm_stream(
        &self,
        round_messages: &[ChatMessage],
        temperature: f32,
        request: &AgentRequest,
        state: &mut IterationState,
        sink: &dyn AgentEventSink,
    ) -> Result<LlmResponse, AppError> {
        use crate::events::AgentEvent;

        let cancel = request
            .cancellation_token
            .clone()
            .unwrap_or_default();
        let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let (reasoning_tx, mut reasoning_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let stream = self.llm.complete_stream(
            round_messages,
            Some(temperature),
            cancel.clone(),
            move |delta| {
                if !delta.is_empty() {
                    let _ = delta_tx.send(delta.to_string());
                }
            },
            move |delta| {
                if !delta.is_empty() {
                    let _ = reasoning_tx.send(delta.to_string());
                }
            },
        );
        tokio::pin!(stream);

        let mut streamed_any = false;
        let response = loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    return Err(AppError::internal("request cancelled during retrieve stream"));
                }
                delta = delta_rx.recv() => {
                    if let Some(delta) = delta {
                        streamed_any = true;
                        let _ = sink.emit(AgentEvent::MessageDelta { text: delta }).await;
                    }
                }
                reasoning = reasoning_rx.recv() => {
                    if let Some(reasoning) = reasoning {
                        let _ = sink
                            .emit(AgentEvent::ReasoningSummaryDelta { text: reasoning })
                            .await;
                    }
                }
                result = &mut stream => {
                    break result.map_err(|e| {
                        AppError::internal(format!("retrieve stream failed: {e}"))
                    })?;
                }
            }
        };

        while let Ok(delta) = delta_rx.try_recv() {
            streamed_any = true;
            let _ = sink.emit(AgentEvent::MessageDelta { text: delta }).await;
        }
        while let Ok(reasoning) = reasoning_rx.try_recv() {
            let _ = sink
                .emit(AgentEvent::ReasoningSummaryDelta { text: reasoning })
                .await;
        }

        if streamed_any {
            state.answer_deltas_streamed = true;
        }
        Ok(response)
    }
}
