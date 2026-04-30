impl AppState {
    pub(crate) async fn execute_clarify_mode_core(
        &self,
        req: &ChatRequest,
        session: &ChatSession,
        message: &str,
    ) -> Result<ChatGraphExecution, AppError> {
        let answer = message.trim().to_string();
        let answer_blocks = common::plain_text_answer_blocks(&answer);

        Ok(ChatGraphExecution {
            mode: req.agent_type.clone(),
            input_usage_text: req.query.trim().to_string(),
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
                degrade_trace: Vec::new(),
                planner_output: None,
                mode_debug: Some(ModeDebug {
                    rag: None,
                    search: None,
                    general: None,
                }),
                message_id: None,
                guard_report: None,
            },
            llm_usage: None,
            debug_metadata: None,
        })
    }

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
            debug_metadata: None,
        })
    }

    pub(crate) async fn execute_general_mode_core(
        &self,
        req: &ChatRequest,
        session: &ChatSession,
    ) -> Result<ChatGraphExecution, AppError> {
        let Some(agent_service) = self.agent_service() else {
            return Err(AppError::internal("agent service is not configured"));
        };
        let mut agent_request = self
            .build_agent_request(req, crate::agents::AgentKind::Chat)
            .await;
        agent_request.stream = false;
        let mut general_debug = self.build_general_agent_debug(&agent_request);
        let sink = crate::agents::events::CollectingSink::new();
        let agent_result = agent_service.run(agent_request, &sink).await?;

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

        Ok(ChatGraphExecution {
            mode: "chat".to_string(),
            input_usage_text: req.query.trim().to_string(),
            apply_output_guard: false,
            response: ChatResponse {
                answer,
                answer_blocks,
                session_id: session.id.clone(),
                agent_type: "chat".to_string(),
                sources: agent_result.sources,
                citations: agent_result.citations,
                trace: TraceInfo {
                    mode: "chat".to_string(),
                },
                degrade_trace: agent_result.degrade_trace,
                planner_output: None,
                mode_debug: Some(ModeDebug {
                    rag: None,
                    search: None,
                    general: Some(general_debug),
                }),
                message_id: None,
                guard_report: None,
            },
            llm_usage,
            debug_metadata: agent_result.debug_payload,
        })
    }

    pub(crate) async fn execute_search_mode_core(
        &self,
        req: &ChatRequest,
        session: &ChatSession,
    ) -> Result<ChatGraphExecution, AppError> {
        let Some(agent_service) = self.agent_service() else {
            return Err(AppError::internal("agent service is not configured"));
        };
        let mut agent_request = self
            .build_agent_request(req, crate::agents::AgentKind::Search)
            .await;
        agent_request.stream = false;
        let sink = crate::agents::events::CollectingSink::new();
        let agent_result = agent_service.run(agent_request, &sink).await?;

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
        let answer_blocks = common::plain_text_answer_blocks(&answer);
        Ok(ChatGraphExecution {
            mode: "search".to_string(),
            input_usage_text: req.query.trim().to_string(),
            apply_output_guard: false,
            response: ChatResponse {
                answer,
                answer_blocks,
                session_id: session.id.clone(),
                agent_type: "search".to_string(),
                sources: agent_result.sources,
                citations: agent_result.citations,
                trace: TraceInfo {
                    mode: "search".to_string(),
                },
                degrade_trace: agent_result.degrade_trace,
                planner_output: None,
                mode_debug: Some(ModeDebug {
                    rag: None,
                    search: Some(search_debug),
                    general: None,
                }),
                message_id: None,
                guard_report: None,
            },
            llm_usage,
            debug_metadata: None,
        })
    }
}
