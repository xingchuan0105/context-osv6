//! Product App — Agent (sessions / search / citations / runtime tools).
//! **Execute** (chat/rag/search/write) goes through ConversationApp only.

use common::{AppError, SourceRow, StatusOnlyResponse};
use contracts::chat::ChatMessage;
use contracts::documents::CitationLookupResponse;
use contracts::workspaces::{
    ChatSession, CreateChatSessionRequest, UpdateChatSessionRequest, Workspace,
};
use contracts::{RuntimeExecuteRequest, RuntimeExecuteResponse};

/// Product entry for sessions, search, citations, and runtime tools (not conversation execute).
pub struct AgentApp<'a> {
    pub(crate) chat: &'a app_chat::ChatContext,
}

impl<'a> AgentApp<'a> {
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

    /// User usage limit (product path; replaces raw `state.chat().get_user_usage_limit`).
    pub async fn get_user_usage_limit(
        &self,
    ) -> Result<avrag_billing::usage_limit::UsageLimitResponse, AppError> {
        self.chat.get_user_usage_limit().await
    }
}
