use crate::agents::{
    AgentKind,
    events::{AgentEvent, AgentEventSink},
};
use contracts::chat::{ChatActivitySourcePreview, ChatEvent};
use tokio::sync::mpsc::UnboundedSender;

const STREAM_PLACEHOLDER_MESSAGE_ID: i64 = 0;
const STREAM_TOKEN_CHUNK_CHARS: usize = 24;

fn send_chat_stream_event(sender: &UnboundedSender<ChatEvent>, event: ChatEvent) {
    let _ = sender.send(event);
}

fn send_chat_activity_event(
    sender: &UnboundedSender<ChatEvent>,
    request_id: &str,
    phase: &str,
    title: impl Into<String>,
    detail: Option<String>,
    counts: BTreeMap<String, usize>,
    sources_preview: Vec<ChatActivitySourcePreview>,
) {
    send_chat_stream_event(
        sender,
        ChatEvent::Activity {
            request_id: request_id.to_string(),
            phase: phase.to_string(),
            title: title.into(),
            detail,
            counts,
            sources_preview,
            timestamp: Some(now_rfc3339()),
        },
    );
}

fn send_chat_answer_start_event(
    sender: &UnboundedSender<ChatEvent>,
    request_id: &str,
    session_id: &str,
    message_id: Option<i64>,
    agent_type: &str,
) {
    send_chat_stream_event(
        sender,
        ChatEvent::AnswerStart {
            request_id: request_id.to_string(),
            session_id: session_id.to_string(),
            message_id: stream_event_message_id(message_id),
            agent_type: agent_type.to_string(),
        },
    );
}

fn stream_event_message_id(message_id: Option<i64>) -> i64 {
    message_id.unwrap_or(STREAM_PLACEHOLDER_MESSAGE_ID)
}

fn chunk_text_for_stream(text: &str) -> Vec<String> {
    let chars = text.chars().collect::<Vec<_>>();

    if chars.is_empty() {
        return Vec::new();
    }

    chars
        .chunks(STREAM_TOKEN_CHUNK_CHARS)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect()
}

async fn emit_buffered_agent_answer_if_needed(
    sink: &crate::agents::sse_sink::SseSink,
    answer: &str,
) {
    if sink.has_message_delta() || answer.is_empty() {
        return;
    }

    for chunk in chunk_text_for_stream(answer) {
        sink.emit(AgentEvent::MessageDelta { text: chunk }).await;
    }
}

fn chat_done_payload(response: &common::ChatResponse) -> serde_json::Value {
    serde_json::to_value(response).unwrap_or_else(|_| serde_json::json!({}))
}

