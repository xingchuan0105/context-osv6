//! Product App — Write mode control ring.
//!
//! **Iron rule (ADR-0007 T2):** `write_refine_*` and write refine control ops are **never**
//! registered in ReAct `ToolCatalog` / Chat-RAG-Search `tool_pool` dispatch.
//! Domain path: `app_chat::writer::run_write_mode` → write-core refine loop.
//! Product entry: **only** this App (transport/MCP must not call ChatContext write path
//! directly).

use common::AppError;
use contracts::auth_runtime::AuthContext;
use contracts::chat::{ChatEvent, ChatRequest, ChatResponse};
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

/// Product entry for writing tasks / refine loop. Independent of ReAct ToolCatalog.
pub struct WriteApp<'a> {
    pub(crate) chat: &'a app_chat::ChatContext,
    pub(crate) auth: &'a AuthContext,
}

impl<'a> WriteApp<'a> {
    pub fn auth(&self) -> &'a AuthContext {
        self.auth
    }

    /// Marker: write refine tools must not appear in ToolCatalog registration.
    pub const WRITE_REFINE_OUTSIDE_TOOL_CATALOG: bool = true;

    pub fn is_write_agent_type(agent_type: &str) -> bool {
        agent_type.eq_ignore_ascii_case("write")
    }

    fn require_write_request(req: &ChatRequest) -> Result<(), AppError> {
        if !Self::is_write_agent_type(&req.agent_type) {
            return Err(AppError::validation(
                "write_mode_required",
                "WriteApp only accepts agent_type=write",
            ));
        }
        Ok(())
    }

    /// Non-streaming write mode (product entry → chat pipeline → `run_write_mode`).
    pub async fn execute(&self, req: ChatRequest) -> Result<ChatResponse, AppError> {
        Self::require_write_request(&req)?;
        self.chat.execute_chat(req).await
    }

    /// Streaming write mode (SSE).
    pub async fn execute_stream(
        &self,
        req: ChatRequest,
        request_id: String,
        sender: UnboundedSender<ChatEvent>,
        token: CancellationToken,
    ) -> Result<(), AppError> {
        Self::require_write_request(&req)?;
        self.chat
            .execute_chat_stream(req, request_id, sender, token)
            .await
    }
}
