//! Product App — Write mode control ring.
//!
//! **Iron rule (ADR-0007 T2):** `write_refine_*` never in ReAct ToolCatalog.
//! Domain path: `ChatContext::execute_write` → write pipeline → `run_write_mode`.

use common::AppError;
use contracts::chat::{ChatEvent, ChatRequest, ChatResponse};
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

/// Product entry for writing tasks / refine loop.
pub struct WriteApp<'a> {
    pub(crate) chat: &'a app_chat::ChatContext,
    #[allow(dead_code)] // reserved for future write-scoped auth checks
    pub(crate) auth: &'a contracts::auth_runtime::AuthContext,
}

impl<'a> WriteApp<'a> {
    pub fn is_write_agent_type(agent_type: &str) -> bool {
        app_chat::is_write_agent_type(agent_type)
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

    /// Non-streaming write (product entry → write pipeline, not agent lane).
    pub async fn execute(&self, req: ChatRequest) -> Result<ChatResponse, AppError> {
        Self::require_write_request(&req)?;
        self.chat.execute_write(req).await
    }

    /// Streaming write (SSE).
    pub async fn execute_stream(
        &self,
        req: ChatRequest,
        request_id: String,
        sender: UnboundedSender<ChatEvent>,
        token: CancellationToken,
    ) -> Result<(), AppError> {
        Self::require_write_request(&req)?;
        self.chat
            .execute_write_stream(req, request_id, sender, token)
            .await
    }
}
