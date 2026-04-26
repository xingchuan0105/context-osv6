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

fn chat_done_payload(response: &common::ChatResponse) -> serde_json::Value {
    serde_json::to_value(response).unwrap_or_else(|_| serde_json::json!({}))
}

fn search_source_preview(result: &avrag_search::SearchResult) -> ChatActivitySourcePreview {
    ChatActivitySourcePreview {
        id: result.url.clone(),
        label: if result.title.trim().is_empty() {
            result.url.clone()
        } else {
            result.title.clone()
        },
        href: Some(result.url.clone()),
    }
}

fn search_citations(results: &[avrag_search::SearchResult]) -> Vec<common::Citation> {
    results
        .iter()
        .enumerate()
        .map(|(index, result)| common::Citation {
            citation_id: result.citation_index.unwrap_or(index + 1) as i64,
            doc_id: result.url.clone(),
            chunk_id: Some(format!(
                "search:{}",
                result.citation_index.unwrap_or(index + 1)
            )),
            page: None,
            doc_name: result.title.clone(),
            preview: Some(result.snippet.clone()),
            content: None,
            score: 1.0,
            layer: Some("search".to_string()),
            chunk_type: None,
            asset_id: None,
            caption: None,
            image_url: None,
            parser_backend: None,
            source_locator: Some(serde_json::json!({
                "url": result.url.clone(),
                "citation_index": result.citation_index.unwrap_or(index + 1),
            })),
        })
        .collect()
}

