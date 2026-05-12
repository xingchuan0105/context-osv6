use crate::ports::{
    chat::{chat_session_store::ChatSessionStore, rag_executor::RagExecutor},
    notebooks::notebook_store::NotebookStore,
    rate_limit::rate_limiter::RateLimiter,
};
use common::{AppError, CreateNotebookRequest};
use contracts::chat::{ChatRequest, ChatResponse};
use std::sync::Arc;

#[derive(Clone)]
pub struct ChatService {
    notebook_store: Arc<dyn NotebookStore>,
    session_store: Arc<dyn ChatSessionStore>,
    rate_limiter: Arc<dyn RateLimiter>,
    rag_executor: Arc<dyn RagExecutor>,
}

impl ChatService {
    pub fn new(
        notebook_store: Arc<dyn NotebookStore>,
        session_store: Arc<dyn ChatSessionStore>,
        rate_limiter: Arc<dyn RateLimiter>,
        rag_executor: Arc<dyn RagExecutor>,
    ) -> Self {
        Self {
            notebook_store,
            session_store,
            rate_limiter,
            rag_executor,
        }
    }

    pub async fn execute(&self, mut req: ChatRequest) -> Result<ChatResponse, AppError> {
        let decision = self
            .rate_limiter
            .check("chat-service")
            .await
            .map_err(|e| AppError::internal(format!("rate limiter failure: {}", e)))?;
        if !decision.allowed {
            return Err(AppError::validation(
                "rate_limit_exceeded",
                "rate limit exceeded",
            ));
        }

        if req.notebook_id.is_none() {
            let notebook = self
                .notebook_store
                .create_notebook(CreateNotebookRequest {
                    name: "Chat".into(),
                    description: String::new(),
                })
                .await?;
            req.notebook_id = Some(notebook.id);
        }

        let session = self.session_store.resolve(&req).await?;
        self.rag_executor.execute(&req, &session).await
    }
}
