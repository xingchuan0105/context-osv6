//! Product App — Conversation (single session-execute entry).
//!
//! Transport/MCP call **only** this App for chat/rag/search/write execution.
//! Lane decision is once here; ChatContext pipelines own the rest.

use common::AppError;
use contracts::auth_runtime::AuthContext;
use contracts::chat::{ChatEvent, ChatRequest, ChatResponse};
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

/// Single product entry for all conversation execute paths (POST + SSE).
pub struct ConversationApp<'a> {
    pub(crate) chat: &'a app_chat::ChatContext,
    #[allow(dead_code)]
    pub(crate) auth: &'a AuthContext,
}

impl<'a> ConversationApp<'a> {
    /// Non-streaming execute. Sole product-level write/agent routing.
    pub async fn execute(&self, req: ChatRequest) -> Result<ChatResponse, AppError> {
        if app_chat::is_write_agent_type(&req.agent_type) {
            self.chat.execute_write(req).await
        } else {
            self.chat.execute_chat(req).await
        }
    }

    /// Streaming execute (SSE). Sole product-level write/agent routing.
    pub async fn execute_stream(
        &self,
        req: ChatRequest,
        request_id: String,
        sender: UnboundedSender<ChatEvent>,
        token: CancellationToken,
    ) -> Result<(), AppError> {
        if app_chat::is_write_agent_type(&req.agent_type) {
            self.chat
                .execute_write_stream(req, request_id, sender, token)
                .await
        } else {
            self.chat
                .execute_chat_stream(req, request_id, sender, token)
                .await
        }
    }
}
