//! Product App — Conversation (single session-execute entry).
//!
//! Transport/MCP call **only** this App for chat/rag/search/write execution.
//! Internal dispatch is one place: write → WriteApp; else → AgentApp.

use common::AppError;
use contracts::auth_runtime::AuthContext;
use contracts::chat::{ChatEvent, ChatRequest, ChatResponse};
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use super::{AgentApp, WriteApp};

/// Single product entry for all conversation execute paths (POST + SSE).
pub struct ConversationApp<'a> {
    pub(crate) chat: &'a app_chat::ChatContext,
    pub(crate) auth: &'a AuthContext,
}

impl<'a> ConversationApp<'a> {
    fn agent(&self) -> AgentApp<'a> {
        AgentApp {
            chat: self.chat,
            auth: self.auth,
        }
    }

    fn write(&self) -> WriteApp<'a> {
        WriteApp {
            chat: self.chat,
            auth: self.auth,
        }
    }

    /// Non-streaming execute. Routes write vs agent once.
    pub async fn execute(&self, req: ChatRequest) -> Result<ChatResponse, AppError> {
        if WriteApp::is_write_agent_type(&req.agent_type) {
            self.write().execute(req).await
        } else {
            self.agent().execute_chat(req).await
        }
    }

    /// Streaming execute (SSE). Routes write vs agent once.
    pub async fn execute_stream(
        &self,
        req: ChatRequest,
        request_id: String,
        sender: UnboundedSender<ChatEvent>,
        token: CancellationToken,
    ) -> Result<(), AppError> {
        if WriteApp::is_write_agent_type(&req.agent_type) {
            self.write()
                .execute_stream(req, request_id, sender, token)
                .await
        } else {
            self.agent()
                .execute_chat_stream(req, request_id, sender, token)
                .await
        }
    }
}
