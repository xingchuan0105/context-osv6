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
            timestamp: None,
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
            citation_id: (index + 1) as i64,
            doc_id: result.url.clone(),
            chunk_id: None,
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
            source_locator: None,
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

        if req.agent_type == "rag"
            && self
                .execute_rag_chat_stream(&req, &session, &preflight, &request_id, &sender)
                .await?
        {
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
            self.emit_notifications_for_execution(session, execution).await?;
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
        let pg = self.pg();
        let memory_context = if let Some(chatmemory) = &self.chatmemory {
            chatmemory.load(&self.auth, session_uuid).await.ok()
        } else {
            None
        };
        let refined_query = self
            .refine_general_query(req.query.trim(), memory_context.as_ref())
            .await;
        let mut messages = vec![avrag_llm::ChatMessage::system(
            "You are the general assistant for Context OS. Maintain continuity across turns, use conversation memory when relevant, and answer directly without inventing facts.",
        )];

        if let Some(ref memory) = memory_context {
            if let Some(ref layer2) = memory.layer2 {
                messages.push(avrag_llm::ChatMessage::system(format!(
                    "Conversation summary:\n{}",
                    layer2.summary
                )));
            }
            if let Some(ref layer3) = memory.layer3 {
                messages.push(avrag_llm::ChatMessage::system(format!(
                    "User profile:\nDomains: {}\nPreferred answer style: {}\nFrequently asked topics: {}",
                    layer3.expertise_domains.join(", "),
                    layer3
                        .preferred_answer_style
                        .clone()
                        .unwrap_or_else(|| "default".to_string()),
                    layer3.frequently_asked_topics.join(", ")
                )));
            }
            if let Some(ref working_memory) = memory.working_memory {
                messages.push(avrag_llm::ChatMessage::system(format!(
                    "Working memory:\nCurrent topic: {}\nPending questions: {}\nKnown facts: {}",
                    working_memory
                        .current_topic
                        .clone()
                        .unwrap_or_else(|| "none".to_string()),
                    working_memory.pending_questions.join(" | "),
                    working_memory.gathered_facts.join(" | ")
                )));
            }
        }

        if let Some(repository) = &pg
            && let Ok(db_messages) = repository.list_messages(&self.auth, session_uuid).await
        {
            for msg in db_messages
                .into_iter()
                .rev()
                .take(12)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
            {
                messages.push(avrag_llm::ChatMessage {
                    role: if msg.role == "user" {
                        "user".to_string()
                    } else {
                        "assistant".to_string()
                    },
                    content: msg.content,
                });
            }
        }
        messages.push(avrag_llm::ChatMessage::user(refined_query.clone()));

        let mut streamed_any = false;
        send_chat_answer_start_event(
            sender,
            request_id,
            &session.id,
            None,
            &req.agent_type,
        );
        let streamed = llm
            .complete_stream(&messages, self.config.answer_llm.temperature, |delta| {
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
            })
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
            send_chat_answer_start_event(
                sender,
                request_id,
                &session.id,
                None,
                &req.agent_type,
            );
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
        let Some(rag_runtime) = &self.rag_runtime else {
            return Ok(false);
        };

        let docscope_metadata = if !req.doc_scope.is_empty() {
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

        let (mut rag_plan, planner_usage) = rag_runtime
            .plan(
                req,
                None,
                docscope_metadata.as_ref(),
                &mut degrade_trace,
            )
            .await
            .map_err(map_anyhow_error)?;
        if let Some(usage) = planner_usage.as_ref() {
            self.record_llm_usage_if_available(
                avrag_usage_limit::BillableFeature::Planner,
                "rag_planner",
                usage,
                "streaming",
            )
            .await;
        }

        if rag_plan.clarify_needed {
            degrade_trace.push(common::DegradeTraceItem {
                stage: "rag.plan".to_string(),
                reason: "planner clarify output ignored by main-agent controlled rag flow"
                    .to_string(),
                impact: "Continuing with normalized execute-plan retrieval.".to_string(),
            });
            rag_plan.clarify_needed = false;
            rag_plan.clarify_message.clear();
        }

        let item_trace = rag_runtime.normalize_plan(req, &mut rag_plan);
        let execute_response = self
            .execute_rag_execute_plan(common::ExecutePlanRequest::from_rag_plan(
                &rag_plan,
                &req.doc_scope,
            ))
            .await?;
        degrade_trace.extend(execute_response.degrade_trace.clone());

        send_chat_activity_event(
            sender,
            request_id,
            "retrieving",
            "正在检索知识库",
            Some("系统正在执行结构化检索计划。".to_string()),
            BTreeMap::from([
                ("queries".to_string(), item_trace.len()),
                (
                    "chunks".to_string(),
                    execute_response.coverage.retrieved_chunk_count,
                ),
            ]),
            Vec::new(),
        );
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
        send_chat_answer_start_event(
            sender,
            request_id,
            &session.id,
            None,
            &req.agent_type,
        );
        let synthesis_output = rag_runtime
            .synthesize_answer_text_stream(
                req,
                None,
                &rag_plan,
                &item_trace,
                &answer_context,
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
            for chunk in chunk_text_for_stream(&synthesis_output.answer_text) {
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

        let rag_llm_usage = synthesis_output.llm_usage.clone();
        let response = rag_runtime
            .build_rag_chat_response_from_bundle(
                req,
                Some(&session.id),
                &rag_plan,
                &execute_response,
                synthesis_output,
                degrade_trace,
            )
            .await
            .map_err(map_anyhow_error)?;

        let mut execution = crate::chat::ChatGraphExecution {
            mode: "rag".to_string(),
            input_usage_text: req.query.trim().to_string(),
            apply_output_guard: true,
            response,
            llm_usage: rag_llm_usage,
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
