impl AppState {
    pub(crate) async fn plan_rag_with_main_agent(
        &self,
        req: &ChatRequest,
        session: Option<&ChatSession>,
        docscope_metadata: Option<&common::DocScopeMetadata>,
        degrade_trace: &mut Vec<DegradeTraceItem>,
    ) -> crate::main_agent::MainAgentPlanResult {
        let reference_context = self.build_main_agent_reference_context(session).await;
        crate::main_agent::MainAgent::plan_rag(
            req,
            docscope_metadata,
            reference_context.as_ref(),
            self.llm_client.as_ref(),
            self.config.answer_llm.temperature.or(Some(0.1)),
            degrade_trace,
        )
        .await
    }

    pub(crate) async fn answer_rag_with_main_agent(
        &self,
        req: &ChatRequest,
        session: Option<&ChatSession>,
        execute_request: &common::ExecutePlanRequest,
        execute_response: &common::ExecutePlanResponse,
        degrade_trace: &mut Vec<DegradeTraceItem>,
    ) -> crate::main_agent::MainAgentAnswerResult {
        let reference_context = self.build_main_agent_reference_context(session).await;
        crate::main_agent::MainAgent::answer_rag(
            req,
            execute_request,
            execute_response,
            reference_context.as_ref(),
            self.llm_client.as_ref(),
            self.config.answer_llm.temperature,
            degrade_trace,
        )
        .await
    }

    pub(crate) async fn answer_rag_with_main_agent_stream(
        &self,
        req: &ChatRequest,
        session: Option<&ChatSession>,
        execute_request: &common::ExecutePlanRequest,
        execute_response: &common::ExecutePlanResponse,
        degrade_trace: &mut Vec<DegradeTraceItem>,
        on_delta: impl FnMut(&str),
    ) -> crate::main_agent::MainAgentAnswerResult {
        let reference_context = self.build_main_agent_reference_context(session).await;
        crate::main_agent::MainAgent::answer_rag_stream(
            req,
            execute_request,
            execute_response,
            reference_context.as_ref(),
            self.llm_client.as_ref(),
            self.config.answer_llm.temperature,
            degrade_trace,
            on_delta,
        )
        .await
    }

    pub(crate) async fn build_main_agent_reference_context(
        &self,
        session: Option<&ChatSession>,
    ) -> Option<crate::main_agent::MainAgentReferenceContext> {
        let user_preferences = self
            .current_user_preferences()
            .await
            .ok()
            .map(|preferences| {
                preferences
                    .agent_memory
                    .active
                    .into_iter()
                    .map(|preference| preference.text)
                    .filter(|value| !value.trim().is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let Some(session) = session else {
            return (!user_preferences.is_empty()).then_some(
                crate::main_agent::MainAgentReferenceContext {
                    session_working_state: None,
                    recent_user_turns: Vec::new(),
                    user_preferences,
                },
            );
        };

        let session_uuid = Uuid::parse_str(&session.id).ok();
        let mut recent_user_turns = Vec::new();
        if let (Some(pg), Some(session_uuid)) = (self.pg(), session_uuid) {
            if let Ok(messages) = pg.list_messages(&self.auth, session_uuid).await {
                recent_user_turns = messages
                    .into_iter()
                    .filter(|message| message.role == "user")
                    .rev()
                    .take(4)
                    .map(|message| message.content.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
            }
        }

        let session_working_state =
            if let (Some(chatmemory), Some(session_uuid)) = (&self.chatmemory, session_uuid) {
                chatmemory
                    .load(&self.auth, session_uuid)
                    .await
                    .ok()
                    .and_then(|memory| {
                        memory
                            .working_memory
                            .as_ref()
                            .and_then(build_session_working_state_text)
                    })
            } else {
                None
            };

        (!user_preferences.is_empty()
            || !recent_user_turns.is_empty()
            || session_working_state.is_some())
        .then_some(crate::main_agent::MainAgentReferenceContext {
            session_working_state,
            recent_user_turns,
            user_preferences,
        })
    }
}

fn build_session_working_state_text(working: &avrag_chatmemory::WorkingMemory) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(topic) = working
        .current_topic
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("topic: {topic}"));
    }
    if let Some(document) = working
        .last_document
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("last_document: {document}"));
    }
    if let Some(entity) = working
        .last_entity
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("last_entity: {entity}"));
    }
    if let Some(question) = working
        .unresolved_question
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            working
                .pending_questions
                .iter()
                .rev()
                .find(|value| !value.trim().is_empty())
                .map(|value| value.trim())
        })
    {
        parts.push(format!("unresolved_question: {question}"));
    }
    (!parts.is_empty()).then(|| parts.join("\n"))
}

#[cfg(test)]
mod main_agent_reference_context_tests {
    use super::*;

    #[test]
    fn working_state_text_uses_v1_shape() {
        let working = avrag_chatmemory::WorkingMemory {
            session_id: Uuid::nil(),
            state_type: "working_memory".to_string(),
            current_topic: Some("pricing".to_string()),
            last_document: Some("pricing.pdf".to_string()),
            last_entity: Some("Atlas".to_string()),
            unresolved_question: Some("What changed?".to_string()),
            pending_questions: vec!["fallback?".to_string()],
            gathered_facts: vec!["ignored fact".to_string()],
            confidence_score: 0.9,
            state_history: Vec::new(),
            last_updated_at: chrono::Utc::now(),
        };

        let text = build_session_working_state_text(&working).unwrap();

        assert!(text.contains("topic: pricing"));
        assert!(text.contains("last_document: pricing.pdf"));
        assert!(text.contains("last_entity: Atlas"));
        assert!(text.contains("unresolved_question: What changed?"));
        assert!(!text.contains("ignored fact"));
    }
}
