//! Product App — Agent (Chat / RAG / Search).
//!
//! Tools execute only via `ToolCatalog` / `dispatch_tool`. Write is **not** this App
//! (see [`super::WriteApp`]).

use common::{AppError, SourceRow, StatusOnlyResponse};
use contracts::auth_runtime::AuthContext;
use contracts::chat::{ChatEvent, ChatMessage, ChatRequest, ChatResponse};
use contracts::documents::CitationLookupResponse;
use contracts::workspaces::{
    ChatSession, CreateChatSessionRequest, UpdateChatSessionRequest, Workspace,
};
use contracts::{RuntimeExecuteRequest, RuntimeExecuteResponse};
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

/// Product entry for chat / RAG / web-search sessions and related session APIs.
pub struct AgentApp<'a> {
    pub(crate) chat: &'a app_chat::ChatContext,
    pub(crate) auth: &'a AuthContext,
}

impl<'a> AgentApp<'a> {
    pub fn auth(&self) -> &'a AuthContext {
        self.auth
    }

    /// Raw chat context (infra / rare escapes). Prefer AgentApp methods for product work.
    pub fn chat(&self) -> &'a app_chat::ChatContext {
        self.chat
    }

    pub fn is_write_agent_type(agent_type: &str) -> bool {
        agent_type.eq_ignore_ascii_case("write")
    }

    /// Non-streaming Chat/RAG/Search. Write requests must use [`super::WriteApp::execute`].
    pub async fn execute_chat(&self, req: ChatRequest) -> Result<ChatResponse, AppError> {
        if Self::is_write_agent_type(&req.agent_type) {
            return Err(AppError::validation(
                "use_write_app",
                "write mode must enter via WriteApp, not AgentApp",
            ));
        }
        self.chat.execute_chat(req).await
    }

    /// Streaming Chat/RAG/Search (SSE). Write must use [`super::WriteApp::execute_stream`].
    pub async fn execute_chat_stream(
        &self,
        req: ChatRequest,
        request_id: String,
        sender: UnboundedSender<ChatEvent>,
        token: CancellationToken,
    ) -> Result<(), AppError> {
        if Self::is_write_agent_type(&req.agent_type) {
            return Err(AppError::validation(
                "use_write_app",
                "write mode must enter via WriteApp, not AgentApp",
            ));
        }
        self.chat
            .execute_chat_stream(req, request_id, sender, token)
            .await
    }

    pub async fn execute_runtime_tools(
        &self,
        req: RuntimeExecuteRequest,
    ) -> Result<RuntimeExecuteResponse, AppError> {
        self.chat.execute_runtime_tools(req).await
    }

    pub async fn search(
        &self,
        pattern: &str,
    ) -> (Vec<Workspace>, Vec<ChatSession>, Vec<SourceRow>) {
        self.chat.search(pattern).await
    }

    pub async fn list_sessions(&self, workspace_id: Option<&str>) -> Vec<ChatSession> {
        self.chat.list_sessions(workspace_id).await
    }

    pub async fn create_session(
        &self,
        req: CreateChatSessionRequest,
    ) -> Result<ChatSession, AppError> {
        self.chat.create_session(req).await
    }

    pub async fn get_session(&self, session_id: &str) -> Option<ChatSession> {
        self.chat.get_session(session_id).await
    }

    pub async fn update_session(
        &self,
        session_id: &str,
        req: UpdateChatSessionRequest,
    ) -> Result<ChatSession, AppError> {
        self.chat.update_session(session_id, req).await
    }

    pub async fn delete_session(&self, session_id: &str) -> Result<StatusOnlyResponse, AppError> {
        self.chat.delete_session(session_id).await
    }

    pub async fn list_messages(&self, session_id: &str) -> Result<Vec<ChatMessage>, AppError> {
        self.chat.list_messages(session_id).await
    }

    pub async fn lookup_citation(
        &self,
        session_id: &str,
        message_id: i64,
        citation_id: i64,
    ) -> Result<CitationLookupResponse, AppError> {
        self.chat
            .lookup_citation(session_id, message_id, citation_id)
            .await
    }

    pub async fn get_citation_asset(
        &self,
        asset_id: &str,
    ) -> Result<(Vec<u8>, String), AppError> {
        self.chat.get_citation_asset(asset_id).await
    }
}
