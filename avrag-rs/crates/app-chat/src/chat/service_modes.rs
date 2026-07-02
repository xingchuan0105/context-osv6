impl ChatContext {
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
                agent_operation_guide: None,
            },
            llm_usage: None,
            debug_metadata: None,
            tokens_emitted: false,
            citations_emitted: false,
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

    let response_usage = agent_result.usage.as_ref().map(|u| contracts::chat::ChatTokenUsage {
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
            tool_results: agent_result.tool_results.iter().map(|r| {
                contracts::chat::ToolResult::from(r.clone())
            }).collect(),
            usage: response_usage,
            agent_operation_guide: None,
        },
        llm_usage,
        debug_metadata: params.debug_metadata,
        tokens_emitted: false,
        citations_emitted: false,
    }
}
