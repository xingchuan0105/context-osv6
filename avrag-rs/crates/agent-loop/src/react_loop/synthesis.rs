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
use avrag_llm::{ChatMessage, LlmClient, LlmResponse, LlmUsage};
use common::AppError;
use contracts::ToolResult;
use tokio_util::sync::CancellationToken;

pub struct SynthesisPhase;

impl SynthesisPhase {
    /// `deliver_to_user`: when false, accumulate prose without MessageDelta/Done
    /// (short Judge path — host delivers once after pass/ceiling).
    pub async fn run(
        &self,
        llm: &LlmClient,
        assembled: &AssembledContext,
        mode: &ModeConfig,
        messages: &[ChatMessage],
        tool_results: &[ToolResult],
        sink: &dyn AgentEventSink,
        cancel: &CancellationToken,
        deliver_to_user: bool,
    ) -> Result<(String, LlmUsage), AppError> {
        if cancel.is_cancelled() {
            return Err(super::cancellation::cancellation_error());
        }

        let contract = mode.synthesis_output.contract;
        if contract == AnswerContractKind::ProseOnly {
            return self
                .run_prose_stream(
                    llm,
                    assembled,
                    mode,
                    messages,
                    tool_results,
                    sink,
                    cancel,
                    deliver_to_user,
                )
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
            .map_err(|e| {
                crate::helpers::map_llm_error_to_app_error("synthesis complete failed", e)
            })?;
        reasoning_emit::emit_reasoning_chunks(sink, first.reasoning_content.as_deref()).await;
        let mut usage = first.usage.clone();

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
            usage.accumulate(&repaired.usage);
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
            if deliver_to_user {
                emit_prose_delivery(
                    sink,
                    &prose,
                    Some(crate::events::AgentUsage {
                        provider: first.usage.provider.clone(),
                        model: first.model.clone(),
                        prompt_tokens: first.usage.prompt_tokens as u64,
                        completion_tokens: first.usage.completion_tokens as u64,
                        total_tokens: first.usage.total_tokens as u64,
                        cached_tokens: 0,
                    }),
                )
                .await;
            } else {
                let _ = sink
                    .emit(AgentEvent::Activity {
                        stage: "synthesis_held_for_judge".to_string(),
                        message: "synthesis draft ready; short Judge pending".to_string(),
                        detail: None,
                        counts: Default::default(),
                        sources_preview: Vec::new(),
                    })
                    .await;
            }

            return Ok((prose, usage));
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
            if deliver_to_user {
                emit_prose_delivery(sink, &refusal, None).await;
            }
            return Ok((refusal, usage));
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
            if deliver_to_user {
                emit_prose_delivery(sink, &partial, None).await;
            }
            return Ok((partial, usage));
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
            if deliver_to_user {
                emit_prose_delivery(sink, &unwrapped, None).await;
            }
            return Ok((unwrapped, usage));
        }

        let fallback = contract_violation_fallback(&mode.id);
        if deliver_to_user {
            emit_prose_delivery(sink, &fallback, None).await;
        }
        Ok((fallback, usage))
    }

