use super::answer_contract::{
    contract_violation_fallback, extract_partial_synthesis_fallback, render_synthesis_prose,
    resolve_synthesis_answer, synthesis_contract_block, unwrap_synthesis_json_envelope,
};

/// Prefer LLM self-format; at most one repair if the envelope cannot be parsed at all.
const DEFAULT_SYNTHESIS_REPAIR_ROUNDS: usize = 1;

/// Never show a raw synthesis JSON envelope in the product UI.
fn ensure_user_facing_prose(text: String) -> String {
    super::answer_contract::ensure_user_visible_answer_text(&text)
}
use super::assembler::AssembledContext;
use super::config::{AnswerContractKind, ModeConfig};
use super::reasoning_emit;
use crate::events::{AgentEvent, AgentEventSink};
use avrag_llm::{ChatMessage, LlmClient, LlmResponse};
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
                    AnswerContractKind::InternalAnswerUnifiedV1
                    | AnswerContractKind::InternalHybridAnswerV1 => {
                        "internal_answer_unified_v1".to_string()
                    }
                    AnswerContractKind::InternalAnswerV1 | AnswerContractKind::ProseOnly => {
                        "internal_answer_v1".to_string()
                    }
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

            // Thin repair: only ask for a parseable envelope; do not nitpick cite hygiene.
            // Body: prompts/loop/synthesis-repair.nudge.md
            let repair_user = super::prompt_assets::synthesis_repair_nudge();
            let _ = (tool_results, messages);
            let last_candidate = candidates.last().expect("candidates non-empty");
            let mut repair_messages = vec![
                ChatMessage::system(system_content.clone()),
                ChatMessage::assistant(last_candidate),
            ];
            append_tool_results_observation(&mut repair_messages, tool_results);
            repair_messages.push(ChatMessage::user(repair_user));
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
        if let Some(answer) =
            resolve_synthesis_answer(&candidate_refs, tool_results, messages, mode)
        {
            let prose = ensure_user_facing_prose(render_synthesis_prose(&answer));
            crate::progress::emit_work_fact(sink, crate::progress::WorkFact::compose_answer())
                .await;
            // P0 "true stream" for JSON synthesis: chunk prose into MessageDelta
            // (generation was complete_json; streaming the validated answer still
            // gives progressive UI without breaking the answer contract).
            const CHUNK: usize = 24;
            let chars: Vec<char> = prose.chars().collect();
            for piece in chars.chunks(CHUNK) {
                let text: String = piece.iter().collect();
                let _ = sink.emit(AgentEvent::MessageDelta { text }).await;
            }

            let usage = crate::events::AgentUsage {
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
                    detail: None,
                    counts: Default::default(),
                    sources_preview: Vec::new(),
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
            let partial = ensure_user_facing_prose(partial);
            let _ = sink
                .emit(AgentEvent::Activity {
                    stage: "synthesis_partial_fallback".to_string(),
                    message: "Salvaged partial answer after contract validation failed".to_string(),
                    detail: None,
                    counts: Default::default(),
                    sources_preview: Vec::new(),
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
                detail: None,
                counts: Default::default(),
                sources_preview: Vec::new(),
            })
            .await;

        // Last resort: never surface a raw synthesis JSON envelope to the user.
        if let Some(unwrapped) = candidates
            .iter()
            .rev()
            .find_map(|c| unwrap_synthesis_json_envelope(c))
        {
            let unwrapped = ensure_user_facing_prose(unwrapped);
            let _ = sink
                .emit(AgentEvent::Activity {
                    stage: "synthesis_json_unwrap".to_string(),
                    message: "Unwrapped answer_text from synthesis JSON after validation failure"
                        .to_string(),
                    detail: None,
                    counts: Default::default(),
                    sources_preview: Vec::new(),
                })
                .await;
            let _ = sink
                .emit(AgentEvent::MessageDelta {
                    text: unwrapped.clone(),
                })
                .await;
            let _ = sink
                .emit(AgentEvent::Done {
                    final_message: Some(unwrapped.clone()),
                    usage: None,
                })
                .await;
            return Ok(unwrapped);
        }

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
        let temperature = mode.temperature.unwrap_or(0.7);

        let (mut full_answer, response) =
            stream_prose_to_sink(llm, &synthesis_messages, temperature, sink, cancel).await?;

        // prose_only contract check (host structural): a code-only answer
        // means the retrieve-phase "output one <code> block" framing leaked
        // into the closing turn (observed on budget-exhausted tails). The
        // same class covers a final answer that pastes a host observation
        // shell (`<retrieval_summary>`, `<loop_budget>`, …) — the model
        // reproduced host-emitted format instead of writing grounded prose.
        // One repair round; if the repair still comes back violating, use the
        // degraded fallback copy — never surface a raw code block or a host
        // observation shell as the final prose answer.
        if let Some(violation) = super::answer_contract::check_final_answer(&full_answer) {
            let rule_id = violation.rule_id;
            let mut repair_counts = std::collections::BTreeMap::new();
            repair_counts.insert(format!("final_check:{rule_id}:repair"), 1usize);
            let _ = sink
                .emit(AgentEvent::Activity {
                    stage: format!("final_check:{rule_id}:repair"),
                    message:
                        "final_answer quality gate fired ({rule_id}); one repair round follows"
                            .to_string(),
                    detail: Some(violation.matched.to_string()),
                    counts: repair_counts,
                    sources_preview: Vec::new(),
                })
                .await;
            let mut repair_messages = synthesis_messages.clone();
            repair_messages.push(ChatMessage::assistant(full_answer.as_str()));
            repair_messages.push(ChatMessage::user(
                super::prompt_assets::synthesis_prose_repair_nudge(violation.feedback_hint),
            ));
            let (repaired, _) =
                stream_prose_to_sink(llm, &repair_messages, temperature, sink, cancel).await?;
            if let Some(violation) = super::answer_contract::check_final_answer(&repaired) {
                let rule_id = violation.rule_id;
                let mut violation_counts = std::collections::BTreeMap::new();
                violation_counts.insert(format!("final_check:{rule_id}:fallback"), 1usize);
                let _ = sink
                    .emit(AgentEvent::Activity {
                        stage: format!("final_check:{rule_id}:fallback"),
                        message:
                            "final_answer quality gate fired again after repair; contract fallback used"
                                .to_string(),
                        detail: Some(violation.matched.to_string()),
                        counts: violation_counts,
                        sources_preview: Vec::new(),
                    })
                    .await;
                full_answer = contract_violation_fallback(&mode.id);
                let _ = sink
                    .emit(AgentEvent::MessageDelta {
                        text: full_answer.clone(),
                    })
                    .await;
            } else {
                full_answer = repaired;
            }
        }

        let usage = crate::events::AgentUsage {
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

/// Stream one prose completion, forwarding content deltas and reasoning
/// summaries to the sink as they arrive; returns the accumulated prose and
/// the final response (usage/model). Shared by the first prose pass and the
/// code-only repair round.
async fn stream_prose_to_sink(
    llm: &LlmClient,
    messages: &[ChatMessage],
    temperature: f32,
    sink: &dyn AgentEventSink,
    cancel: &CancellationToken,
) -> Result<(String, LlmResponse), AppError> {
    let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let (reasoning_tx, mut reasoning_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let stream = llm.complete_stream(
        messages,
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

    Ok((full_answer, response))
}

fn append_tool_results_observation(out: &mut Vec<ChatMessage>, tool_results: &[ToolResult]) {
    if tool_results.is_empty() {
        return;
    }
    let text = serde_json::to_string_pretty(&trim_tool_results_for_synthesis(tool_results))
        .unwrap_or_else(|_| "[]".to_string());
    out.push(ChatMessage::user(format!(
        "<tool_results>\n{text}\n</tool_results>"
    )));
}

/// Total char budget for the synthesis-time `<tool_results>` re-play.
/// Larger than the per-message budget so the final evidence set still fits,
/// while remaining far below the mode token budget (pro ~28k tokens).
const SYNTHESIS_TOOL_RESULTS_MAX_CHARS: usize = 48_000;

/// Deduplicate identical `(tool, data)` results and keep the most recent
/// entries within a total char budget. Mirrors the Reasonix `snip` tier:
/// stale/duplicate tool results are dropped before the final synthesis so the
/// model sees a bounded evidence set instead of the full accumulated history
/// (which can reach ~1.5MB in long RAG sessions).
fn trim_tool_results_for_synthesis(tool_results: &[ToolResult]) -> Vec<serde_json::Value> {
    let mut seen = std::collections::HashSet::new();
    let mut kept = Vec::new();
    let mut used = 0usize;
    for result in tool_results.iter().rev() {
        let data_json = serde_json::to_string(&result.data).unwrap_or_default();
        let key = format!("{}:{data_json}", result.tool);
        if !seen.insert(key) {
            continue;
        }
        let data = result.data.as_ref().map(|d| {
            super::message_format::trim_json_for_context(
                d,
                SYNTHESIS_TOOL_RESULTS_MAX_CHARS
                    .saturating_sub(used)
                    .max(512),
            )
        });
        let entry = serde_json::json!({
            "tool": result.tool,
            "status": result.status,
            "data": data,
        });
        let len = serde_json::to_string(&entry).map(|s| s.len()).unwrap_or(0);
        if used + len > SYNTHESIS_TOOL_RESULTS_MAX_CHARS {
            if kept.is_empty() {
                // Never return an empty evidence set: the newest result (already
                // budget-trimmed) is better than dropping everything.
                kept.push(entry);
            }
            break;
        }
        used += len;
        kept.push(entry);
    }
    kept.reverse();
    kept
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
    use super::{extract_refusal_sentence, trim_tool_results_for_synthesis};
    use contracts::ToolResult;

    fn tool_result(tool: &str, data: serde_json::Value) -> ToolResult {
        ToolResult {
            tool: tool.to_string(),
            version: "1.0".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(data),
            trace: None,
        }
    }

    #[test]
    fn lifts_refusal_sentence_from_reasoning() {
        // Synthetic prose only — no realistic-corpus entities.
        let reasoning = "我们进行了几轮检索，没有任何一个chunk提及目标实体的该项属性。\
                         由于没有找到足够证据，我应该如实说明。";
        let lifted = extract_refusal_sentence(Some(reasoning)).unwrap();
        assert!(lifted.contains("没有找到足够证据") || lifted.contains("提及"));
        assert!(lifted.ends_with('。'));
    }

    #[test]
    fn returns_none_when_no_refusal_cue() {
        let reasoning = "文档指出组件于2020年发布，版本号为3。这是答案。";
        assert!(extract_refusal_sentence(Some(reasoning)).is_none());
    }

    #[test]
    fn returns_none_for_empty_reasoning() {
        assert!(extract_refusal_sentence(None).is_none());
        assert!(extract_refusal_sentence(Some("")).is_none());
    }

    #[test]
    fn synthesis_replay_deduplicates_identical_tool_results() {
        let results = vec![
            tool_result(
                "dense_retrieval",
                serde_json::json!({"query": "q", "chunks": [1, 2]}),
            ),
            tool_result(
                "dense_retrieval",
                serde_json::json!({"query": "q", "chunks": [1, 2]}),
            ),
            tool_result("web_search", serde_json::json!({"query": "w"})),
        ];
        let trimmed = trim_tool_results_for_synthesis(&results);
        assert_eq!(trimmed.len(), 2, "duplicate should be dropped: {trimmed:?}");
    }

    #[test]
    fn synthesis_replay_keeps_most_recent_within_budget() {
        let big_payload = serde_json::json!({"text": "x".repeat(40_000)});
        let results = vec![
            tool_result(
                "dense_retrieval",
                serde_json::json!({"query": "old", "data": "y".repeat(40_000)}),
            ),
            tool_result("dense_retrieval", big_payload.clone()),
        ];
        let trimmed = trim_tool_results_for_synthesis(&results);
        // Newest result is kept (reversed order); total stays under budget.
        assert_eq!(
            trimmed.len(),
            1,
            "expected one result within budget: {trimmed:?}"
        );
        assert_eq!(trimmed[0]["data"]["text"], "x".repeat(40_000));
        let text = serde_json::to_string(&trimmed).unwrap();
        assert!(
            text.len() <= 48_000 + 128,
            "replay exceeds budget: {}",
            text.len()
        );
    }
}
