use crate::agents::AgentKind;
use crate::agents::events::AgentEventSink;
use crate::agents::runtime::{Agent, AgentRequest, AgentRunResult};
use common::AppError;

/// Unified dispatcher for direct agent execution.
///
/// Owns the concrete agent implementations and routes requests based on
/// `AgentRequest.kind`. Chat and Web Search production chat paths execute
/// through this service. RAG production orchestration still lives in GraphFlow;
/// the direct `RagAgent` branch fails closed until it is wired as that adapter.
///
/// # Usage
///
/// ```ignore
/// let service = UnifiedAgentService::new(chat_agent, search_agent, rag_agent);
/// let result = service.run(request, &sink).await?;
/// ```
pub struct UnifiedAgentService {
    chat: Box<dyn Agent>,
    search: Box<dyn Agent>,
    rag: Box<dyn Agent>,
}

impl UnifiedAgentService {
    /// Build the service from three agent implementations.
    pub fn new(chat: Box<dyn Agent>, search: Box<dyn Agent>, rag: Box<dyn Agent>) -> Self {
        Self { chat, search, rag }
    }

    /// Run the agent that matches `request.kind`.
    ///
    /// The `sink` receives progress events (`AgentEvent`) during execution.
    /// For streaming paths these events are forwarded immediately over SSE;
    /// for non-streaming paths a `CollectingSink` can be used and the final
    /// `AgentRunResult` assembled from the collected events.
    pub async fn run(
        &self,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, AppError> {
        match request.kind {
            AgentKind::Chat => self.chat.run(request, sink).await,
            AgentKind::Search => self.search.run(request, sink).await,
            AgentKind::Rag => self.rag.run(request, sink).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::events::{AgentEvent, CollectingSink};
    use async_trait::async_trait;

    struct EchoAgent(&'static str);

    #[async_trait]
    impl Agent for EchoAgent {
        async fn run(
            &self,
            request: AgentRequest,
            sink: &dyn AgentEventSink,
        ) -> Result<AgentRunResult, AppError> {
            sink.emit(AgentEvent::Activity {
                stage: self.0.to_string(),
                message: "running".to_string(),
            })
            .await;
            sink.emit(AgentEvent::MessageDelta {
                text: request.query.clone(),
            })
            .await;
            sink.emit(AgentEvent::Done {
                final_message: Some(request.query),
                usage: None,
            })
            .await;
            Ok(AgentRunResult::default())
        }
    }

    #[tokio::test]
    async fn test_service_routes_chat() {
        let svc = UnifiedAgentService::new(
            Box::new(EchoAgent("chat")),
            Box::new(EchoAgent("search")),
            Box::new(EchoAgent("rag")),
        );
        let sink = CollectingSink::new();
        let req = AgentRequest {
            kind: AgentKind::Chat,
            query: "hello".to_string(),
            notebook_id: None,
            session_id: None,
            doc_scope: vec![],
            messages: vec![],
            session_summary: None,
            user_preferences: None,
            working_memory: None,
            debug: false,
            stream: false,
            auth_context: serde_json::json!({}),
            metadata: Default::default(),
        };
        let _ = svc.run(req, &sink).await.unwrap();
        let events = sink.events();
        assert_eq!(events.len(), 3);
        assert!(matches!(&events[0], AgentEvent::Activity { stage, .. } if stage == "chat"));
    }

    #[tokio::test]
    async fn test_service_routes_search() {
        let svc = UnifiedAgentService::new(
            Box::new(EchoAgent("chat")),
            Box::new(EchoAgent("search")),
            Box::new(EchoAgent("rag")),
        );
        let sink = CollectingSink::new();
        let req = AgentRequest {
            kind: AgentKind::Search,
            query: "q".to_string(),
            notebook_id: None,
            session_id: None,
            doc_scope: vec![],
            messages: vec![],
            session_summary: None,
            user_preferences: None,
            working_memory: None,
            debug: false,
            stream: false,
            auth_context: serde_json::json!({}),
            metadata: Default::default(),
        };
        let _ = svc.run(req, &sink).await.unwrap();
        let events = sink.events();
        assert!(matches!(&events[0], AgentEvent::Activity { stage, .. } if stage == "search"));
    }

    #[tokio::test]
    async fn test_service_routes_rag() {
        let svc = UnifiedAgentService::new(
            Box::new(EchoAgent("chat")),
            Box::new(EchoAgent("search")),
            Box::new(EchoAgent("rag")),
        );
        let sink = CollectingSink::new();
        let req = AgentRequest {
            kind: AgentKind::Rag,
            query: "q".to_string(),
            notebook_id: None,
            session_id: None,
            doc_scope: vec!["doc1".to_string()],
            messages: vec![],
            session_summary: None,
            user_preferences: None,
            working_memory: None,
            debug: false,
            stream: false,
            auth_context: serde_json::json!({}),
            metadata: Default::default(),
        };
        let _ = svc.run(req, &sink).await.unwrap();
        let events = sink.events();
        assert!(matches!(&events[0], AgentEvent::Activity { stage, .. } if stage == "rag"));
    }
}