impl AppState {
    pub async fn execute_chat_stream(
        &self,
        mut req: common::ChatRequest,
        request_id: String,
        sender: UnboundedSender<ChatEvent>,
    ) -> Result<(), AppError> {
        if req.query.trim().is_empty() {
            return Err(AppError::validation("query_required", "query is required"));
        }

        let preflight = self.execute_chat_preflight(&req).await?;
        let session = self.resolve_chat_session(&req).await?;
        req.session_id = Some(session.id.clone());

        send_chat_stream_event(
            &sender,
            ChatEvent::Start {
                request_id: request_id.clone(),
                session_id: session.id.clone(),
            },
        );

        // Route via AgentKind (canonical agent dispatch).
        match AgentKind::parse(&req.agent_type) {
            Some(AgentKind::Rag) if req.doc_scope.is_empty() => {
                // Clarify: RAG mode requires doc_scope
                let message = "请先选择要检索的文档范围，再让我执行知识库检索。".to_string();
                let mut execution = self
                    .execute_clarify_mode_core(&req, &session, &message)
                    .await?;
                send_chat_answer_start_event(
                    &sender,
                    &request_id,
                    &session.id,
                    None,
                    &req.agent_type,
                );
                for chunk in chunk_text_for_stream(&execution.response.answer) {
                    send_chat_stream_event(
                        &sender,
                        ChatEvent::Token {
                            request_id: request_id.clone(),
                            message_id: STREAM_PLACEHOLDER_MESSAGE_ID,
                            content: chunk,
                        },
                    );
                }
                self.finalize_stream_execution(
                    &req,
                    &session,
                    &preflight,
                    &request_id,
                    &sender,
                    &mut execution,
                )
                .await?;
                return Ok(());
            }
            Some(AgentKind::Chat) | None => {
                req.agent_type = "chat".to_string();
            }
            Some(AgentKind::Search) => {
                req.agent_type = "search".to_string();
            }
            Some(AgentKind::Rag) => {
                req.agent_type = "rag".to_string();
            }
        }

        if req.agent_type == "chat"
            && self
                .execute_general_chat_stream(&req, &session, &preflight, &request_id, &sender)
                .await?
        {
            return Ok(());
        }

        if req.agent_type == "search"
            && self
                .execute_search_chat_stream(&req, &session, &preflight, &request_id, &sender)
                .await?
        {
            return Ok(());
        }

        if req.agent_type == "rag" {
            if self
                .execute_rag_chat_stream(&req, &session, &preflight, &request_id, &sender)
                .await?
            {
                return Ok(());
            }

            send_chat_activity_event(
                &sender,
                &request_id,
                "planning",
                "正在分析问题",
                Some("系统正在规划知识库检索范围。".to_string()),
                BTreeMap::new(),
                Vec::new(),
            );
            send_chat_activity_event(
                &sender,
                &request_id,
                "retrieving",
                "正在检索知识库",
                Some("系统正在执行结构化检索计划。".to_string()),
                BTreeMap::from([("queries".to_string(), 1)]),
                Vec::new(),
            );

            let response = self.execute_chat(req).await?;

            send_chat_activity_event(
                &sender,
                &request_id,
                "reading_sources",
                "正在阅读命中内容",
                Some("系统正在筛选证据片段并准备最终答案上下文。".to_string()),
                BTreeMap::from([
                    ("documents".to_string(), response.sources.len()),
                    ("chunks".to_string(), response.citations.len()),
                ]),
                response
                    .sources
                    .iter()
                    .take(3)
                    .map(|source| ChatActivitySourcePreview {
                        id: source.id.clone(),
                        label: if source.title.trim().is_empty() {
                            source.id.clone()
                        } else {
                            source.title.clone()
                        },
                        href: None,
                    })
                    .collect(),
            );
            send_chat_activity_event(
                &sender,
                &request_id,
                "drafting_answer",
                "正在生成回答",
                Some("证据整理完成，开始生成最终答案。".to_string()),
                BTreeMap::from([("chunks".to_string(), response.citations.len())]),
                Vec::new(),
            );
            self.stream_buffered_chat_response(&request_id, &response, &sender);
            return Ok(());
        }

        let response = self.execute_chat(req).await?;
        self.stream_buffered_chat_response(&request_id, &response, &sender);
        Ok(())
    }

    async fn finalize_stream_execution(
        &self,
        req: &common::ChatRequest,
        session: &common::ChatSession,
        preflight: &crate::chat::ChatPreflight,
        request_id: &str,
        sender: &UnboundedSender<ChatEvent>,
        execution: &mut crate::chat::ChatGraphExecution,
    ) -> Result<(), AppError> {
        self.apply_output_guard_to_execution(
            session,
            execution,
            &preflight.trace_id,
            preflight.user_uuid,
            self.pg().as_deref(),
        )
        .await?;

        if req.source_type.as_deref() != Some("share")
            && let Some(repository) = self.pg()
        {
            self.persist_chat_execution(req, session, execution, repository.as_ref())
                .await?;
        }

        self.record_usage_for_execution(execution).await?;

        if req.source_type.as_deref() != Some("share") {
            self.emit_notifications_for_execution(session, execution)
                .await?;
        }

        if !execution.response.citations.is_empty() {
            send_chat_stream_event(
                sender,
                ChatEvent::Citations {
                    request_id: request_id.to_string(),
                    message_id: stream_event_message_id(execution.response.message_id),
                    citations: execution
                        .response
                        .citations
                        .iter()
                        .filter_map(|citation| serde_json::to_value(citation).ok())
                        .collect(),
                },
            );
        }

        send_chat_stream_event(
            sender,
            ChatEvent::Done {
                request_id: request_id.to_string(),
                session_id: execution.response.session_id.clone(),
                message_id: stream_event_message_id(execution.response.message_id),
                payload: chat_done_payload(&execution.response),
            },
        );

        Ok(())
    }