    async fn run_prose_stream(
        &self,
        llm: &LlmClient,
        assembled: &AssembledContext,
        mode: &ModeConfig,
        messages: &[ChatMessage],
        tool_results: &[ToolResult],
        sink: &dyn AgentEventSink,
        cancel: &CancellationToken,
        deliver_to_user: bool,
    ) -> Result<(String, LlmUsage), AppError> {
        let system_msg = ChatMessage::system(assembled.system_content.clone());
        let mut synthesis_messages = vec![system_msg];
        for msg in messages {
            if msg.role != "system" {
                synthesis_messages.push(msg.clone());
            }
        }
        let temperature = mode.temperature.unwrap_or(0.7);
        // L3 salvage decision: whether the retrieval loop actually returned
        // evidence. Reuse the structural observer so the rerender / degraded
        // fork is grounded in the same fact the L2 gate used.
        let has_evidence =
            super::exit_policy::has_retrieval_observation(messages, tool_results, mode);

        let (mut full_answer, response) = stream_prose_to_sink(
            llm,
            &synthesis_messages,
            temperature,
            sink,
            cancel,
            deliver_to_user,
        )
        .await?;
        let mut usage = response.usage.clone();

        // prose_only contract check (host structural): a code-only answer
        // means the retrieve-phase "output one <code> block" framing leaked
        // into the closing turn (observed on budget-exhausted tails). The
        // same class covers a final answer that pastes a host observation
        // shell (`<retrieval_summary>`, `<loop_budget>`, …) — the model
        // reproduced host-emitted format instead of writing grounded prose.
        // Repair → optional rerender; third failure → disaster prose (no 4th
        // full synthesis by default). Never surface protocol shells as the answer.
        if let Some(violation) = super::answer_contract::check_final_answer(&full_answer) {
            let rule_id = violation.rule_id;
            let mut repair_counts = std::collections::BTreeMap::new();
            repair_counts.insert(format!("final_check:{rule_id}:repair"), 1usize);
            let _ = sink
                .emit(AgentEvent::Activity {
                    stage: format!("final_check:{rule_id}:repair"),
                    message: "progress.final_check_repair".to_string(),
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
            let (repaired, repaired_resp) = stream_prose_to_sink(
                llm,
                &repair_messages,
                temperature,
                sink,
                cancel,
                deliver_to_user,
            )
            .await?;
            usage.accumulate(&repaired_resp.usage);
            if let Some(violation) = super::answer_contract::check_final_answer(&repaired) {
                // L3 salvage (2026-08-03): distinguish "repair failed because
                // the model mangled the form again" from "there was never any
                // evidence to write from". With evidence, replay the pooled
                // tool results one more time and re-ask; without evidence,
                // degrade honestly instead of pretending material was found.
                let rule_id = violation.rule_id;
                if has_evidence {
                    let mut rerender_counts = std::collections::BTreeMap::new();
                    rerender_counts.insert(format!("final_check:{rule_id}:rerender"), 1usize);
                    let _ = sink
                        .emit(AgentEvent::Activity {
                            stage: format!("final_check:{rule_id}:rerender"),
                            message: "progress.final_check_rerender".to_string(),
                            detail: Some(violation.matched.to_string()),
                            counts: rerender_counts,
                            sources_preview: Vec::new(),
                        })
                        .await;
                    let mut rerender_messages = synthesis_messages.clone();
                    rerender_messages.push(ChatMessage::assistant(repaired.as_str()));
                    append_tool_results_observation(&mut rerender_messages, tool_results);
                    rerender_messages.push(ChatMessage::user(
                        super::prompt_assets::synthesis_rerender_nudge(),
                    ));
                    let (rerendered, rr) = stream_prose_to_sink(
                        llm,
                        &rerender_messages,
                        temperature,
                        sink,
                        cancel,
                        deliver_to_user,
                    )
                    .await?;
                    usage.accumulate(&rr.usage);
                    if let Some(violation) = super::answer_contract::check_final_answer(&rerendered)
                    {
                        let rule_id = violation.rule_id;
                        let mut violation_counts = std::collections::BTreeMap::new();
                        violation_counts.insert(format!("final_check:{rule_id}:fallback"), 1usize);
                        let _ = sink
                            .emit(AgentEvent::Activity {
                                stage: format!("final_check:{rule_id}:fallback"),
                                message: "progress.final_check_fallback".to_string(),
                                detail: Some(violation.matched.to_string()),
                                counts: violation_counts,
                                sources_preview: Vec::new(),
                            })
                            .await;
                        // §17.3: after draft+repair+rerender, disaster prose (no 4th LLM).
                        full_answer =
                            super::prompt_assets::disaster_format_exhausted().to_string();
                        if deliver_to_user {
                            let _ = sink
                                .emit(AgentEvent::MessageDelta {
                                    text: full_answer.clone(),
                                })
                                .await;
                        }
                    } else {
                        full_answer = rerendered;
                    }
                } else {
                    let mut degraded_counts = std::collections::BTreeMap::new();
                    degraded_counts.insert(format!("final_check:{rule_id}:degraded"), 1usize);
                    let _ = sink
                        .emit(AgentEvent::Activity {
                            stage: format!("final_check:{rule_id}:degraded"),
                            message: "progress.final_check_degraded".to_string(),
                            detail: Some(violation.matched.to_string()),
                            counts: degraded_counts,
                            sources_preview: Vec::new(),
                        })
                        .await;
                    full_answer =
                        super::prompt_assets::disaster_no_evidence_answer(&mode.id).to_string();
                    if deliver_to_user {
                        let _ = sink
                            .emit(AgentEvent::MessageDelta {
                                text: full_answer.clone(),
                            })
                            .await;
                    }
                }
            } else {
                full_answer = repaired;
            }
        }

        if deliver_to_user {
            let done_usage = crate::events::AgentUsage {
                provider: usage.provider.clone(),
                model: response.model.clone(),
                prompt_tokens: usage.prompt_tokens as u64,
                completion_tokens: usage.completion_tokens as u64,
                total_tokens: usage.total_tokens as u64,
                cached_tokens: usage.cached_tokens as u64,
            };
            let _ = sink
                .emit(AgentEvent::Done {
                    final_message: Some(full_answer.clone()),
                    usage: Some(done_usage),
                })
                .await;
        } else {
            let _ = sink
                .emit(AgentEvent::Activity {
                    stage: "synthesis_held_for_verify".to_string(),
                    message: "progress.synthesis_held_for_verify".to_string(),
                    detail: None,
                    counts: Default::default(),
                    sources_preview: Vec::new(),
                })
                .await;
        }

        Ok((full_answer, usage))
    }
}

/// Emit final prose to the user bubble (chunked deltas + Done). Used when
/// short Judge deferred delivery during synthesis.
pub async fn emit_prose_delivery(
    sink: &dyn AgentEventSink,
    prose: &str,
    usage: Option<crate::events::AgentUsage>,
) {
    const CHUNK: usize = 24;
    let chars: Vec<char> = prose.chars().collect();
    for piece in chars.chunks(CHUNK) {
        let text: String = piece.iter().collect();
        let _ = sink.emit(AgentEvent::MessageDelta { text }).await;
    }
    let _ = sink
        .emit(AgentEvent::Done {
            final_message: Some(prose.to_string()),
            usage,
        })
        .await;
}

/// Max non-stream recovery attempts after a failed stream (stream + non-stream×N).
const SYNTHESIS_NONSTREAM_FALLBACK_ATTEMPTS: u8 = 2;

/// One prose completion with **hybrid** delivery (option C):
///
/// 1. Stream once: **hold** user-bubble deltas until the first **valid prose**
///    prefix, then **live** `MessageDelta` for the rest (reasoning stays real-time).
/// 2. On retryable stream failure → up to **two** non-stream `complete` fallbacks
///    (1s backoff). If live paint already started, caller repair/disaster still
///    owns the final bubble text.
/// 3. Only the winning attempt's text is returned; if live emit never started,
///    flush the full prose as chunked deltas.
///
/// Shared by the first prose pass and the code-only repair / rerender rounds.
async fn stream_prose_to_sink(
    llm: &LlmClient,
    messages: &[ChatMessage],
    temperature: f32,
    sink: &dyn AgentEventSink,
    cancel: &CancellationToken,
    emit_answer_deltas: bool,
) -> Result<(String, LlmResponse), AppError> {
    // If hybrid already opened the user bubble, never re-flush full prose on
    // non-stream recovery (would double-paint). Done still carries final text.
    let already_live = match stream_prose_hybrid(
        llm,
        messages,
        temperature,
        sink,
        cancel,
        emit_answer_deltas,
    )
    .await
    {
        Ok((text, response, live)) => {
            telemetry::prometheus::observe_synthesis_stream_outcome("ok");
            if emit_answer_deltas && !live {
                flush_answer_deltas(sink, &text).await;
            }
            return Ok((text, response));
        }
        Err((e, live))
            if crate::helpers::is_cancellation_error(&e) || cancel.is_cancelled() =>
        {
            let _ = live;
            return Err(super::cancellation::cancellation_error());
        }
        Err((e, live)) if !crate::helpers::is_retryable_upstream_error(&e) => {
            let _ = live;
            telemetry::prometheus::observe_synthesis_stream_outcome("exhausted");
            return Err(crate::helpers::map_llm_error_to_app_error(
                "synthesis stream failed",
                e,
            ));
        }
        Err((e, live)) => {
            telemetry::prometheus::observe_synthesis_stream_outcome("stream_fail");
            tracing::warn!(
                error = %e,
                already_live = live,
                "synthesis stream failed (retryable); trying non-stream fallbacks (max {SYNTHESIS_NONSTREAM_FALLBACK_ATTEMPTS})"
            );
            let mut counts = std::collections::BTreeMap::new();
            counts.insert("synthesis_stream_fail".to_string(), 1usize);
            let _ = sink
                .emit(AgentEvent::Activity {
                    stage: "synthesis_stream_fail".to_string(),
                    message: format!(
                        "synthesis stream interrupted; up to {SYNTHESIS_NONSTREAM_FALLBACK_ATTEMPTS} non-stream fallback(s) follow"
                    ),
                    detail: Some(format!("{e:#}")),
                    counts,
                    sources_preview: Vec::new(),
                })
                .await;
            live
        }
    };

    let mut last_err: Option<anyhow::Error> = None;
    for attempt in 1..=SYNTHESIS_NONSTREAM_FALLBACK_ATTEMPTS {
        if cancel.is_cancelled() {
            return Err(super::cancellation::cancellation_error());
        }
        if attempt > 1 {
            // Brief backoff between non-stream retries (1s).
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            if cancel.is_cancelled() {
                return Err(super::cancellation::cancellation_error());
            }
        }

        match llm.complete(messages, Some(temperature)).await {
            Ok(response) => {
                telemetry::prometheus::observe_synthesis_stream_outcome("nonstream_fallback_ok");
                let mut counts = std::collections::BTreeMap::new();
                counts.insert("synthesis_nonstream_fallback".to_string(), 1usize);
                counts.insert(
                    format!("synthesis_nonstream_fallback_attempt_{attempt}"),
                    1usize,
                );
                let _ = sink
                    .emit(AgentEvent::Activity {
                        stage: "synthesis_nonstream_fallback".to_string(),
                        message: format!(
                            "synthesis recovered via non-stream completion (attempt {attempt}/{SYNTHESIS_NONSTREAM_FALLBACK_ATTEMPTS})"
                        ),
                        detail: None,
                        counts,
                        sources_preview: Vec::new(),
                    })
                    .await;
                let text = response.content.clone();
                // Skip paint if hybrid already opened the bubble.
                if emit_answer_deltas && !already_live {
                    flush_answer_deltas(sink, &text).await;
                }
                return Ok((text, response));
            }
            Err(e) => {
                let retryable = crate::helpers::is_retryable_upstream_error(&e);
                tracing::warn!(
                    attempt,
                    max = SYNTHESIS_NONSTREAM_FALLBACK_ATTEMPTS,
                    retryable,
                    error = %e,
                    "synthesis non-stream fallback failed"
                );
                last_err = Some(e);
                if !retryable {
                    break;
                }
            }
        }
    }

    telemetry::prometheus::observe_synthesis_stream_outcome("exhausted");
    let detail = last_err
        .as_ref()
        .map(|e| format!("{e:#}"))
        .unwrap_or_else(|| "unknown".to_string());
    let mut counts = std::collections::BTreeMap::new();
    counts.insert("synthesis_upstream_exhausted".to_string(), 1usize);
    let _ = sink
        .emit(AgentEvent::Activity {
            stage: "synthesis_upstream_exhausted".to_string(),
            message: format!(
                "synthesis stream + {SYNTHESIS_NONSTREAM_FALLBACK_ATTEMPTS} non-stream fallback(s) all failed"
            ),
            detail: Some(detail.clone()),
            counts,
            sources_preview: Vec::new(),
        })
        .await;
    Err(crate::helpers::map_llm_error_to_app_error(
        "synthesis complete failed after stream + non-stream fallbacks",
        last_err.unwrap_or_else(|| anyhow::anyhow!("synthesis fallback exhausted")),
    ))
}

/// Hybrid stream: hold user-bubble paint until first valid prose prefix, then
/// live `MessageDelta`. Reasoning is always real-time on the process panel.
///
/// Ok: `(text, response, already_live)` — when `already_live` the caller must
/// not re-flush the full answer.
/// Err: `(error, already_live)` so recovery can skip double-paint.
async fn stream_prose_hybrid(
    llm: &LlmClient,
    messages: &[ChatMessage],
    temperature: f32,
    sink: &dyn AgentEventSink,
    cancel: &CancellationToken,
    emit_answer_deltas: bool,
) -> Result<(String, LlmResponse, bool), (anyhow::Error, bool)> {
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
    // Bytes already painted to the user bubble (prefix of `full_answer`).
    let mut painted_len = 0usize;
    let mut live = false;

    let response = loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                return Err((
                    anyhow::anyhow!("request cancelled during synthesis stream"),
                    live,
                ));
            }
            delta = delta_rx.recv() => {
                if let Some(delta) = delta {
                    full_answer.push_str(&delta);
                    if emit_answer_deltas {
                        if !live {
                            if is_first_valid_prose_prefix(&full_answer) {
                                // Open bubble with the whole buffered prefix (first valid prose).
                                live = true;
                                let _ = sink
                                    .emit(AgentEvent::MessageDelta {
                                        text: full_answer.clone(),
                                    })
                                    .await;
                                painted_len = full_answer.len();
                            }
                        } else if full_answer.len() > painted_len {
                            let tail = full_answer[painted_len..].to_string();
                            painted_len = full_answer.len();
                            let _ = sink
                                .emit(AgentEvent::MessageDelta { text: tail })
                                .await;
                        }
                    }
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
                match result {
                    Ok(resp) => break resp,
                    Err(e) => return Err((e, live)),
                }
            }
        }
    };

