use std::collections::BTreeMap;

use common::{AppError, ChatRequest, ChatSession, ModeDebug};

use crate::AppState;

use super::pipeline::{ChatExecution, StreamConfig};

pub(crate) async fn dispatch_mode(
    state: &AppState,
    request: &ChatRequest,
    session: &ChatSession,
    stream_config: Option<&StreamConfig>,
) -> Result<ChatExecution, AppError> {
    let agent_kind = crate::agents::AgentKind::parse(&request.agent_type);

    if matches!(agent_kind, Some(crate::agents::AgentKind::Rag)) && request.doc_scope.is_empty() {
        let message =
            crate::chat::i18n::clarify::need_doc_scope(request.language.as_deref()).to_string();
        return state
            .execute_clarify_mode_core(request, session, &message)
            .await;
    }

    if matches!(agent_kind, Some(crate::agents::AgentKind::Rag))
        && state.uses_memory_adapters()
    {
        return state.execute_memory_chat_compat(request, session).await;
    }

    match agent_kind {
        Some(crate::agents::AgentKind::Chat) | None => {
            run_general_mode(state, request, session, stream_config).await
        }
        Some(crate::agents::AgentKind::Search) => {
            run_search_mode(state, request, session, stream_config).await
        }
        Some(crate::agents::AgentKind::Rag) => {
            run_rag_mode(state, request, session, stream_config).await
        }
    }
}

async fn run_general_mode(
    state: &AppState,
    request: &ChatRequest,
    session: &ChatSession,
    stream_config: Option<&StreamConfig>,
) -> Result<ChatExecution, AppError> {
    let Some(agent_service) = state.agent_service() else {
        return Err(AppError::internal("agent service is not configured"));
    };

    let mut agent_request = state
        .build_agent_request(request, crate::agents::AgentKind::Chat)
        .await;
    if let Some(config) = stream_config {
        agent_request.stream = true;
        agent_request.cancellation_token = Some(config.token.clone());
    }
    let emit_debug_trace = agent_request.debug;
    let mut general_debug = state.build_general_agent_debug(&agent_request);

    if let Some(config) = stream_config {
        let sink = crate::agents::sse_sink::SseSink::new_with_agent_type(
            config.sender.clone(),
            config.request_id.clone(),
            session.id.clone(),
            crate::STREAM_PLACEHOLDER_MESSAGE_ID,
            "chat".to_string(),
        )
        .without_done_event()
        .with_debug_trace(emit_debug_trace);

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
                mode: "chat",
                agent_type: "chat",
                session_id: &session.id,
                input_usage_text: request.query.trim(),
                apply_output_guard: false,
                mode_debug: Some(ModeDebug {
                    rag: None,
                    search: None,
                    general: Some(general_debug),
                }),
                debug_metadata: agent_result.debug_payload.clone(),
            },
        );
        execution.tokens_emitted = true;
        execution.citations_emitted = true;
        return Ok(execution);
    }

    let sink = crate::agents::events::CollectingSink::new();
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
            mode: "chat",
            agent_type: "chat",
            session_id: &session.id,
            input_usage_text: request.query.trim(),
            apply_output_guard: false,
            mode_debug: Some(ModeDebug {
                rag: None,
                search: None,
                general: Some(general_debug),
            }),
            debug_metadata: agent_result.debug_payload.clone(),
        },
    );
    if emit_debug_trace {
        let debug_events: Vec<_> = sink
            .events()
            .into_iter()
            .filter_map(|e| match e {
                crate::agents::events::AgentEvent::DebugTrace { kind, payload } => {
                    Some((kind, payload))
                }
                _ => None,
            })
            .collect();
        if !debug_events.is_empty() {
            execution.debug_metadata = Some(serde_json::json!({
                "agent_debug_trace": debug_events,
            }));
        }
    }
    Ok(execution)
}

