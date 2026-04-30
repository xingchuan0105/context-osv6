use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::runtime::{Agent, AgentRequest, AgentRunResult};
use common::AppError;
use std::sync::Arc;

/// RagAgent handles retrieval-augmented generation queries.
///
/// RAG production orchestration still lives in GraphFlow. Until this agent is
/// wired as that GraphFlow adapter, it must fail explicitly instead of
/// returning a fake successful answer.
pub struct RagAgent {
    rag_runtime: Option<Arc<avrag_rag_core::RagRuntime>>,
}

impl RagAgent {
    pub fn new(rag_runtime: Option<Arc<avrag_rag_core::RagRuntime>>) -> Self {
        Self { rag_runtime }
    }
}

#[async_trait::async_trait]
impl Agent for RagAgent {
    async fn run(
        &self,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, AppError> {
        if request.doc_scope.is_empty() {
            sink.emit(AgentEvent::Error {
                code: "missing_doc_scope".to_string(),
                message: "RAG mode requires a non-empty doc_scope".to_string(),
            })
            .await;
            return Err(AppError::validation(
                "missing_doc_scope",
                "RAG mode requires a non-empty doc_scope",
            ));
        }

        let Some(ref _rag) = self.rag_runtime else {
            sink.emit(AgentEvent::Error {
                code: "rag_unavailable".to_string(),
                message: "RAG runtime is not configured".to_string(),
            })
            .await;
            return Err(AppError::internal("RAG runtime is not configured"));
        };

        sink.emit(AgentEvent::Error {
            code: "rag_agent_not_wired".to_string(),
            message: "RAG mode is served by GraphFlow until RagAgent is wired as its adapter"
                .to_string(),
        })
        .await;
        Err(AppError::internal(
            "RagAgent is not wired to the GraphFlow RAG production path",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::events::CollectingSink;

    #[tokio::test]
    async fn test_rag_agent_rejects_empty_doc_scope() {
        let agent = RagAgent::new(None);
        let sink = CollectingSink::new();
        let req = AgentRequest {
            kind: crate::agents::AgentKind::Rag,
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
        let result = agent.run(req, &sink).await;
        assert!(result.is_err());
        let events = sink.events();
        assert!(events.iter().any(|e| matches!(
            e,
            AgentEvent::Error { code, .. } if code == "missing_doc_scope"
        )));
    }

    #[tokio::test]
    async fn test_rag_agent_without_runtime_returns_error() {
        let agent = RagAgent::new(None);
        let sink = CollectingSink::new();
        let req = AgentRequest {
            kind: crate::agents::AgentKind::Rag,
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
        let result = agent.run(req, &sink).await;
        assert!(result.is_err());
        let events = sink.events();
        assert!(events.iter().any(|e| matches!(
            e,
            AgentEvent::Error { code, .. } if code == "rag_unavailable"
        )));
    }
}
