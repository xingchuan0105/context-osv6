use crate::ports::chat::chat_session_store::ChatSessionStore;
use async_trait::async_trait;
use common::{AppError, now_rfc3339};
use contracts::chat::ChatRequest;
use contracts::notebooks::ChatSession;

#[derive(Clone, Default)]
pub struct MemoryChatSessionStore;

#[async_trait]
impl ChatSessionStore for MemoryChatSessionStore {
    async fn resolve(&self, req: &ChatRequest) -> Result<ChatSession, AppError> {
        Ok(ChatSession {
            id: req
                .session_id
                .clone()
                .unwrap_or_else(|| "session-1".to_string()),
            notebook_id: req
                .notebook_id
                .clone()
                .unwrap_or_else(|| "nb-1".to_string()),
            title: Some("Chat".to_string()),
            agent_type: req.agent_type.clone(),
            pinned: false,
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
        })
    }
}
