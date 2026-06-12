use async_trait::async_trait;
use common::{AppError};
use contracts::notebooks::{ChatSession};
use contracts::chat::ChatRequest;

#[async_trait]
pub trait ChatSessionStore: Send + Sync {
    async fn resolve(&self, req: &ChatRequest) -> Result<ChatSession, AppError>;
}
