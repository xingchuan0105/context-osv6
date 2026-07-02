use crate::agents::runtime::{Agent, AgentRequest, AgentRunResult};
use common::AppError;

/// Thin wrapper around any [`Agent`] implementation.
///
/// Production uses [`crate::agents::unified::UnifiedAgent`];
/// tests may inject custom `dyn Agent` implementations.
pub struct UnifiedAgentService {
    agent: Box<dyn Agent>,
}

impl UnifiedAgentService {
    pub fn new(agent: Box<dyn Agent>) -> Self {
        Self { agent }
    }

    #[tracing::instrument(skip(self, sink), fields(agent_kind = ?request.kind))]
    pub async fn run(
        &self,
        request: AgentRequest,
        sink: &dyn crate::agents::events::AgentEventSink,
    ) -> Result<AgentRunResult, AppError> {
        self.agent.run(request, sink).await
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
            sink: &dyn crate::agents::events::AgentEventSink,
        ) -> Result<AgentRunResult, AppError> {
            sink.emit(AgentEvent::Activity {
                stage: self.0.to_string(),
                message: "running".to_string(),
            })
            .await;
            let query = request.query.clone();
            sink.emit(AgentEvent::MessageDelta {
                text: query.clone(),
            })
            .await;
            sink.emit(AgentEvent::Done {
                final_message: Some(query),
                usage: None,
            })
            .await;
            Ok(AgentRunResult {
                answer: request.query.clone(),
                ..Default::default()
            })
        }
    }

    #[tokio::test]
    async fn test_service_routes_chat() {
        let svc = UnifiedAgentService::new(Box::new(EchoAgent("chat")));
        let sink = CollectingSink::new();
        let req = AgentRequest {
            kind: crate::agents::AgentKind::Chat,
            query: "hello".to_string(),
            notebook_id: None,
            session_id: None,
            doc_scope: vec![],
            messages: vec![],
            user_preferences: None,
            debug: false,
            stream: false,
            language: None,
            auth_context: serde_json::json!({}),
            docscope_metadata: None,
            metadata: Default::default(),
            cancellation_token: None,
            guard_pipeline: None,
            preferred_tools: vec![],
            format_hint: None,
            max_iterations: None,
        };
        let result = svc.run(req, &sink).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().answer, "hello");
    }

    #[tokio::test]
    async fn test_service_routes_search() {
        let svc = UnifiedAgentService::new(Box::new(EchoAgent("search")));
        let sink = CollectingSink::new();
        let req = AgentRequest {
            kind: crate::agents::AgentKind::Search,
            query: "q".to_string(),
            notebook_id: None,
            session_id: None,
            doc_scope: vec![],
            messages: vec![],
            user_preferences: None,
            debug: false,
            stream: false,
            language: None,
            auth_context: serde_json::json!({}),
            docscope_metadata: None,
            metadata: Default::default(),
            cancellation_token: None,
            guard_pipeline: None,
            preferred_tools: vec![],
            format_hint: None,
            max_iterations: None,
        };
        let result = svc.run(req, &sink).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().answer, "q");
    }

    #[tokio::test]
    async fn test_service_routes_rag() {
        let svc = UnifiedAgentService::new(Box::new(EchoAgent("rag")));
        let sink = CollectingSink::new();
        let req = AgentRequest {
            kind: crate::agents::AgentKind::Rag,
            query: "q".to_string(),
            notebook_id: None,
            session_id: None,
            doc_scope: vec!["doc1".to_string()],
            messages: vec![],
            user_preferences: None,
            debug: false,
            stream: false,
            language: None,
            auth_context: serde_json::json!({}),
            docscope_metadata: None,
            metadata: Default::default(),
            cancellation_token: None,
            guard_pipeline: None,
            preferred_tools: vec![],
            format_hint: None,
            max_iterations: None,
        };
        let result = svc.run(req, &sink).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().answer, "q");
    }
}