fn search_sources(results: &[avrag_search::SearchResult]) -> Vec<common::SourceRef> {
    results
        .iter()
        .map(|result| common::SourceRef {
            id: result.url.clone(),
            title: result.title.clone(),
            snippet: Some(result.snippet.clone()),
            doc_id: None,
            page: None,
        })
        .collect()
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

        match crate::main_agent::MainAgent::decide(&req) {
            crate::main_agent::MainAgentDecision::Clarify { message } => {
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
            crate::main_agent::MainAgentDecision::DirectChat => {
                req.agent_type = "general".to_string();
            }
            crate::main_agent::MainAgentDecision::ExternalSearch => {
                req.agent_type = "search".to_string();
            }
            crate::main_agent::MainAgentDecision::ExecutePlan => {
                req.agent_type = "rag".to_string();
            }
        }

        if req.agent_type == "general"
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
        let Some(llm) = &self.llm_client else {
            return Ok(false);
        };

        let session_uuid =
            parse_uuid_or_app_error(&session.id, "session_not_found", "session not found")?;
        let memory_context = if let Some(chatmemory) = &self.chatmemory {
            chatmemory.load(&self.auth, session_uuid).await.ok()
        } else {
            None
        };
        let refined_query = self
            .refine_general_query(req.query.trim(), memory_context.as_ref())
            .await;
        let reference_context = self.build_main_agent_reference_context(Some(session)).await;
        let messages = crate::main_agent::MainAgent::build_general_messages(
            &refined_query,
            reference_context.as_ref(),
        );

        let mut streamed_any = false;
        send_chat_answer_start_event(sender, request_id, &session.id, None, &req.agent_type);
        let streamed = crate::main_agent::MainAgent::answer_general_stream(
            llm,
            &messages,
            self.config.answer_llm.temperature,
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

        let streamed = match streamed {
            Ok(streamed) => streamed,
            Err(error) if !streamed_any => {
                info!(error = %error, "general mode streaming unavailable; falling back to buffered response");
                return Ok(false);
            }
            Err(error) => {
                return Err(AppError::internal(format!(
                    "general mode stream interrupted: {error}"
                )));
            }
        };

        if !streamed_any {
            for chunk in chunk_text_for_stream(&streamed.content) {
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

        let mut general_debug = BTreeMap::new();
        general_debug.insert(
            "refined_query".to_string(),
            serde_json::json!(refined_query.clone()),
        );
        general_debug.insert(
            "memory_loaded".to_string(),
            serde_json::json!(memory_context.is_some()),
        );
        general_debug.insert("summary_updated".to_string(), serde_json::json!(false));
        general_debug.insert(
            "has_profile".to_string(),
            serde_json::json!(
                memory_context
                    .as_ref()
                    .and_then(|m| m.layer3.as_ref())
                    .is_some()
            ),
        );
        general_debug.insert(
            "has_working_memory".to_string(),
            serde_json::json!(
                memory_context
                    .as_ref()
                    .and_then(|m| m.working_memory.as_ref())
                    .is_some()
            ),
        );
        general_debug.insert(
            "answer_model".to_string(),
            serde_json::json!(streamed.model.clone()),
        );

        let answer = streamed.content;
        let answer_blocks = common::plain_text_answer_blocks(&answer);
        let mut execution = crate::chat::ChatGraphExecution {
            mode: "general".to_string(),
            input_usage_text: refined_query,
            apply_output_guard: false,
            response: common::ChatResponse {
                answer,
                answer_blocks,
                session_id: session.id.clone(),
                agent_type: req.agent_type.clone(),
                sources: Vec::new(),
                citations: Vec::new(),
                trace: common::TraceInfo {
                    mode: session.agent_type.clone(),
                },
                degrade_trace: Vec::new(),
                planner_output: None,
                mode_debug: Some(common::ModeDebug {
                    rag: None,
                    search: None,
                    general: Some(general_debug),
                }),
                message_id: None,
                guard_report: None,
            },
            llm_usage: Some(streamed.usage),
            debug_metadata: None,
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
        let Some(executor) = &self.search_executor else {
            return Ok(false);
        };

        let mut answer_started = false;
        let search_response = executor
            .execute_stream(req, |update| match update {
                avrag_search::SearchStreamUpdate::Searching { queries } => {
                    if queries.is_empty() {
                        return;
                    }
                    let mut counts = BTreeMap::new();
                    counts.insert("queries".to_string(), queries.len());
                    send_chat_activity_event(
                        sender,
                        request_id,
                        "searching",
                        "正在搜索网页",
                        Some(format!("查询：{}", queries.join(" · "))),
                        counts,
                        Vec::new(),
                    );
                }
                avrag_search::SearchStreamUpdate::SourcesCollected { results } => {
                    let mut counts = BTreeMap::new();
                    counts.insert("sources".to_string(), results.len());
                    send_chat_activity_event(
                        sender,
                        request_id,
                        "reading_sources",
                        "正在阅读来源",
                        Some("系统正在接收 Perplexity 搜索来源。".to_string()),
                        counts,
                        results.iter().take(3).map(search_source_preview).collect(),
                    );
                }
                avrag_search::SearchStreamUpdate::TextDelta { delta } => {
                    if delta.is_empty() {
                        return;
                    }
                    if !answer_started {
                        send_chat_answer_start_event(
                            sender,
                            request_id,
                            &session.id,
                            None,
                            &req.agent_type,
                        );
                        answer_started = true;
                    }
                    send_chat_stream_event(
                        sender,
                        ChatEvent::Token {
                            request_id: request_id.to_string(),
                            message_id: STREAM_PLACEHOLDER_MESSAGE_ID,
                            content: delta,
                        },
                    );
                }
            })
            .await
            .map_err(map_anyhow_error)?;

        if !answer_started {
            send_chat_answer_start_event(sender, request_id, &session.id, None, &req.agent_type);
            for chunk in chunk_text_for_stream(&search_response.synthesized_answer) {
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

        let mut search_debug = BTreeMap::new();
        search_debug.insert(
            "query_type".to_string(),
            serde_json::json!(search_response.query_type.clone()),
        );
        search_debug.insert(
            "sub_queries".to_string(),
            serde_json::json!(search_response.sub_queries.clone()),
        );
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
            serde_json::json!(search_response.results.len()),
        );

        let answer = search_response.synthesized_answer.clone();
        let mut execution = crate::chat::ChatGraphExecution {
            mode: "search".to_string(),
            input_usage_text: req.query.trim().to_string(),
            apply_output_guard: false,
            response: common::ChatResponse {
                answer: answer.clone(),
                answer_blocks: common::plain_text_answer_blocks(&answer),
                session_id: session.id.clone(),
                agent_type: req.agent_type.clone(),
                sources: search_sources(&search_response.results),
                citations: search_citations(&search_response.results),
                trace: common::TraceInfo {
                    mode: session.agent_type.clone(),
                },
                degrade_trace: Vec::new(),
                planner_output: None,
                mode_debug: Some(common::ModeDebug {
                    rag: None,
                    search: Some(search_debug),
                    general: None,
                }),
                message_id: None,
                guard_report: None,
            },
            llm_usage: search_response.llm_usage,
            debug_metadata: None,
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
            crate::main_agent::MainAgentRagPlanDecision::Execute(execute_request) => {
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
