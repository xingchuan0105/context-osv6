impl AppState {
    pub(crate) async fn execute_clarify_mode_core(
        &self,
        req: &ChatRequest,
        session: &ChatSession,
        message: &str,
    ) -> Result<ChatExecution, AppError> {
        let answer = message.trim().to_string();
        let answer_blocks = common::plain_text_answer_blocks(&answer);

        Ok(ChatExecution {
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
                tool_results: Vec::new(),
                usage: None,
            },
            llm_usage: None,
            debug_metadata: None,
            tokens_emitted: false,
            citations_emitted: false,
            query_resolution: None,
        })
    }

    pub(crate) async fn execute_memory_chat_compat(
        &self,
        req: &ChatRequest,
        session: &ChatSession,
    ) -> Result<ChatExecution, AppError> {
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
            tool_results: Vec::new(),
            turn_metadata: None,
            created_at: now.clone(),
        });
        messages.push(ChatMessage {
            id: assistant_message_id,
            session_id: session.id.clone(),
            role: "assistant".to_string(),
            content: answer.clone(),
            answer_blocks: common::answer_blocks_from_rendered_answer(&answer, &citations),
            agent_id: Some(req.agent_type.clone()),
            agent_name: Some(agent_name(&req.agent_type, req.language.as_deref()).to_string()),
            agent_icon: Some(agent_icon(&req.agent_type).to_string()),
            citations: citations.clone(),
            tool_results: Vec::new(),
            turn_metadata: None,
            created_at: now.clone(),
        });

        if let Some(existing) = state.sessions.get_mut(&session.id) {
            existing.updated_at = now;
        }

        let answer_blocks = common::answer_blocks_from_rendered_answer(&answer, &citations);
        Ok(ChatExecution {
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
                tool_results: Vec::new(),
                usage: None,
            },
            llm_usage: None,
            debug_metadata: None,
            tokens_emitted: false,
            citations_emitted: false,
            query_resolution: None,
        })
    }

}

pub(crate) struct BuildChatExecutionParams<'a> {
    pub mode: &'a str,
    pub agent_type: &'a str,
    pub session_id: &'a str,
    pub input_usage_text: &'a str,
    pub apply_output_guard: bool,
    pub mode_debug: Option<ModeDebug>,
    pub debug_metadata: Option<serde_json::Value>,
}

pub(crate) fn build_chat_execution_from_result(
    agent_result: &crate::agents::runtime::AgentRunResult,
    params: BuildChatExecutionParams<'_>,
) -> ChatExecution {
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
        cached_tokens: usage.cached_tokens.min(u32::MAX as u64) as u32,
    });

    let response_usage = agent_result.usage.as_ref().map(|u| common::ChatTokenUsage {
        prompt_tokens: u.prompt_tokens,
        completion_tokens: u.completion_tokens,
        total_tokens: u.total_tokens,
        cached_tokens: u.cached_tokens,
        provider: Some(u.provider.clone()),
        model: Some(u.model.clone()),
    });

    ChatExecution {
        mode: params.mode.to_string(),
        input_usage_text: params.input_usage_text.to_string(),
        apply_output_guard: params.apply_output_guard,
        response: ChatResponse {
            answer,
            answer_blocks,
            session_id: params.session_id.to_string(),
            agent_type: params.agent_type.to_string(),
            sources: agent_result.sources.clone(),
            citations: agent_result.citations.clone(),
            trace: TraceInfo {
                mode: params.mode.to_string(),
            },
            degrade_trace: agent_result.degrade_trace.clone(),
            planner_output: None,
            mode_debug: params.mode_debug,
            message_id: None,
            guard_report: None,
            tool_results: agent_result.tool_results.iter().map(|r| contracts::chat::ToolResult {
                tool: r.tool.clone(),
                version: r.version.clone(),
                status: match r.status {
                    common::ToolStatus::Ok => contracts::chat::ToolStatus::Ok,
                    common::ToolStatus::Timeout => contracts::chat::ToolStatus::Timeout,
                    common::ToolStatus::Error => contracts::chat::ToolStatus::Error,
                    common::ToolStatus::NotFound => contracts::chat::ToolStatus::NotFound,
                    common::ToolStatus::NotImplemented => contracts::chat::ToolStatus::NotImplemented,
                },
                data: r.data.clone(),
            }).collect(),
            usage: response_usage,
        },
        llm_usage,
        debug_metadata: params.debug_metadata,
        tokens_emitted: false,
        citations_emitted: false,
        query_resolution: agent_result
            .query_resolution
            .as_ref()
            .and_then(|meta| serde_json::to_value(meta).ok()),
    }
}