    while let Ok(delta) = delta_rx.try_recv() {
        full_answer.push_str(&delta);
        if emit_answer_deltas && live && full_answer.len() > painted_len {
            let tail = full_answer[painted_len..].to_string();
            painted_len = full_answer.len();
            let _ = sink
                .emit(AgentEvent::MessageDelta { text: tail })
                .await;
        }
    }
    while let Ok(reasoning) = reasoning_rx.try_recv() {
        let _ = sink
            .emit(AgentEvent::ReasoningSummaryDelta { text: reasoning })
            .await;
    }

    // Prefer stream-assembled text; fall back to response.content if deltas were empty.
    if full_answer.is_empty() && !response.content.is_empty() {
        full_answer = response.content.clone();
    }

    // Stream ended still holding a non-empty prefix (e.g. only `{` JSON start that
    // never became "valid prose" mid-stream) — not live yet; caller may flush.
    Ok((full_answer, response, live))
}

/// First user-bubble paint gate: non-empty, not host/protocol shells, not raw JSON.
fn is_first_valid_prose_prefix(s: &str) -> bool {
    let t = s.trim_start();
    if t.is_empty() {
        return false;
    }
    // Hold host observation / tool shells until more arrives.
    if t.starts_with('<') {
        return false;
    }
    // Hold raw JSON envelopes until stream end (caller unwrap / contract check).
    if t.starts_with('{') || t.starts_with('[') {
        return false;
    }
    true
}

async fn flush_answer_deltas(sink: &dyn AgentEventSink, prose: &str) {
    const CHUNK: usize = 24;
    let chars: Vec<char> = prose.chars().collect();
    for piece in chars.chunks(CHUNK) {
        let text: String = piece.iter().collect();
        let _ = sink.emit(AgentEvent::MessageDelta { text }).await;
    }
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
    use super::{
        extract_refusal_sentence, is_first_valid_prose_prefix, trim_tool_results_for_synthesis,
    };
    use contracts::ToolResult;

    #[test]
    fn first_valid_prose_prefix_holds_empty_json_and_shells() {
        assert!(!is_first_valid_prose_prefix(""));
        assert!(!is_first_valid_prose_prefix("   "));
        assert!(!is_first_valid_prose_prefix("{"));
        assert!(!is_first_valid_prose_prefix("  {\"answer\":"));
        assert!(!is_first_valid_prose_prefix("[1,2]"));
        assert!(!is_first_valid_prose_prefix("<retrieval_summary>"));
        assert!(is_first_valid_prose_prefix("主要观点如下"));
        assert!(is_first_valid_prose_prefix("  Hello"));
    }

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
