use async_trait::async_trait;
use common::AppError;
use contracts::chat::ChatRequest;
use contracts::notebooks::ChatSession;

#[async_trait]
pub trait ChatSessionStore: Send + Sync {
    async fn resolve(&self, req: &ChatRequest) -> Result<ChatSession, AppError>;
}
