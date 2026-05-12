use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::runtime::{Agent, AgentRequest, AgentRunResult, AgentRunUsage};
use common::AppError;

/// ChatAgent handles direct (non-RAG, non-search) conversational queries.
///
/// It uses a configured LLM client to answer the user query.  Memory context
/// (session summary, user preferences, working memory) is injected by the
/// caller via `AgentRequest` fields.
pub struct ChatAgent {
    llm_client: Option<avrag_llm::LlmClient>,
    temperature: Option<f32>,
}

impl ChatAgent {
    pub fn new(llm_client: Option<avrag_llm::LlmClient>, temperature: Option<f32>) -> Self {
        Self {
            llm_client,
            temperature,
        }
    }
}

pub(crate) fn build_chat_messages(request: &AgentRequest) -> Vec<avrag_llm::ChatMessage> {
    let mut system = String::from(include_str!(
        "../../../../prompts/chat_agent_system.txt"
    ));
    if let Some(summary) = request
        .session_summary
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        system.push_str("\n\nSession summary:\n");
        system.push_str(summary.trim());
    }
    if let Some(preferences) = request.user_preferences.as_ref() {
        system.push_str("\n\nUser preferences:\n");
        system.push_str(&preferences.to_string());
    }

    let mut messages = vec![avrag_llm::ChatMessage::system(system)];
    for message in &request.messages {
        match message.role.as_str() {
            "assistant" => messages.push(avrag_llm::ChatMessage::assistant(&message.content)),
            _ => messages.push(avrag_llm::ChatMessage::user(&message.content)),
        }
    }
    messages.push(avrag_llm::ChatMessage::user(&request.query));
    messages
}

#[async_trait::async_trait]
impl Agent for ChatAgent {
    #[tracing::instrument(skip(self, sink), fields(agent_kind = ?request.kind))]
    async fn run(
        &self,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, AppError> {
        let Some(ref llm) = self.llm_client else {
            let _ = sink.emit(AgentEvent::Error {
                code: "llm_unavailable".to_string(),
                message: "LLM client is not configured".to_string(),
            })
            .await;
            return Err(AppError::internal("LLM client is not configured"));
        };

        let _ = sink.emit(AgentEvent::Activity {
            stage: "chat".to_string(),
            message: "Direct chat".to_string(),
        })
        .await;

        let messages = build_chat_messages(&request);

        let response = if request.stream {
            let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
            let token = request
                .cancellation_token
                .clone()
                .unwrap_or_default();
            let stream = llm.complete_stream(&messages, self.temperature, token, move |delta| {
                if !delta.is_empty() {
                    let _ = delta_tx.send(delta.to_string());
                }
            });
            tokio::pin!(stream);

            let response = loop {
                tokio::select! {
                    delta = delta_rx.recv() => {
                        if let Some(delta) = delta {
                            let _ = sink.emit(AgentEvent::MessageDelta { text: delta }).await;
                        }
                    }
                    result = &mut stream => {
                        break result.map_err(|e| AppError::internal(format!("LLM completion stream failed: {}", e)))?;
                    }
                }
            };

            while let Ok(delta) = delta_rx.try_recv() {
                let _ = sink.emit(AgentEvent::MessageDelta { text: delta }).await;
            }

            response
        } else {
            let response = llm
                .complete(&messages, self.temperature)
                .await
                .map_err(|e| AppError::internal(format!("LLM completion failed: {}", e)))?;
            let _ = sink.emit(AgentEvent::MessageDelta {
                text: response.content.clone(),
            })
            .await;
            response
        };

        let usage = response.usage.clone();
        let _ = sink.emit(AgentEvent::Usage {
            provider: usage.provider.clone(),
            model: usage.model.clone(),
            prompt_tokens: usage.prompt_tokens as u64,
            completion_tokens: usage.completion_tokens as u64,
            total_tokens: usage.total_tokens as u64,
            request_count: 1,
            metadata: Default::default(),
        })
        .await;

        let run_usage = AgentRunUsage {
            provider: usage.provider.clone(),
            model: usage.model.clone(),
            prompt_tokens: usage.prompt_tokens as u64,
            completion_tokens: usage.completion_tokens as u64,
            total_tokens: usage.total_tokens as u64,
            request_count: 1,
        };

        let _ = sink.emit(AgentEvent::Done {
            final_message: Some(response.content.clone()),
            usage: Some(crate::agents::events::AgentUsage {
                provider: usage.provider.clone(),
                model: usage.model.clone(),
                prompt_tokens: usage.prompt_tokens as u64,
                completion_tokens: usage.completion_tokens as u64,
                total_tokens: usage.total_tokens as u64,
            }),
        })
        .await;

        Ok(AgentRunResult {
            answer: response.content,
            usage: Some(run_usage),
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::events::CollectingSink;

    // Helper to build a no-op LLM client for tests when needed.
    // For now we test that ChatAgent returns error when LLM is missing.
    #[tokio::test]
    async fn test_chat_agent_without_llm_returns_error() {
        let agent = ChatAgent::new(None, None);
        let sink = CollectingSink::new();
        let req = AgentRequest {
            kind: crate::agents::AgentKind::Chat,
            query: "hello".to_string(),
            notebook_id: None,
            session_id: None,
            doc_scope: vec![],
            messages: vec![],
            session_summary: None,
            user_preferences: None,
            language: None,
                docscope_metadata: None,
            debug: false,
            stream: false,
            auth_context: serde_json::json!({}),
            metadata: Default::default(),
            cancellation_token: None,
        };
        let result = agent.run(req, &sink).await;
        assert!(result.is_err());
        let events = sink.events();
        assert!(events.iter().any(|e| matches!(e, AgentEvent::Error { .. })));
    }

    #[test]
    fn build_chat_messages_includes_memory_context() {
        let req = AgentRequest {
            kind: crate::agents::AgentKind::Chat,
            query: "hello".to_string(),
            notebook_id: None,
            session_id: None,
            doc_scope: vec![],
            messages: vec![],
            session_summary: Some("Previously discussed pricing.".to_string()),
            user_preferences: Some(serde_json::json!({"preferred_answer_style": "concise"})),
            language: None,
            docscope_metadata: None,
            debug: false,
            stream: false,
            auth_context: serde_json::json!({}),
            metadata: Default::default(),
            cancellation_token: None,
        };

        let messages = build_chat_messages(&req);
        let system = &messages[0].content;

        assert!(system.contains("Previously discussed pricing."));
        assert!(system.contains("preferred_answer_style"));
    }
}
