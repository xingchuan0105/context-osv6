use super::answer_contract::{
    collect_synthesis_validation_errors, contract_violation_fallback,
    render_synthesis_prose, resolve_synthesis_answer, synthesis_contract_block,
};
use super::assembler::AssembledContext;
use super::config::{AnswerContractKind, ModeConfig};
use super::reasoning_emit;
use crate::agents::events::{AgentEvent, AgentEventSink};
use avrag_llm::{ChatMessage, LlmClient};
use common::{AppError, ToolResult};
use tokio_util::sync::CancellationToken;

pub struct SynthesisPhase;

impl SynthesisPhase {
    pub async fn run(
        &self,
        llm: &LlmClient,
        assembled: &AssembledContext,
        mode: &ModeConfig,
        messages: &[ChatMessage],
        tool_results: &[ToolResult],
        sink: &dyn AgentEventSink,
        cancel: &CancellationToken,
    ) -> Result<String, AppError> {
        if cancel.is_cancelled() {
            return Err(crate::agents::react_loop::cancellation_error());
        }

        let contract = mode.synthesis_output.contract;
        if contract == AnswerContractKind::ProseOnly {
            return self
                .run_prose_stream(llm, assembled, mode, messages, sink, cancel)
                .await;
        }

        let _ = sink
            .emit(AgentEvent::SynthesisContract {
                schema_version: match contract {
                    AnswerContractKind::InternalSearchAnswerV1 => {
                        "internal_search_answer_v1".to_string()
                    }
                    _ => "internal_answer_v1".to_string(),
                },
            })
            .await;

        let mut system_content = assembled.system_content.clone();
        system_content.push_str("\n\n");
        system_content.push_str(synthesis_contract_block(mode));

        let mut synthesis_messages = vec![ChatMessage::system(system_content.clone())];
        for msg in messages {
            if msg.role != "system" {
                synthesis_messages.push(msg.clone());
            }
        }
        append_tool_results_observation(&mut synthesis_messages, tool_results);

        let temperature = mode.temperature.unwrap_or(0.7);
        let first = llm
            .complete(&synthesis_messages, Some(temperature))
            .await
            .map_err(|e| AppError::internal(format!("synthesis complete failed: {e}")))?;
        reasoning_emit::emit_reasoning_chunks(sink, first.reasoning_content.as_deref()).await;

        let mut candidates: Vec<&str> = vec![&first.content];
        let repaired_content;

        if resolve_synthesis_answer(&[&first.content], tool_results, messages, mode).is_none() {
            if cancel.is_cancelled() {
                return Err(crate::agents::react_loop::cancellation_error());
            }

            let validation_errors = collect_synthesis_validation_errors(
                &[&first.content],
                tool_results,
                messages,
                mode,
            );
            let repair_user = if validation_errors.is_empty() {
                "Return ONLY valid JSON matching the synthesis contract. No markdown fences."
                    .to_string()
            } else {
                format!(
                    "Return ONLY valid JSON matching the synthesis contract. No markdown fences.\n\
                     Validation errors from your previous response:\n{}",
                    validation_errors.join("\n")
                )
            };
            let mut repair_messages = vec![
                ChatMessage::system(system_content.clone()),
                ChatMessage::assistant(&first.content),
            ];
            append_tool_results_observation(&mut repair_messages, tool_results);
            repair_messages.push(ChatMessage::user(&repair_user));
            let repaired = llm
                .complete(&repair_messages, Some(temperature))
                .await
                .map_err(|e| AppError::internal(format!("synthesis repair failed: {e}")))?;
            reasoning_emit::emit_reasoning_chunks(sink, repaired.reasoning_content.as_deref())
                .await;
            repaired_content = repaired.content;
            candidates.push(&repaired_content);
        }

        if let Some(answer) = resolve_synthesis_answer(&candidates, tool_results, messages, mode) {
            let prose = render_synthesis_prose(&answer);
            let _ = sink
                .emit(AgentEvent::MessageDelta {
                    text: prose.clone(),
                })
                .await;

            let usage = crate::agents::events::AgentUsage {
                provider: first.usage.provider.clone(),
                model: first.model.clone(),
                prompt_tokens: first.usage.prompt_tokens as u64,
                completion_tokens: first.usage.completion_tokens as u64,
                total_tokens: first.usage.total_tokens as u64,
            };

            let _ = sink
                .emit(AgentEvent::Done {
                    final_message: Some(prose.clone()),
                    usage: Some(usage),
                })
                .await;

            return Ok(prose);
        }

        let _ = sink
            .emit(AgentEvent::Activity {
                stage: "synthesis_contract_violation".to_string(),
                message: "Synthesis JSON contract validation failed after repair".to_string(),
            })
            .await;

        let fallback = contract_violation_fallback(&mode.id);
        let _ = sink
            .emit(AgentEvent::MessageDelta {
                text: fallback.clone(),
            })
            .await;
        let _ = sink
            .emit(AgentEvent::Done {
                final_message: Some(fallback.clone()),
                usage: None,
            })
            .await;
        Ok(fallback)
    }

    async fn run_prose_stream(
        &self,
        llm: &LlmClient,
        assembled: &AssembledContext,
        mode: &ModeConfig,
        messages: &[ChatMessage],
        sink: &dyn AgentEventSink,
        cancel: &CancellationToken,
    ) -> Result<String, AppError> {
        let system_msg = ChatMessage::system(assembled.system_content.clone());
        let mut synthesis_messages = vec![system_msg];
        for msg in messages {
            if msg.role != "system" {
                synthesis_messages.push(msg.clone());
            }
        }

        let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let (reasoning_tx, mut reasoning_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let temperature = mode.temperature.unwrap_or(0.7);
        let stream = llm.complete_stream(
            &synthesis_messages,
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

        let mut full_answer = String::new();

        let response = loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    return Err(crate::agents::react_loop::cancellation_error());
                }
                delta = delta_rx.recv() => {
                    if let Some(delta) = delta {
                        full_answer.push_str(&delta);
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
                    break result.map_err(|e| AppError::internal(format!("synthesis stream failed: {e}")))?;
                }
            }
        };

        while let Ok(delta) = delta_rx.try_recv() {
            full_answer.push_str(&delta);
            let _ = sink.emit(AgentEvent::MessageDelta { text: delta }).await;
        }
        while let Ok(reasoning) = reasoning_rx.try_recv() {
            let _ = sink
                .emit(AgentEvent::ReasoningSummaryDelta { text: reasoning })
                .await;
        }

        let usage = crate::agents::events::AgentUsage {
            provider: response.usage.provider.clone(),
            model: response.model.clone(),
            prompt_tokens: response.usage.prompt_tokens as u64,
            completion_tokens: response.usage.completion_tokens as u64,
            total_tokens: response.usage.total_tokens as u64,
        };

        let _ = sink
            .emit(AgentEvent::Done {
                final_message: Some(full_answer.clone()),
                usage: Some(usage),
            })
            .await;

        Ok(full_answer)
    }
}

fn append_tool_results_observation(out: &mut Vec<ChatMessage>, tool_results: &[ToolResult]) {
    if tool_results.is_empty() {
        return;
    }
    let text = serde_json::to_string_pretty(tool_results).unwrap_or_else(|_| "[]".to_string());
    out.push(ChatMessage::user(format!(
        "<tool_results>\n{text}\n</tool_results>"
    )));
}
