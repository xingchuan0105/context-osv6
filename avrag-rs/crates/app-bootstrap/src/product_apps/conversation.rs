//! Product App — Conversation (single session-execute entry).
//!
//! Transport/MCP call **only** this App for chat/rag/search execution.
//! Product write is hard-disabled here; agent chat lane owns the rest.

use common::AppError;
use contracts::chat::{ChatEvent, ChatRequest, ChatResponse};
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

/// Single product entry for all conversation execute paths (POST + SSE).
pub struct ConversationApp<'a> {
    pub(crate) chat: &'a app_chat::ChatContext,
}

impl<'a> ConversationApp<'a> {
    /// Reject internal-only agent_type strings at the product boundary.
    fn validate_user_agent_type(agent_type: &str) -> Result<(), AppError> {
        if app_chat::is_reserved_internal_agent_type(agent_type) {
            return Err(AppError::validation(
                "write_refine_not_user_mode",
                "write_refine is an internal control ring and is not available as a user agent_type",
            ));
        }
        Ok(())
    }

    /// Non-streaming execute. Product boundary rejects write; always agent chat lane.
    pub async fn execute(&self, req: ChatRequest) -> Result<ChatResponse, AppError> {
        Self::validate_user_agent_type(&req.agent_type)?;
        // Reject product write
        if app_chat::is_write_agent_type(&req.agent_type) {
            return Err(app_chat::write_disabled_error());
        }
        // Also resolve to catch issues early (write already rejected above)
        app_chat::resolve_capabilities(req.capabilities.as_deref(), &req.agent_type)?;
        // Always agent chat lane from product boundary
        self.chat.execute_chat(req).await
    }

    /// Streaming execute (SSE). Product boundary rejects write; always agent chat lane.
    pub async fn execute_stream(
        &self,
        req: ChatRequest,
        request_id: String,
        sender: UnboundedSender<ChatEvent>,
        token: CancellationToken,
    ) -> Result<(), AppError> {
        Self::validate_user_agent_type(&req.agent_type)?;
        // Reject product write
        if app_chat::is_write_agent_type(&req.agent_type) {
            return Err(app_chat::write_disabled_error());
        }
        // Also resolve to catch issues early (write already rejected above)
        app_chat::resolve_capabilities(req.capabilities.as_deref(), &req.agent_type)?;
        // Always agent chat lane from product boundary
        self.chat
            .execute_chat_stream(req, request_id, sender, token)
            .await
    }
}
