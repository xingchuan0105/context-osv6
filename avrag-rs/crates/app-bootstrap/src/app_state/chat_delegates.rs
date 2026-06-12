use common::{AppError, SourceRow, StatusOnlyResponse};
use contracts::chat::{ChatMessage, ChatRequest, ChatResponse};
use contracts::notebooks::{ChatSession, CreateChatSessionRequest, Notebook, UpdateChatSessionRequest};
use contracts::chat::ChatEvent;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use super::AppState;

impl AppState {
    pub(crate) fn chat_ctx(&self) -> &app_chat::ChatContext {
        &self.chat
    }

    pub async fn search(&self, pattern: &str) -> (Vec<Notebook>, Vec<ChatSession>, Vec<SourceRow>) {
        self.chat_ctx().search(pattern).await
    }

    pub async fn list_sessions(&self, notebook_id: Option<&str>) -> Vec<ChatSession> {
        self.chat_ctx().list_sessions(notebook_id).await
    }

    pub async fn create_session(
        &self,
        req: CreateChatSessionRequest,
    ) -> Result<ChatSession, AppError> {
        self.chat_ctx().create_session(req).await
    }

    pub async fn update_session(
        &self,
        session_id: &str,
        req: UpdateChatSessionRequest,
    ) -> Result<ChatSession, AppError> {
        self.chat_ctx().update_session(session_id, req).await
    }

    pub async fn get_session(&self, session_id: &str) -> Option<ChatSession> {
        self.chat_ctx().get_session(session_id).await
    }

    pub async fn delete_session(&self, session_id: &str) -> Result<StatusOnlyResponse, AppError> {
        self.chat_ctx().delete_session(session_id).await
    }

    pub async fn list_messages(&self, session_id: &str) -> Result<Vec<ChatMessage>, AppError> {
        self.chat_ctx().list_messages(session_id).await
    }

    pub async fn execute_chat(&self, req: ChatRequest) -> Result<ChatResponse, AppError> {
        self.chat_ctx().execute_chat(req).await
    }

    pub async fn execute_chat_stream(
        &self,
        req: ChatRequest,
        request_id: String,
        sender: UnboundedSender<ChatEvent>,
        token: CancellationToken,
    ) -> Result<(), AppError> {
        self.chat_ctx()
            .execute_chat_stream(req, request_id, sender, token)
            .await
    }

    pub async fn execute_rag_execute_plan(
        &self,
        req: common::ExecutePlanRequest,
    ) -> Result<common::ExecutePlanResponse, AppError> {
        self.chat_ctx().execute_rag_execute_plan(req).await
    }

    pub async fn execute_runtime_tools(
        &self,
        req: common::RuntimeExecuteRequest,
    ) -> Result<common::RuntimeExecuteResponse, AppError> {
        self.chat_ctx().execute_runtime_tools(req).await
    }

    pub async fn get_user_usage_limit(
        &self,
    ) -> Result<avrag_billing::usage_limit::UsageLimitResponse, AppError> {
        self.chat_ctx().get_user_usage_limit().await
    }

    pub async fn check_user_quota(
        &self,
    ) -> Result<avrag_billing::usage_limit::QuotaCheckResult, AppError> {
        self.chat_ctx().check_user_quota().await
    }

    pub(crate) async fn validate_rag_doc_scope(
        &self,
        doc_scope: &[String],
    ) -> Result<(), AppError> {
        self.chat_ctx().validate_rag_doc_scope(doc_scope).await
    }

    pub(crate) fn memory_session_visible(
        &self,
        state: &app_core::MemoryState,
        session: &contracts::notebooks::ChatSession,
    ) -> bool {
        self.chat_ctx().memory_session_visible(state, session)
    }

    pub async fn execute_chat_pipeline(
        &self,
        req: contracts::chat::ChatRequest,
    ) -> Result<contracts::chat::ChatResponse, common::AppError> {
        self.chat_ctx().execute_chat_pipeline(req).await
    }

    pub async fn load_docscope_metadata(
        &self,
        doc_scope: &[String],
    ) -> Result<common::DocScopeMetadata, common::AppError> {
        self.chat_ctx().load_docscope_metadata(doc_scope).await
    }

    pub async fn build_session_context(
        &self,
        session: &contracts::notebooks::ChatSession,
    ) -> Result<Option<avrag_rag_core::context::SessionContext>, common::AppError> {
        self.chat_ctx().build_session_context(session).await
    }

    pub async fn resolve_agent_messages(
        &self,
        req: &contracts::chat::ChatRequest,
    ) -> Vec<contracts::chat::ChatTurnInput> {
        self.chat_ctx().resolve_agent_messages(req).await
    }

    pub async fn build_agent_request(
        &self,
        req: &contracts::chat::ChatRequest,
        kind: app_chat::AgentKind,
        session_id_override: Option<String>,
    ) -> app_chat::agents::runtime::AgentRequest {
        self.chat_ctx()
            .build_agent_request(req, kind, session_id_override)
            .await
    }

    pub(crate) fn build_general_agent_debug(
        &self,
        agent_request: &app_chat::agents::runtime::AgentRequest,
    ) -> std::collections::BTreeMap<String, serde_json::Value> {
        self.chat_ctx().build_general_agent_debug(agent_request)
    }

    pub fn build_rag_session_context(
        messages: Vec<contracts::chat::ChatMessage>,
        summary: Option<String>,
    ) -> Option<avrag_rag_core::context::SessionContext> {
        app_chat::ChatContext::build_rag_session_context(messages, summary)
    }
}
