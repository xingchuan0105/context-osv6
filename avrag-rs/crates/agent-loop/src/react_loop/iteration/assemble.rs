use avrag_llm::{ChatMessage, LlmClient, LlmResponse, LlmUsage};
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
        let mut system = assembled.system_content.clone();
        if mode.id == "chat" {
            system.push_str("\n\n");
            system.push_str(super::super::prompt_assets::chat_answer_channel());
        }
        let mut round_messages = vec![ChatMessage::system(system)];
        // Model-visible View (retrieve): history + budget + query_card + plan + claims.
        // Order fixed for prefix-cache; system stays outside the View.
        let visible = super::super::model_visible::build_retrieve_model_visible(
            super::super::model_visible::RetrieveViewInputs {
                durable_messages: &state.messages,
                budget_hint: &assembled.budget_hint,
                query_card: state.query_card.as_ref(),
                claim_notes: &state.evidence.claim_notes,
                ews_items: state.ews.active(),
                keep_recent: super::super::context_visibility::HISTORY_FULL_RETRIEVAL_ROUNDS,
                char_budget: super::super::context_visibility::WORKING_SET_CHAR_BUDGET,
            },
        );
        round_messages.extend(visible);
        // B5: LLM boundary transform (default: identity).
        let mut round_messages = hooks.convert_to_llm(&round_messages);

        let temperature = mode.temperature.unwrap_or(0.7);

        // Live-stream retrieve when the client asked for stream:
        // - no tools this round, or
        // - pure chat / prose_only: avoid complete_with_tools freeze (A4).
        // Retrieval drafts stay on the process channel. Pure chat has an
        // explicit answer channel; only its framed prose can paint the bubble.
        let prefer_prose_stream = request.stream
            && (assembled.tools.is_empty()
                || mode.id == "chat"
                || mode.synthesis_output.contract
                    == super::super::config::AnswerContractKind::ProseOnly);
        let retrieve_llm = self.llm_for_retrieve(mode);
        let mut channel_repair_used = false;
        let llm_response = loop {
            let llm_response = if prefer_prose_stream {
                self.call_retrieve_llm_stream(
                    retrieve_llm,
                    &round_messages,
                    temperature,
                    request,
                    state,
                    sink,
                    mode.id == "chat",
                )
                .await?
            } else {
                retrieve_llm
                    .complete_with_tools(&round_messages, &assembled.tools, Some(temperature))
                    .await
                    .map_err(|e| AppError::internal(format!("llm completion failed: {e}")))?
            };
            total_usage.accumulate(&llm_response.usage);

            if mode.id == "chat" {
                let answer = super::super::chat_answer_channel::decode(&llm_response.content)
                    .map_err(AppError::internal)?;
                if answer.is_none()
                    && !super::super::skill_request::is_skill_request_message(&llm_response.content)
                    && matches!(
                        super::super::parse::parse_llm_output(&llm_response),
                        super::super::parse::LlmOutput::Content(_)
                    )
                {
                    if !channel_repair_used && !state.answer_deltas_streamed {
                        channel_repair_used = true;
                        round_messages.push(ChatMessage::assistant(llm_response.content));
                        round_messages.push(ChatMessage::user(
                            super::super::prompt_assets::chat_answer_channel_repair(),
                        ));
                        continue;
                    }
                    return Err(AppError::internal("chat_answer_channel_missing"));
                }
            }

            break llm_response;
        };
        record_reasoning(
            sink,
            &mut state.reasoning_acc,
            llm_response.reasoning_content.as_deref(),
        )
        .await;
        Ok(llm_response)
    }

    pub(super) async fn call_retrieve_llm_stream(
        &self,
        llm: &LlmClient,
        round_messages: &[ChatMessage],
        temperature: f32,
        request: &AgentRequest,
        state: &mut IterationState,
        sink: &dyn AgentEventSink,
        chat: bool,
    ) -> Result<LlmResponse, AppError> {
        use crate::events::AgentEvent;

        let cancel = request.cancellation_token.clone().unwrap_or_default();
        let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let (reasoning_tx, mut reasoning_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let stream = llm.complete_stream(
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
        let mut answer = super::super::chat_answer_channel::AnswerChannel::default();
        state.answer_deltas_streamed = false;

        let response = loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    return Err(AppError::internal("request cancelled during retrieve stream"));
                }
                Some(delta) = delta_rx.recv() => {
                    if chat {
                        let text = answer.push(&delta).map_err(AppError::internal)?;
                        if !text.is_empty() {
                            state.answer_deltas_streamed = true;
                            let _ = sink.emit(AgentEvent::MessageDelta { text }).await;
                        }
                    } else {
                        // Process panel only — retrieve drafts (codegen, scratch)
                        // must not paint the main answer bubble.
                        let _ = sink
                            .emit(AgentEvent::ReasoningSummaryDelta { text: delta })
                            .await;
                    }
                }
                Some(reasoning) = reasoning_rx.recv() => {
                        let _ = sink
                            .emit(AgentEvent::ReasoningSummaryDelta { text: reasoning })
                            .await;
                }
                result = &mut stream => {
                    break result.map_err(|e| {
                        crate::helpers::map_llm_error_to_app_error("retrieve stream failed", e)
                    })?;
                }
            }
        };

        while let Ok(delta) = delta_rx.try_recv() {
            if chat {
                let text = answer.push(&delta).map_err(AppError::internal)?;
                if !text.is_empty() {
                    state.answer_deltas_streamed = true;
                    let _ = sink.emit(AgentEvent::MessageDelta { text }).await;
                }
            } else {
                let _ = sink
                    .emit(AgentEvent::ReasoningSummaryDelta { text: delta })
                    .await;
            }
        }
        if chat {
            if let Some(full) = answer.finish().map_err(AppError::internal)? {
                let text = full[answer.emitted..].to_string();
                if !text.is_empty() {
                    state.answer_deltas_streamed = true;
                    let _ = sink.emit(AgentEvent::MessageDelta { text }).await;
                }
            }
        }
        while let Ok(reasoning) = reasoning_rx.try_recv() {
            let _ = sink
                .emit(AgentEvent::ReasoningSummaryDelta { text: reasoning })
                .await;
        }

        // Only explicitly framed chat answers own the final bubble.
        Ok(response)
    }
}
