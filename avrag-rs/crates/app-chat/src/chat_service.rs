use crate::context::ChatContext;
use common::{AppError, ChatRequest, ChatResponse};

#[derive(Clone)]
pub struct ChatService {
    state: ChatContext,
}

impl ChatService {
    pub fn new(state: ChatContext) -> Self {
        Self { state }
    }

    pub async fn execute(&self, req: ChatRequest) -> Result<ChatResponse, AppError> {
        self.state.execute_chat_pipeline(req).await
    }
}
