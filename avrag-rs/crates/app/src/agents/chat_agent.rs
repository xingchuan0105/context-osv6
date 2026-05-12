use crate::agents::agent_loop::{AgentLoopConfig, AgentLoopOutcome, run_agent_loop};
use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::react_loop::{LoopBudget, UserTier};
use crate::agents::runtime::{Agent, AgentRequest, AgentRunResult, AgentRunUsage};
use crate::agents::tool_registry::{AgentToolRegistry, PlaceholderTool};
use common::AppError;

/// ChatAgent handles direct (non-RAG, non-search) conversational queries.
///
/// It uses the generic [`run_agent_loop`] with a minimal tool set
/// (`load_skill`, `compact_history`) so the model can self-improve its
/// context when needed.
pub struct ChatAgent {
    llm_client: Option<avrag_llm::LlmClient>,
    temperature: Option<f32>,
    registry: AgentToolRegistry,
}

impl ChatAgent {
    pub fn new(llm_client: Option<avrag_llm::LlmClient>, temperature: Option<f32>) -> Self {
        let mut registry = AgentToolRegistry::new();
        registry.register(Box::new(PlaceholderTool::load_skill()));
        registry.register(Box::new(PlaceholderTool::compact_history()));
        Self {
            llm_client,
            temperature,
            registry,
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

        let config = AgentLoopConfig {
            llm,
            temperature: self.temperature,
            system_prompt: messages[0].content.clone(),
            messages: messages.into_iter().skip(1).collect(),
            registry: &self.registry,
            budget: LoopBudget::chat(UserTier::Pro),
            cancellation: request.cancellation_token.clone().unwrap_or_default(),
            trace_id: request.session_id.clone().unwrap_or_else(|| "chat-agent".to_string()),
        };

        let outcome = run_agent_loop(config, sink).await?;

        match outcome {
            AgentLoopOutcome::Answer(answer) => {
                // Emit usage and done events
                let _ = sink.emit(AgentEvent::Done {
                    final_message: Some(answer.clone()),
                    usage: None,
                })
                .await;

                Ok(AgentRunResult {
                    answer,
                    usage: None,
                    ..Default::default()
                })
            }
            AgentLoopOutcome::Degraded { reason, partial_answer } => {
                let _ = sink.emit(AgentEvent::Error {
                    code: "chat_degraded".to_string(),
                    message: format!("Chat degraded: {reason:?}"),
                })
                .await;
                Ok(AgentRunResult {
                    answer: partial_answer.unwrap_or_else(|| {
                        "I'm sorry, I couldn't process your request.".to_string()
                    }),
                    usage: None,
                    final_decision: Some(crate::agents::runtime::FinalDecision::Degraded { reason }),
                    ..Default::default()
                })
            }
            AgentLoopOutcome::Clarify(question) => {
                let _ = sink.emit(AgentEvent::Done {
                    final_message: Some(question.clone()),
                    usage: None,
                })
                .await;
                Ok(AgentRunResult {
                    answer: question,
                    usage: None,
                    ..Default::default()
                })
            }
        }
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