async fn run_search_mode(
    state: &AppState,
    request: &ChatRequest,
    session: &ChatSession,
    stream_config: Option<&StreamConfig>,
) -> Result<ChatExecution, AppError> {
    let Some(agent_service) = state.agent_service() else {
        return Err(AppError::internal("agent service is not configured"));
    };

    let mut agent_request = state
        .build_agent_request(request, crate::agents::AgentKind::Search)
        .await;
    if let Some(config) = stream_config {
        agent_request.stream = true;
        agent_request.cancellation_token = Some(config.token.clone());
    }
    let emit_debug_trace = agent_request.debug;

    if let Some(config) = stream_config {
        let sink = crate::agents::sse_sink::SseSink::new_with_agent_type(
            config.sender.clone(),
            config.request_id.clone(),
            session.id.clone(),
            crate::STREAM_PLACEHOLDER_MESSAGE_ID,
            "search".to_string(),
        )
        .without_done_event()
        .with_debug_trace(emit_debug_trace);

        let agent_result = agent_service.run(agent_request, &sink).await?;
        crate::emit_buffered_agent_answer_if_needed(&sink, &agent_result.answer).await;

        let search_debug = build_search_debug(state, &agent_result);
        let mut execution = crate::chat::build_chat_execution_from_result(
            &agent_result,
            crate::chat::BuildChatExecutionParams {
                mode: "search",
                agent_type: "search",
                session_id: &session.id,
                input_usage_text: request.query.trim(),
                // Search 答案合成基于外部网页 snippet，存在 prompt 注入与 PII
                // 泄露风险，必须经过 prompt_leak + pii_scrubber 双层过滤。
                apply_output_guard: true,
                mode_debug: Some(ModeDebug {
                    rag: None,
                    search: Some(search_debug),
                    general: None,
                }),
                debug_metadata: None,
            },
        );
        execution.tokens_emitted = true;
        execution.citations_emitted = true;
        return Ok(execution);
    }

    let sink = crate::agents::events::CollectingSink::new();
    let agent_result = agent_service.run(agent_request, &sink).await?;

    let search_debug = build_search_debug(state, &agent_result);
    let mut execution = crate::chat::build_chat_execution_from_result(
        &agent_result,
        crate::chat::BuildChatExecutionParams {
            mode: "search",
            agent_type: "search",
            session_id: &session.id,
            input_usage_text: request.query.trim(),
            // 同 stream 分支：search 模式输出必经 output guard。
            apply_output_guard: true,
            mode_debug: Some(ModeDebug {
                rag: None,
                search: Some(search_debug),
                general: None,
            }),
            debug_metadata: None,
        },
    );
    if emit_debug_trace {
        let debug_events: Vec<_> = sink
            .events()
            .into_iter()
            .filter_map(|e| match e {
                crate::agents::events::AgentEvent::DebugTrace { kind, payload } => {
                    Some((kind, payload))
                }
                _ => None,
            })
            .collect();
        if !debug_events.is_empty() {
            execution.debug_metadata = Some(serde_json::json!({
                "agent_debug_trace": debug_events,
            }));
        }
    }
    Ok(execution)
}

fn build_search_debug(
    state: &AppState,
    agent_result: &crate::agents::runtime::AgentRunResult,
) -> BTreeMap<String, serde_json::Value> {
    let mut search_debug = BTreeMap::new();
    if let Some(payload) = agent_result.debug_payload.as_ref() {
        if let Some(query_type) = payload.get("query_type") {
            search_debug.insert("query_type".to_string(), query_type.clone());
        }
        if let Some(sub_queries) = payload.get("sub_queries") {
            search_debug.insert("sub_queries".to_string(), sub_queries.clone());
        }
    }
    search_debug.insert(
        "provider".to_string(),
        serde_json::json!(state.search_provider.clone()),
    );
    search_debug.insert(
        "mode".to_string(),
        serde_json::json!(state.search_mode.clone()),
    );
    search_debug.insert(
        "result_count".to_string(),
        serde_json::json!(agent_result.sources.len()),
    );
    search_debug
}

async fn run_rag_mode(
    state: &AppState,
    request: &ChatRequest,
    session: &ChatSession,
    stream_config: Option<&StreamConfig>,
) -> Result<ChatExecution, AppError> {
    let Some(agent_service) = state.agent_service() else {
        return Err(AppError::internal("agent service is not configured"));
    };

    let mut agent_request = state
        .build_agent_request(request, crate::agents::AgentKind::Rag)
        .await;

    if !request.doc_scope.is_empty()
        && let Ok(metadata) = state.load_docscope_metadata(&request.doc_scope).await {
            agent_request.docscope_metadata = Some(metadata);
        }

    if let Some(config) = stream_config {
        agent_request.stream = true;
        agent_request.cancellation_token = Some(config.token.clone());
        let emit_debug_trace = agent_request.debug;
        let sink = crate::agents::sse_sink::SseSink::new_with_agent_type(
            config.sender.clone(),
            config.request_id.clone(),
            session.id.clone(),
            crate::STREAM_PLACEHOLDER_MESSAGE_ID,
            "rag".to_string(),
        )
        .without_done_event()
        .with_debug_trace(emit_debug_trace);

        let agent_result = agent_service.run(agent_request, &sink).await?;
        crate::emit_buffered_agent_answer_if_needed(&sink, &agent_result.answer).await;

        let mut execution = crate::chat::build_chat_execution_from_result(
            &agent_result,
            crate::chat::BuildChatExecutionParams {
                mode: "rag",
                agent_type: "rag",
                session_id: &session.id,
                input_usage_text: request.query.trim(),
                apply_output_guard: true,
                mode_debug: None,
                debug_metadata: agent_result.debug_payload.clone(),
            },
        );
        execution.tokens_emitted = true;
        execution.citations_emitted = true;
        return Ok(execution);
    }

    let sink = crate::agents::events::CollectingSink::new();
    let agent_result = agent_service.run(agent_request, &sink).await?;

    let execution = crate::chat::build_chat_execution_from_result(
        &agent_result,
        crate::chat::BuildChatExecutionParams {
            mode: "rag",
            agent_type: "rag",
            session_id: &session.id,
            input_usage_text: request.query.trim(),
            apply_output_guard: true,
            mode_debug: None,
            debug_metadata: agent_result.debug_payload.clone(),
        },
    );
    Ok(execution)
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
