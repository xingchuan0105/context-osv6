use super::answer_contract::{
    collect_synthesis_validation_errors, contract_violation_fallback,
    extract_partial_synthesis_fallback, render_synthesis_prose, resolve_synthesis_answer,
    synthesis_contract_block,
};

const DEFAULT_SYNTHESIS_REPAIR_ROUNDS: usize = 2;
use super::assembler::AssembledContext;
use super::config::{AnswerContractKind, ModeConfig};
use super::reasoning_emit;
use crate::agents::events::{AgentEvent, AgentEventSink};
use avrag_llm::{ChatMessage, LlmClient};
use common::AppError;
use contracts::ToolResult;
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
            return Err(super::cancellation::cancellation_error());
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
            .complete_json_mode(&synthesis_messages, Some(temperature))
            .await
            .map_err(|e| AppError::internal(format!("synthesis complete failed: {e}")))?;
        reasoning_emit::emit_reasoning_chunks(sink, first.reasoning_content.as_deref()).await;

        let mut candidates: Vec<String> = vec![first.content.clone()];
        let mut repair_round = 0usize;

        loop {
            let candidate_refs: Vec<&str> = candidates.iter().map(String::as_str).collect();
            if resolve_synthesis_answer(&candidate_refs, tool_results, messages, mode).is_some() {
                break;
            }
            if repair_round >= DEFAULT_SYNTHESIS_REPAIR_ROUNDS {
                break;
            }
            if cancel.is_cancelled() {
                return Err(super::cancellation::cancellation_error());
            }

            let validation_errors =
                collect_synthesis_validation_errors(&candidate_refs, tool_results, messages, mode);
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
            let last_candidate = candidates.last().expect("candidates non-empty");
            let mut repair_messages = vec![
                ChatMessage::system(system_content.clone()),
                ChatMessage::assistant(last_candidate),
            ];
            append_tool_results_observation(&mut repair_messages, tool_results);
            repair_messages.push(ChatMessage::user(&repair_user));
            let repaired = llm
                .complete_json_mode(&repair_messages, Some(temperature))
                .await
                .map_err(|e| AppError::internal(format!("synthesis repair failed: {e}")))?;
            reasoning_emit::emit_reasoning_chunks(sink, repaired.reasoning_content.as_deref())
                .await;
            candidates.push(repaired.content);
            repair_round += 1;
        }

        let candidate_refs: Vec<&str> = candidates.iter().map(String::as_str).collect();
        if let Some(answer) = resolve_synthesis_answer(&candidate_refs, tool_results, messages, mode) {
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
                cached_tokens: 0,
            };

            let _ = sink
                .emit(AgentEvent::Done {
                    final_message: Some(prose.clone()),
                    usage: Some(usage),
                })
                .await;

            return Ok(prose);
        }

        // Safety net: when the model failed to emit parseable synthesis JSON
        // (a frequent failure mode is emitting a `<code>` retrieval block on the
        // synthesis turn instead of JSON), but its reasoning articulated a
        // refusal, lift that refusal as the final answer. This preserves the
        // model's own grounded Chinese refusal instead of leaking the uninformative
        // English contract-violation fallback. If the query should have been
        // answered, the evaluator will still flag the refusal as REFUSAL_WRONG.
        if let Some(refusal) = extract_refusal_sentence(first.reasoning_content.as_deref()) {
            let _ = sink
                .emit(AgentEvent::Activity {
                    stage: "synthesis_refusal_lift".to_string(),
                    message: "Lifted refusal from reasoning after contract violation".to_string(),
                })
                .await;
            let _ = sink
                .emit(AgentEvent::MessageDelta {
                    text: refusal.clone(),
                })
                .await;
            let _ = sink
                .emit(AgentEvent::Done {
                    final_message: Some(refusal.clone()),
                    usage: None,
                })
                .await;
            return Ok(refusal);
        }

        let candidate_refs: Vec<&str> = candidates.iter().map(String::as_str).collect();
        if let Some(partial) =
            extract_partial_synthesis_fallback(&candidate_refs, tool_results, messages, mode)
        {
            let _ = sink
                .emit(AgentEvent::Activity {
                    stage: "synthesis_partial_fallback".to_string(),
                    message: "Salvaged partial answer after contract validation failed".to_string(),
                })
                .await;
            let _ = sink
                .emit(AgentEvent::MessageDelta {
                    text: partial.clone(),
                })
                .await;
            let _ = sink
                .emit(AgentEvent::Done {
                    final_message: Some(partial.clone()),
                    usage: None,
                })
                .await;
            return Ok(partial);
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
                    return Err(super::cancellation::cancellation_error());
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
            cached_tokens: 0,
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

/// Refusal cue words for the synthesis safety-net. Mirrors the evaluator's
/// `DEFAULT_REFUSAL_KEYWORDS` so a lifted refusal is recognized downstream.
const SYNTHESIS_REFUSAL_CUES: &[&str] = &[
    "未找到",
    "未提及",
    "未提到",
    "没有提及",
    "没有找到",
    "没有提到",
    "未在文档中找到",
    "文档中未",
    "资料中未",
    "不在文档",
    "不在资料",
    "未提供",
    "无法确认",
    "无法确定",
    "无法回答",
    "暂无相关",
    "无相关内容",
];

/// Extract a single refusal sentence from the model's synthesis reasoning.
///
/// When the model fails to emit parseable synthesis JSON (e.g. it emits a
/// `<code>` retrieval block instead) but its reasoning articulated a refusal,
/// this pulls the most specific refusal sentence out so it can be surfaced to
/// the user instead of the English contract-violation fallback. Returns the
/// last sentence (most conclusive) containing a refusal cue.
fn extract_refusal_sentence(reasoning: Option<&str>) -> Option<String> {
    let reasoning = reasoning?;
    let sentences: Vec<&str> = reasoning
        .split(|c: char| matches!(c, '。' | '；' | ';' | '.' | '!' | '！' | '？' | '?' | '\n'))
        .collect();
    for s in sentences.into_iter().rev() {
        let trimmed = s.trim().trim_start_matches([' ', ',']).trim();
        if trimmed.is_empty() || trimmed.chars().count() < 4 {
            continue;
        }
        if SYNTHESIS_REFUSAL_CUES.iter().any(|c| trimmed.contains(c)) {
            return Some(format!("{}。", trimmed.trim_end_matches('。')));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::extract_refusal_sentence;

    #[test]
    fn lifts_refusal_sentence_from_reasoning() {
        let reasoning = "我们进行了几轮检索，没有任何一个chunk提及Y冷冻设备公司速冻机产品的保修期限。\
                         由于没有找到足够证据，我应该如实说明。";
        let lifted = extract_refusal_sentence(Some(reasoning)).unwrap();
        assert!(lifted.contains("没有找到足够证据") || lifted.contains("提及"));
        assert!(lifted.ends_with('。'));
    }

    #[test]
    fn returns_none_when_no_refusal_cue() {
        let reasoning = "文档指出公司于2019年在大连建厂，营收550万元。这是答案。";
        assert!(extract_refusal_sentence(Some(reasoning)).is_none());
    }

    #[test]
    fn returns_none_for_empty_reasoning() {
        assert!(extract_refusal_sentence(None).is_none());
        assert!(extract_refusal_sentence(Some("")).is_none());
    }
}
