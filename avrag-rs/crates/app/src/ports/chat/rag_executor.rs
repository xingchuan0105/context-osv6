use async_trait::async_trait;
use common::{AppError};
use contracts::notebooks::{ChatSession};
use contracts::chat::{ChatRequest, ChatResponse};

#[async_trait]
pub trait RagExecutor: Send + Sync {
    async fn execute(
        &self,
        req: &ChatRequest,
        session: &ChatSession,
    ) -> Result<ChatResponse, AppError>;
}
