use app::ports::{
    chat::rag_executor::RagExecutor,
    notebooks::notebook_store::NotebookStore,
    rate_limit::rate_limiter::{RateLimitDecision, RateLimiter},
};
use app::services::chat::service::ChatService;
use async_trait::async_trait;
use common::{AppError, ChatSession, CreateNotebookRequest, Notebook};
use contracts::chat::{ChatRequest, ChatResponse, TraceInfo};
use std::sync::Arc;

#[derive(Clone, Default)]
struct FakeNotebookStore;

#[async_trait]
impl NotebookStore for FakeNotebookStore {
    async fn list_notebooks(&self) -> Result<Vec<Notebook>, AppError> {
        Ok(Vec::new())
    }

    async fn create_notebook(&self, req: CreateNotebookRequest) -> Result<Notebook, AppError> {
        Ok(Notebook {
            id: "nb-1".into(),
            org_id: "org-1".into(),
            owner_id: "user-1".into(),
            name: req.name.clone(),
            title: req.name,
            description: req.description,
            created_at: "now".into(),
            updated_at: "now".into(),
            document_count: 0,
            status_summary: std::collections::HashMap::new(),
            shared: false,
        })
    }
}

#[derive(Clone, Default)]
struct FakeRateLimiter;

#[async_trait]
impl RateLimiter for FakeRateLimiter {
    async fn check(&self, _key: &str) -> anyhow::Result<RateLimitDecision> {
        Ok(RateLimitDecision {
            allowed: true,
            remaining: 9,
            limit: 10,
        })
    }
}

#[derive(Clone, Default)]
struct FakeRagExecutor;

#[async_trait]
impl RagExecutor for FakeRagExecutor {
    async fn execute(
        &self,
        req: &ChatRequest,
        _session: &ChatSession,
    ) -> Result<ChatResponse, AppError> {
        Ok(ChatResponse {
            session_id: "session-1".into(),
            answer: format!("echo: {}", req.query),
            agent_type: req.agent_type.clone(),
            citations: Vec::new(),
            sources: Vec::new(),
            planner_output: None,
            mode_debug: None,
            message_id: Some(1),
            trace: TraceInfo {
                mode: "general".into(),
            },
            degrade_trace: Vec::new(),
            answer_blocks: Vec::new(),
            guard_report: None,
        })
    }
}

#[tokio::test]
async fn chat_service_executes_against_ports() {
    let service = ChatService::new(
        Arc::new(FakeNotebookStore),
        Arc::new(app::services::chat::session::MemoryChatSessionStore::default()),
        Arc::new(FakeRateLimiter),
        Arc::new(FakeRagExecutor),
    );

    let response = service
        .execute(ChatRequest {
            query: "say hello".into(),
            notebook_id: None,
            session_id: None,
            agent_type: "general".into(),
            source_type: None,
            source_token: None,
            doc_scope: Vec::new(),
            messages: Vec::new(),
            stream: false,
            language: None,
        })
        .await
        .unwrap();

    assert_eq!(response.agent_type, "general");
    assert!(response.answer.contains("hello"));
}
