impl AppState {
    pub(crate) async fn execute_memory_chat_compat(
        &self,
        req: &ChatRequest,
        session: &ChatSession,
    ) -> Result<ChatGraphExecution, AppError> {
        let notebook = self
            .get_notebook(&session.notebook_id)
            .await
            .ok_or_else(|| AppError::not_found("notebook_not_found", "notebook not found"))?;

        let ready_documents = self
            .list_ready_documents_for_chat(&session.notebook_id, &req.doc_scope)
            .await;
        let context_document = ready_documents.first().cloned();

        let answer = build_answer(
            req.query.trim(),
            &req.agent_type,
            &notebook.name,
            context_document.as_ref(),
            ready_documents.len(),
        );
        let retrieval = context_document.as_ref().map(|document| RetrievedContext {
            chunk_id: document.document.id.clone(),
            page: Some(1),
            score: 0.82,
            source_count: 1,
            source_ids: vec![document.document.id.clone()],
            sparse_hits: 0,
            dense_hits: 0,
            stored_document: document.clone(),
        });
        let citations = build_citations(retrieval.as_ref());
        let sources = build_sources(retrieval.as_ref());
        let planner_output = build_planner_output(req, retrieval.as_ref());
        let mode_debug = build_mode_debug(req, retrieval.as_ref(), &sources);
        let degrade_trace = build_degrade_trace(&req.agent_type, context_document.is_some());

        let mut state = self.inner.write().await;
        let user_message_id = next_message_id(&mut state);
        let assistant_message_id = next_message_id(&mut state);
        let now = now_rfc3339();
        let messages = state.messages.entry(session.id.clone()).or_default();
        messages.push(ChatMessage {
            id: user_message_id,
            session_id: session.id.clone(),
            role: "user".to_string(),
            content: req.query.trim().to_string(),
            answer_blocks: Vec::new(),
            agent_id: None,
            agent_name: None,
            agent_icon: None,
            citations: Vec::new(),
            created_at: now.clone(),
        });
        messages.push(ChatMessage {
            id: assistant_message_id,
            session_id: session.id.clone(),
            role: "assistant".to_string(),
            content: answer.clone(),
            answer_blocks: common::answer_blocks_from_rendered_answer(&answer, &citations),
            agent_id: Some(req.agent_type.clone()),
            agent_name: Some(agent_name(&req.agent_type).to_string()),
            agent_icon: Some(agent_icon(&req.agent_type).to_string()),
            citations: citations.clone(),
            created_at: now.clone(),
        });

        if let Some(existing) = state.sessions.get_mut(&session.id) {
            existing.updated_at = now;
        }

        let answer_blocks = common::answer_blocks_from_rendered_answer(&answer, &citations);
        Ok(ChatGraphExecution {
            mode: req.agent_type.clone(),
            input_usage_text: req.query.trim().to_string(),
            apply_output_guard: false,
            response: ChatResponse {
                answer,
                answer_blocks,
                session_id: session.id.clone(),
                agent_type: req.agent_type.clone(),
                sources,
                citations,
                trace: TraceInfo {
                    mode: session.agent_type.clone(),
                },
                degrade_trace,
                planner_output,
                mode_debug,
                message_id: Some(assistant_message_id),
                guard_report: None,
            },
            llm_usage: None,
        })
    }