    async fn execute_general_chat_stream(
        &self,
        req: &common::ChatRequest,
        session: &common::ChatSession,
        preflight: &crate::chat::ChatPreflight,
        request_id: &str,
        sender: &UnboundedSender<ChatEvent>,
    ) -> Result<bool, AppError> {
        if self.llm_client.is_none() {
            return Ok(false);
        }
        let Some(agent_service) = self.agent_service() else {
            return Ok(false);
        };

        let mut agent_request = self.build_agent_request(req, AgentKind::Chat).await;
        agent_request.stream = true;
        let emit_debug_trace = agent_request.debug;
        let mut general_debug = self.build_general_agent_debug(&agent_request);
        let sink = crate::agents::sse_sink::SseSink::new_with_agent_type(
            sender.clone(),
            request_id.to_string(),
            session.id.clone(),
            STREAM_PLACEHOLDER_MESSAGE_ID,
            "chat".to_string(),
        )
        .without_done_event()
        .with_debug_trace(emit_debug_trace);

        let agent_result = agent_service.run(agent_request, &sink).await?;
        emit_buffered_agent_answer_if_needed(&sink, &agent_result.answer).await;

        if let Some(usage) = agent_result.usage.as_ref() {
            general_debug.insert("answer_model".to_string(), serde_json::json!(usage.model.clone()));
        }

        let answer = agent_result.answer.clone();
        let answer_blocks = if agent_result.answer_blocks.is_empty() {
            common::plain_text_answer_blocks(&answer)
        } else {
            agent_result.answer_blocks.clone()
        };
        let llm_usage = agent_result.usage.as_ref().map(|usage| avrag_llm::LlmUsage {
            prompt_tokens: usage.prompt_tokens.min(u32::MAX as u64) as u32,
            completion_tokens: usage.completion_tokens.min(u32::MAX as u64) as u32,
            total_tokens: usage.total_tokens.min(u32::MAX as u64) as u32,
            provider: usage.provider.clone(),
            model: usage.model.clone(),
        });

        let mut execution = crate::chat::ChatGraphExecution {
            mode: "chat".to_string(),
            input_usage_text: req.query.trim().to_string(),
            apply_output_guard: false,
            response: common::ChatResponse {
                answer,
                answer_blocks,
                session_id: session.id.clone(),
                agent_type: "chat".to_string(),
                sources: agent_result.sources,
                citations: agent_result.citations,
                trace: common::TraceInfo {
                    mode: "chat".to_string(),
                },
                degrade_trace: agent_result.degrade_trace,
                planner_output: None,
                mode_debug: Some(common::ModeDebug {
                    rag: None,
                    search: None,
                    general: Some(general_debug),
                }),
                message_id: None,
                guard_report: None,
            },
            llm_usage,
            debug_metadata: agent_result.debug_payload,
        };

        self.finalize_stream_execution(req, session, preflight, request_id, sender, &mut execution)
            .await?;

        Ok(true)
    }

    async fn execute_search_chat_stream(
        &self,
        req: &common::ChatRequest,
        session: &common::ChatSession,
        preflight: &crate::chat::ChatPreflight,
        request_id: &str,
        sender: &UnboundedSender<ChatEvent>,
    ) -> Result<bool, AppError> {
        let Some(agent_service) = self.agent_service() else {
            return Ok(false);
        };

        let mut agent_request = self.build_agent_request(req, AgentKind::Search).await;
        agent_request.stream = true;
        let emit_debug_trace = agent_request.debug;
        let sink = crate::agents::sse_sink::SseSink::new_with_agent_type(
            sender.clone(),
            request_id.to_string(),
            session.id.clone(),
            STREAM_PLACEHOLDER_MESSAGE_ID,
            "search".to_string(),
        )
        .without_done_event()
        .with_debug_trace(emit_debug_trace);

        let agent_result = agent_service.run(agent_request, &sink).await?;
        emit_buffered_agent_answer_if_needed(&sink, &agent_result.answer).await;

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
            serde_json::json!(self.config.search.provider.clone()),
        );
        search_debug.insert(
            "mode".to_string(),
            serde_json::json!(self.config.search.mode.clone()),
        );
        search_debug.insert(
            "result_count".to_string(),
            serde_json::json!(agent_result.sources.len()),
        );

