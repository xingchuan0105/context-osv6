use std::collections::BTreeMap;

use common::AppError;
use contracts::chat::{ChatRequest, ModeDebug};
use contracts::workspaces::ChatSession;

use crate::capabilities::CapabilitySet;
use crate::chat_streaming::STREAM_PLACEHOLDER_MESSAGE_ID;
use crate::context::ChatContext;
use crate::mode_assemble::AssembledMode;
use agent_loop::runtime::AgentRequest;

use super::pipeline::{ChatExecution, StreamConfig};

fn agent_request_with_resolved_session(
    mut agent_request: AgentRequest,
    session: &ChatSession,
) -> AgentRequest {
    if agent_request.session_id.is_none() {
        agent_request.session_id = Some(session.id.clone());
    }
    agent_request
}

/// Attach assembled mode config + capability metadata for UnifiedAgent / assembler.
pub(crate) fn inject_assembled_metadata(
    agent_request: &mut AgentRequest,
    caps: CapabilitySet,
    assembled: &AssembledMode,
) {
    agent_request.metadata.insert(
        "capabilities".to_string(),
        serde_json::to_value(caps.as_string_list()).unwrap_or_else(|_| serde_json::json!([])),
    );
    agent_request.metadata.insert(
        "system_prompt_parts".to_string(),
        serde_json::to_value(&assembled.system_prompt_parts)
            .unwrap_or_else(|_| serde_json::json!([])),
    );
    agent_request.metadata.insert(
        "assembled_mode_config".to_string(),
        serde_json::to_value(&assembled.config).unwrap_or_else(|_| serde_json::json!({})),
    );
}

fn merge_capabilities_turn_metadata(
    existing: Option<serde_json::Value>,
    caps: CapabilitySet,
) -> Option<serde_json::Value> {
    let caps_val =
        serde_json::to_value(caps.as_string_list()).unwrap_or_else(|_| serde_json::json!([]));
    match existing {
        Some(mut v) => {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("capabilities".to_string(), caps_val);
            } else {
                v = serde_json::json!({ "capabilities": caps_val });
            }
            Some(v)
        }
        None => Some(serde_json::json!({ "capabilities": caps_val })),
    }
}

/// Agent-lane modes only (chat / RAG / search). Write enters via write `PipelineLane`.
pub(crate) async fn dispatch_agent_mode(
    state: &ChatContext,
    request: &ChatRequest,
    session: &ChatSession,
    stream_config: Option<&StreamConfig>,
) -> Result<ChatExecution, AppError> {
    let caps = crate::resolve_capabilities(request.capabilities.as_deref(), &request.agent_type)?;
    let agent_type_label = caps.agent_type_label();

    // Doc-scope clarify only when RAG capability is on (not pure chat / search-only).
    if caps.rag && request.doc_scope.is_empty() {
        let message = crate::i18n::clarify::need_doc_scope(request.language.as_deref()).to_string();
        let mut execution = state
            .execute_clarify_mode_core(request, session, &message)
            .await?;
        execution.mode = agent_type_label.to_string();
        execution.response.agent_type = agent_type_label.to_string();
        execution.assistant_turn_metadata =
            merge_capabilities_turn_metadata(execution.assistant_turn_metadata, caps);
        return Ok(execution);
    }

    // A2 single agent (2026-07-30 SaC): one ReAct loop for chat / rag / search.
    // Orchestrator / worker / brief / handoff path is no longer the product entry.
    let assembled = crate::assemble_mode(caps)?;
    let kind = match (caps.rag, caps.search) {
        (true, _) => crate::agents::AgentKind::Rag, // dual also uses Rag + search metadata
        (false, true) => crate::agents::AgentKind::Search,
        (false, false) => crate::agents::AgentKind::Chat,
    };
    run_general_mode(
        state,
        request,
        session,
        stream_config,
        kind,
        agent_type_label,
        caps,
        &assembled,
    )
    .await
}

/// @deprecated name — tests may still call this; agent lane only.
#[cfg(test)]
pub(crate) async fn dispatch_mode(
    state: &ChatContext,
    request: &ChatRequest,
    session: &ChatSession,
    stream_config: Option<&StreamConfig>,
) -> Result<ChatExecution, AppError> {
    dispatch_agent_mode(state, request, session, stream_config).await
}

