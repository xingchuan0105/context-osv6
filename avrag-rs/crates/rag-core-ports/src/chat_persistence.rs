use async_trait::async_trait;
use common::{AppError, SourceRow};
use contracts::chat::ChatMessage;
use contracts::notebooks::{ChatSession, Notebook};
use contracts::{AuthContext, ToolResult};
use ingestion_types::AuditRecord;
use uuid::Uuid;

use crate::port_rows::{
    ConversationHistoryHit, ConversationHistoryScope, DocumentAssetRow, MultimodalChunkRow,
    NotificationCreateParams, UserProfileRow,
};

/// User + assistant turn persisted as one atomic chat write.
pub struct AppendChatTurn<'a> {
    pub user_content: &'a str,
    pub assistant_content: &'a str,
    pub assistant_answer_blocks: &'a [contracts::chat::AnswerBlock],
    pub agent_type: &'a str,
    pub citations: &'a [contracts::chat::Citation],
    pub tool_results: &'a [ToolResult],
    pub user_turn_metadata: Option<serde_json::Value>,
    pub user_resolved_query: Option<&'a str>,
}

/// Chat/session persistence boundary — implementations live in storage adapters (PG).
#[async_trait]
pub trait ChatPersistencePort: Send + Sync {
    async fn search_notebooks(
        &self,
        auth: &AuthContext,
        pattern: &str,
    ) -> Result<Vec<Notebook>, AppError>;

    async fn search_sessions(
        &self,
        auth: &AuthContext,
        pattern: &str,
    ) -> Result<Vec<ChatSession>, AppError>;

    async fn search_sources(
        &self,
        auth: &AuthContext,
        pattern: &str,
    ) -> Result<Vec<SourceRow>, AppError>;

    async fn list_sessions(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
    ) -> Result<Vec<ChatSession>, AppError>;

    async fn get_session(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
    ) -> Result<Option<ChatSession>, AppError>;

    async fn create_session(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        title: Option<&str>,
        agent_type: &str,
    ) -> Result<ChatSession, AppError>;

    async fn update_session(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
        title: Option<&str>,
        pinned: Option<bool>,
    ) -> Result<Option<ChatSession>, AppError>;

    async fn delete_session(&self, auth: &AuthContext, session_id: Uuid) -> Result<bool, AppError>;

    async fn list_messages(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
    ) -> Result<Vec<ChatMessage>, AppError>;

    async fn get_message(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
        message_id: i64,
    ) -> Result<Option<ChatMessage>, AppError>;

    async fn append_chat_turn(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
        turn: AppendChatTurn<'_>,
    ) -> Result<i64, AppError>;

    async fn get_notebook(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
    ) -> Result<Option<Notebook>, AppError>;

    async fn get_user_profile(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
    ) -> Result<Option<UserProfileRow>, AppError>;

    async fn search_conversation_history(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
        query: &str,
        scope: ConversationHistoryScope,
        limit: i64,
        exclude_message_ids: &[i64],
    ) -> Result<Vec<ConversationHistoryHit>, AppError>;

    async fn create_notification(
        &self,
        auth: &AuthContext,
        params: NotificationCreateParams,
    ) -> Result<(), AppError>;

    async fn record_usage_event(
        &self,
        auth: &AuthContext,
        metric_type: &str,
        quantity: i64,
        source: &str,
    ) -> Result<(), AppError>;

    async fn get_document_asset_by_id(
        &self,
        auth: &AuthContext,
        asset_id: Uuid,
    ) -> Result<Option<DocumentAssetRow>, AppError>;

    async fn get_multimodal_chunk_by_id(
        &self,
        auth: &AuthContext,
        chunk_id: Uuid,
    ) -> Result<Option<MultimodalChunkRow>, AppError>;

    async fn append_audit_record(&self, record: &AuditRecord) -> Result<(), AppError>;

    async fn get_chunk_by_id(
        &self,
        auth: &AuthContext,
        chunk_id: Uuid,
    ) -> Result<Option<common::IndexedChunk>, AppError>;

    async fn get_summary_metadata(
        &self,
        auth: &AuthContext,
        doc_ids: &[Uuid],
    ) -> Result<Vec<common::SummaryMetadata>, AppError>;

    async fn upsert_user_profile(
        &self,
        auth: &AuthContext,
        profile: &UserProfileRow,
    ) -> Result<(), AppError>;
}