        let answer = agent_result.answer.clone();
        let llm_usage = agent_result.usage.as_ref().map(|usage| avrag_llm::LlmUsage {
            prompt_tokens: usage.prompt_tokens.min(u32::MAX as u64) as u32,
            completion_tokens: usage.completion_tokens.min(u32::MAX as u64) as u32,
            total_tokens: usage.total_tokens.min(u32::MAX as u64) as u32,
            provider: usage.provider.clone(),
            model: usage.model.clone(),
        });
        let mut execution = crate::chat::ChatGraphExecution {
            mode: "search".to_string(),
            input_usage_text: req.query.trim().to_string(),
            apply_output_guard: false,
            response: common::ChatResponse {
                answer: answer.clone(),
                answer_blocks: common::plain_text_answer_blocks(&answer),
                session_id: session.id.clone(),
                agent_type: "search".to_string(),
                sources: agent_result.sources,
                citations: agent_result.citations,
                trace: common::TraceInfo {
                    mode: "search".to_string(),
                },
                degrade_trace: agent_result.degrade_trace,
                planner_output: None,
                mode_debug: Some(common::ModeDebug {
                    rag: None,
                    search: Some(search_debug),
                    general: None,
                }),
                message_id: None,
                guard_report: None,
            },
            llm_usage,
            debug_metadata: agent_result.debug_payload,
        };

        self.finalize_stream_execution(req, session, preflight, request_id, sender, &mut execution)
            .await?;