    pub(crate) async fn execute_general_mode_core(
        &self,
        req: &ChatRequest,
        session: &ChatSession,
        pg: &PgAppRepository,
    ) -> Result<ChatGraphExecution, AppError> {
        let session_uuid =
            parse_uuid_or_app_error(&session.id, "session_not_found", "session not found")?;
        let memory_context = if let Some(cm) = &self.chatmemory {
            cm.load(&self.auth, session_uuid).await.ok()
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

        if let Ok(db_messages) = pg.list_messages(&self.auth, session_uuid).await {
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

        let mut degrade_trace = Vec::new();
        let mut general_llm_usage: Option<avrag_llm::LlmUsage> = None;
        let mut answer_model: Option<String> = None;
        let answer = if let Some(ref llm) = self.llm_client {
            match llm
                .complete(&messages, self.config.answer_llm.temperature)
                .await
            {
                Ok(resp) => {
                    answer_model = Some(self.config.answer_llm.model.clone());
                    general_llm_usage = Some(resp.usage.clone());
                    resp.content
                }
                Err(error) => {
                    degrade_trace.push(DegradeTraceItem {
                        stage: "general.answer".to_string(),
                        reason: format!("llm_error: {error}"),
                        impact: "Returned retry hint to user".to_string(),
                    });
                    "Network is unstable. Please try again later.".to_string()
                }
            }
        } else {
            degrade_trace.push(DegradeTraceItem {
                stage: "general.answer".to_string(),
                reason: "answer_llm_not_configured".to_string(),
                impact: "Returned retry hint to user".to_string(),
            });
            "Network is unstable. Please try again later.".to_string()
        };

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
        general_debug.insert("answer_model".to_string(), serde_json::json!(answer_model));

        let answer_blocks = common::plain_text_answer_blocks(&answer);
        Ok(ChatGraphExecution {
            mode: "general".to_string(),
            input_usage_text: refined_query,
            apply_output_guard: false,
            response: ChatResponse {
                answer,
                answer_blocks,
                session_id: session.id.clone(),
                agent_type: req.agent_type.clone(),
                sources: Vec::new(),
                citations: Vec::new(),
                trace: TraceInfo {
                    mode: session.agent_type.clone(),
                },
                degrade_trace,
                planner_output: None,
                mode_debug: Some(ModeDebug {
                    rag: None,
                    search: None,
                    general: Some(general_debug),
                }),
                message_id: None,
                guard_report: None,
            },
            llm_usage: general_llm_usage,
        })
    }

    pub(crate) async fn execute_search_mode_core(
        &self,
        req: &ChatRequest,
        session: &ChatSession,
    ) -> Result<ChatGraphExecution, AppError> {
        let mut degrade_trace = Vec::new();
        let (answer, search_results, query_type, sub_queries) =
            if let Some(ref executor) = self.search_executor {
                match executor.execute(req, &self.auth).await {
                    Ok(search_resp) => (
                        search_resp.synthesized_answer.clone(),
                        search_resp.results,
                        search_resp.query_type,
                        search_resp.sub_queries,
                    ),
                    Err(error) => {
                        degrade_trace.push(DegradeTraceItem {
                            stage: "search.execute".to_string(),
                            reason: error.to_string(),
                            impact: "Search mode could not obtain external evidence".to_string(),
                        });
                        (
                            format!("Search mode is unavailable: {}", error),
                            Vec::new(),
                            "unavailable".to_string(),
                            vec![req.query.trim().to_string()],
                        )
                    }
                }
            } else {
                degrade_trace.push(DegradeTraceItem {
                    stage: "search.execute".to_string(),
                    reason: "search_executor_not_configured".to_string(),
                    impact: "Search mode is disabled".to_string(),
                });
                (
                    "Search mode is unavailable because the search executor is not configured."
                        .to_string(),
                    Vec::new(),
                    "unavailable".to_string(),
                    vec![req.query.trim().to_string()],
                )
            };

        let citations: Vec<Citation> = search_results
            .iter()
            .enumerate()
            .map(|(index, result)| Citation {
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
            })
            .collect();

        let sources: Vec<SourceRef> = search_results
            .iter()
            .map(|result| SourceRef {
                id: result.url.clone(),
                title: result.title.clone(),
                snippet: Some(result.snippet.clone()),
                doc_id: None,
                page: None,
            })
            .collect();

        let mut search_debug = BTreeMap::new();
        search_debug.insert("query_type".to_string(), serde_json::json!(query_type));
        search_debug.insert("sub_queries".to_string(), serde_json::json!(sub_queries));
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
            serde_json::json!(search_results.len()),
        );

        let answer_blocks = common::plain_text_answer_blocks(&answer);
        Ok(ChatGraphExecution {
            mode: "search".to_string(),
            input_usage_text: req.query.trim().to_string(),
            apply_output_guard: false,
            response: ChatResponse {
                answer,
                answer_blocks,
                session_id: session.id.clone(),
                agent_type: req.agent_type.clone(),
                sources,
                citations,
                trace: TraceInfo {
                    mode: session.agent_type.clone(),
                },
                degrade_trace,
                planner_output: None,
                mode_debug: Some(ModeDebug {
                    rag: None,
                    search: Some(search_debug),
                    general: None,
                }),
                message_id: None,
                guard_report: None,
            },
            llm_usage: None,
        })
    }
}