async fn run_general_mode(
    state: &ChatContext,
    request: &ChatRequest,
    session: &ChatSession,
    stream_config: Option<&StreamConfig>,
    kind: crate::agents::AgentKind,
    agent_type: &'static str,
    caps: CapabilitySet,
    assembled: &AssembledMode,
) -> Result<ChatExecution, AppError> {
    let Some(agent_service) = state.agent_service() else {
        return Err(AppError::internal("agent service is not configured"));
    };

    let mut agent_request = agent_request_with_resolved_session(
        state
            .build_agent_request(request, kind, Some(session.id.clone()))
            .await,
        session,
    );
    inject_assembled_metadata(&mut agent_request, caps, assembled);
    if let Some(config) = stream_config {
        agent_request.stream = true;
        agent_request.cancellation_token = Some(config.token.clone());
    }
    let emit_debug_trace = agent_request.debug;
    let mut general_debug = state.build_general_agent_debug(&agent_request);
    general_debug.insert(
        "capabilities".to_string(),
        serde_json::to_value(caps.as_string_list()).unwrap_or_else(|_| serde_json::json!([])),
    );

    if let Some(config) = stream_config {
        let sink = agent_loop::sse_sink::SseSink::new_with_agent_type(
            config.sender.clone(),
            config.request_id.clone(),
            session.id.clone(),
            STREAM_PLACEHOLDER_MESSAGE_ID,
            agent_type.to_string(),
        )
        .without_done_event()
        .with_debug_trace(emit_debug_trace);

        // Turn-start fact for pure chat (2026-07-23): the first LLM call is
        // silent otherwise — same immediate step as orchestrated turns.
        agent_loop::progress::emit_work_fact(
            &sink,
            agent_loop::progress::WorkFact::understand(&agent_request.query),
        )
        .await;

        let agent_result = agent_service.run(agent_request, &sink).await?;
        crate::emit_buffered_agent_answer_if_needed(&sink, &agent_result.answer).await;

        if let Some(usage) = agent_result.usage.as_ref() {
            general_debug.insert(
                "answer_model".to_string(),
                serde_json::json!(usage.model.clone()),
            );
        }

        let mut execution = crate::chat::build_chat_execution_from_result(
            &agent_result,
            crate::chat::BuildChatExecutionParams {
                mode: agent_type,
                agent_type,
                session_id: &session.id,
                input_usage_text: request.query.trim(),
                apply_output_guard: true,
                mode_debug: Some(ModeDebug {
                    rag: None,
                    search: None,
                    general: Some(general_debug),
                }),
                debug_metadata: agent_result.debug_payload.clone(),
            },
        );
        execution.tokens_emitted = true;
        execution.citations_emitted = sink.has_citations_emitted();
        execution.assistant_turn_metadata =
            merge_capabilities_turn_metadata(sink.progress_turn_metadata(), caps);
        return Ok(execution);
    }

    let sink = agent_loop::events::CollectingSink::new();
    let agent_result = agent_service.run(agent_request, &sink).await?;

    if let Some(usage) = agent_result.usage.as_ref() {
        general_debug.insert(
            "answer_model".to_string(),
            serde_json::json!(usage.model.clone()),
        );
    }

    let mut execution = crate::chat::build_chat_execution_from_result(
        &agent_result,
        crate::chat::BuildChatExecutionParams {
            mode: agent_type,
            agent_type,
            session_id: &session.id,
            input_usage_text: request.query.trim(),
            apply_output_guard: true,
            mode_debug: Some(ModeDebug {
                rag: None,
                search: None,
                general: Some(general_debug),
            }),
            debug_metadata: agent_result.debug_payload.clone(),
        },
    );
    if emit_debug_trace {
        attach_debug_trace_from_sink(&mut execution, &sink);
    }
    attach_activity_counts_from_sink(&mut execution, &sink);
    merge_activity_counts_into_mode_debug(&mut execution);
    execution.assistant_turn_metadata = merge_capabilities_turn_metadata(
        agent_loop::progress::assistant_progress_turn_metadata(agent_type, &sink.events()),
        caps,
    );
    Ok(execution)
}

/// Extract `DebugTrace` events from a `CollectingSink` and attach them to
/// `execution.debug_metadata` as `{"agent_debug_trace": [...]}`.
/// Used by the non-streaming branch of pure-chat general mode.
pub(crate) fn attach_debug_trace_from_sink(
    execution: &mut ChatExecution,
    sink: &agent_loop::events::CollectingSink,
) {
    let debug_events: Vec<_> = sink
        .events()
        .into_iter()
        .filter_map(|e| match e {
            agent_loop::events::AgentEvent::DebugTrace { kind, payload } => Some((kind, payload)),
            _ => None,
        })
        .collect();
    if !debug_events.is_empty() {
        execution.debug_metadata = Some(serde_json::json!({
            "agent_debug_trace": debug_events,
        }));
    }
}