        Ok(true)
    }

    async fn execute_rag_chat_stream(
        &self,
        req: &common::ChatRequest,
        session: &common::ChatSession,
        preflight: &crate::chat::ChatPreflight,
        request_id: &str,
        sender: &UnboundedSender<ChatEvent>,
    ) -> Result<bool, AppError> {
        let docscope_metadata = if !req.doc_scope.is_empty() && self.pg().is_some() {
            Some(self.load_docscope_metadata(&req.doc_scope).await?)
        } else {
            None
        };

        let mut degrade_trace = Vec::new();
        send_chat_activity_event(
            sender,
            request_id,
            "planning",
            "正在分析问题",
            Some("系统正在规划知识库检索范围。".to_string()),
            BTreeMap::new(),
            Vec::new(),
        );

        let plan_result = self
            .plan_rag_with_main_agent(
                req,
                Some(session),
                docscope_metadata.as_ref(),
                &mut degrade_trace,
            )
            .await;
        if let Some(usage) = plan_result.llm_usage.as_ref() {
            self.record_llm_usage_if_available(
                avrag_usage_limit::BillableFeature::Chat,
                "main_agent_rag_plan",
                usage,
                "streaming",
            )
            .await;
        }

        let execute_request = match plan_result.decision {
            crate::main_agent::MainAgentRagPlanDecision::Execute(mut execute_request) => {
                execute_request.ensure_original_query_text_dense_item(req.query.trim());
                execute_request.validate().map_err(|error| {
                    AppError::validation("invalid_rag_plan", error.to_string())
                })?;
                execute_request
            }
            crate::main_agent::MainAgentRagPlanDecision::Clarify(message) => {
                let mut execution = self
                    .execute_clarify_mode_core(req, session, &message)
                    .await?;
                send_chat_answer_start_event(
                    sender,
                    request_id,
                    &session.id,
                    None,
                    &req.agent_type,
                );
                for chunk in chunk_text_for_stream(&execution.response.answer) {
                    send_chat_stream_event(
                        sender,
                        ChatEvent::Token {
                            request_id: request_id.to_string(),
                            message_id: STREAM_PLACEHOLDER_MESSAGE_ID,
                            content: chunk,
                        },
                    );
                }
                self.finalize_stream_execution(
                    req,
                    session,
                    preflight,
                    request_id,
                    sender,
                    &mut execution,
                )
                .await?;
                return Ok(true);
            }
        };

        send_chat_activity_event(
            sender,
            request_id,
            "retrieving",
            "正在检索知识库",
            Some("系统正在执行结构化检索计划。".to_string()),
            BTreeMap::from([("queries".to_string(), execute_request.items.len())]),
            Vec::new(),
        );
        let execute_response = self
            .execute_rag_execute_plan(execute_request.clone())
            .await?;
        degrade_trace.extend(execute_response.degrade_trace.clone());
        let unique_doc_count = execute_response.coverage.matched_doc_count;

        send_chat_activity_event(
            sender,
            request_id,
            "reading_sources",
            "正在阅读命中内容",
            Some("系统正在筛选证据片段并准备最终答案上下文。".to_string()),
            BTreeMap::from([
                ("documents".to_string(), unique_doc_count),
                (
                    "chunks".to_string(),
                    execute_response.coverage.retrieved_chunk_count,
                ),
            ]),
            execute_response
                .bundle
                .chunks
                .iter()
                .take(3)
                .map(|chunk| ChatActivitySourcePreview {
                    id: chunk.chunk_id.clone(),
                    label: format!("Doc {}", chunk.doc_id),
                    href: None,
                })
                .collect(),
        );
        let answer_context = crate::main_agent::MainAgent::answer_context(&execute_response);

        send_chat_activity_event(
            sender,
            request_id,
            "drafting_answer",
            "正在生成回答",
            Some("证据整理完成，开始生成最终答案。".to_string()),
            BTreeMap::from([("chunks".to_string(), answer_context.len())]),
            Vec::new(),
        );

        let mut streamed_any = false;
        send_chat_answer_start_event(sender, request_id, &session.id, None, &req.agent_type);
        let answer_output = self
            .answer_rag_with_main_agent_stream(
                req,
                Some(session),
                &execute_request,
                &execute_response,
                &mut degrade_trace,
                |delta| {
                    if delta.is_empty() {
                        return;
                    }
                    streamed_any = true;
                    send_chat_stream_event(
                        sender,
                        ChatEvent::Token {
                            request_id: request_id.to_string(),
                            message_id: STREAM_PLACEHOLDER_MESSAGE_ID,
                            content: delta.to_string(),
                        },
                    );
                },
            )
            .await;

        if !streamed_any {
            for chunk in chunk_text_for_stream(&answer_output.answer_text) {
                send_chat_stream_event(
                    sender,
                    ChatEvent::Token {
                        request_id: request_id.to_string(),
                        message_id: STREAM_PLACEHOLDER_MESSAGE_ID,
                        content: chunk,
                    },
                );
            }
        }

        let rag_llm_usage = answer_output.llm_usage.clone();
        let response = crate::main_agent::MainAgent::build_rag_chat_response(
            req,
            Some(&session.id),
            &execute_request,
            &execute_response,
            answer_output,
            degrade_trace,
        );

        let mut execution = crate::chat::ChatGraphExecution {
            mode: "rag".to_string(),
            input_usage_text: req.query.trim().to_string(),
            apply_output_guard: true,
            response,
            llm_usage: rag_llm_usage,
            debug_metadata: Some(serde_json::json!({
                "rag_tool": {
                    "execute_plan_request": &execute_request,
                    "execute_plan_coverage": &execute_response.coverage,
                    "execute_plan_backend_trace": &execute_response.backend_trace,
                }
            })),
        };

        self.finalize_stream_execution(req, session, preflight, request_id, sender, &mut execution)
            .await?;

        Ok(true)
    }

    fn stream_buffered_chat_response(
        &self,
        request_id: &str,
        response: &common::ChatResponse,
        sender: &UnboundedSender<ChatEvent>,
    ) {
        let message_id = stream_event_message_id(response.message_id);
        send_chat_answer_start_event(
            sender,
            request_id,
            &response.session_id,
            response.message_id,
            &response.agent_type,
        );

        for chunk in chunk_text_for_stream(&response.answer) {
            send_chat_stream_event(
                sender,
                ChatEvent::Token {
                    request_id: request_id.to_string(),
                    message_id,
                    content: chunk,
                },
            );
        }

        if !response.citations.is_empty() {
            send_chat_stream_event(
                sender,
                ChatEvent::Citations {
                    request_id: request_id.to_string(),
                    message_id,
                    citations: response
                        .citations
                        .iter()
                        .filter_map(|citation| serde_json::to_value(citation).ok())
                        .collect(),
                },
            );
        }

        send_chat_stream_event(
            sender,
            ChatEvent::Done {
                request_id: request_id.to_string(),
                session_id: response.session_id.clone(),
                message_id,
                payload: chat_done_payload(response),
            },
        );
    }
}