/// Sum `Activity` event counts across the collected non-streaming events.
/// Stage keys are stable machine identifiers emitted by agent-loop (e.g.
/// `synthesis_code_answer_repair`); consumers fold them into
/// `debug_metadata` / analytics without parsing prose.
pub(crate) fn activity_counts_from_events(
    events: &[agent_loop::events::AgentEvent],
) -> BTreeMap<String, usize> {
    let mut out: BTreeMap<String, usize> = BTreeMap::new();
    for event in events {
        if let agent_loop::events::AgentEvent::Activity { stage, counts, .. } = event {
            let entry = out.entry(stage.clone()).or_insert(0);
            *entry += counts.values().sum::<usize>().max(1);
        }
    }
    out
}

/// Attach the folded `activity_counts` to `execution.debug_metadata`
/// (merged into any existing debug object). Non-streaming path only —
/// streaming consumers read Activity events from SSE directly.
pub(crate) fn attach_activity_counts_from_sink(
    execution: &mut ChatExecution,
    sink: &agent_loop::events::CollectingSink,
) {
    let counts = activity_counts_from_events(&sink.events());
    if counts.is_empty() {
        return;
    }
    let mut meta = execution
        .debug_metadata
        .take()
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    meta.insert(
        "activity_counts".to_string(),
        serde_json::to_value(counts).unwrap_or_default(),
    );
    execution.debug_metadata = Some(serde_json::Value::Object(meta));
}

/// Mirror the folded `activity_counts` from `debug_metadata` into the
/// user-visible `ChatResponse.mode_debug.general` map so non-streaming
/// harnesses can read per-stage counters from the HTTP response.
pub(crate) fn merge_activity_counts_into_mode_debug(execution: &mut ChatExecution) {
    let Some(counts) = execution
        .debug_metadata
        .as_ref()
        .and_then(|v| v.get("activity_counts").cloned())
    else {
        return;
    };
    let mode_debug =
        execution
            .response
            .mode_debug
            .get_or_insert_with(|| contracts::chat::ModeDebug {
                rag: None,
                search: None,
                general: None,
            });
    let general = mode_debug.general.get_or_insert_with(BTreeMap::new);
    general.insert("activity_counts".to_string(), counts);
}

pub(crate) fn emit_terminal_stream_events(
    stream_config: Option<&StreamConfig>,
    execution: &ChatExecution,
) {
    let Some(config) = stream_config else {
        return;
    };

    if !execution.tokens_emitted {
        let answer = execution.response.answer.clone();
        if !answer.is_empty() {
            for chunk in crate::chunk_text_for_stream(&answer) {
                let _ = config.sender.send(contracts::chat::ChatEvent::Token {
                    request_id: config.request_id.clone(),
                    message_id: crate::stream_event_message_id(execution.response.message_id),
                    content: chunk,
                });
            }
        }
    }

    if !execution.citations_emitted && !execution.response.citations.is_empty() {
        let _ = config.sender.send(contracts::chat::ChatEvent::Citations {
            request_id: config.request_id.clone(),
            message_id: crate::stream_event_message_id(execution.response.message_id),
            citations: execution
                .response
                .citations
                .iter()
                .filter_map(|citation| serde_json::to_value(citation).ok())
                .collect(),
        });
    }

    let _ = config.sender.send(contracts::chat::ChatEvent::Done {
        request_id: config.request_id.clone(),
        session_id: execution.response.session_id.clone(),
        message_id: crate::stream_event_message_id(execution.response.message_id),
        payload: crate::chat_done_payload(&execution.response),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn activity(stage: &str, counts: &[(&str, usize)]) -> agent_loop::events::AgentEvent {
        agent_loop::events::AgentEvent::Activity {
            stage: stage.to_string(),
            message: "m".to_string(),
            detail: None,
            counts: counts.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            sources_preview: Vec::new(),
        }
    }

    #[test]
    fn activity_counts_folds_stage_counters() {
        let events = vec![
            activity(
                "synthesis_code_answer_repair",
                &[("synthesis_code_answer_repair", 1)],
            ),
            activity("budget_exhausted", &[]),
            activity(
                "synthesis_code_answer_violation",
                &[("synthesis_code_answer_violation", 1)],
            ),
        ];
        let counts = activity_counts_from_events(&events);
        assert_eq!(counts["synthesis_code_answer_repair"], 1);
        assert_eq!(counts["synthesis_code_answer_violation"], 1);
        assert_eq!(counts["budget_exhausted"], 1);
    }

    #[test]
    fn activity_counts_empty_for_non_activity_events() {
        let events = vec![agent_loop::events::AgentEvent::Done {
            final_message: Some("ok".into()),
            usage: None,
        }];
        assert!(activity_counts_from_events(&events).is_empty());
    }
}
