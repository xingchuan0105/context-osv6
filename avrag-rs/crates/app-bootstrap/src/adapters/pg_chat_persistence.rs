use std::sync::Arc;

use async_trait::async_trait;
use app_core::{
    chat_persistence::{AppendChatTurn, ChatPersistencePort},
    domain_rows::{
        DocumentAssetRow, IndexedChunk, MultimodalChunkRow, NotificationCreateParams, TaggedMessage,
        UserProfileRow,
    },
};
use crate::domain_row_convert::{
    document_asset_row, indexed_chunk, multimodal_chunk_row, notification_create_params,
    tagged_message, user_profile_row,
};
use crate::pg_error::map_pg_error;
use avrag_auth::AuthContext;
use avrag_storage_pg::{ChatTurn, PgAppRepository};
use common::{AppError, ChatMessage, ChatSession, Notebook, SourceRow};
use ingestion_types::AuditRecord;
use uuid::Uuid;

pub struct PgChatPersistenceAdapter {
    repo: Arc<PgAppRepository>,
}

impl PgChatPersistenceAdapter {
    pub fn new(repo: Arc<PgAppRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl ChatPersistencePort for PgChatPersistenceAdapter {
    async fn search_notebooks(
        &self,
        auth: &AuthContext,
        pattern: &str,
    ) -> Result<Vec<Notebook>, AppError> {
        self.repo
            .search_notebooks(auth, pattern)
            .await
            .map_err(map_pg_error)
    }

    async fn search_sessions(
        &self,
        auth: &AuthContext,
        pattern: &str,
    ) -> Result<Vec<ChatSession>, AppError> {
        self.repo
            .search_sessions(auth, pattern)
            .await
            .map_err(map_pg_error)
    }

    async fn search_sources(
        &self,
        auth: &AuthContext,
        pattern: &str,
    ) -> Result<Vec<SourceRow>, AppError> {
        self.repo
            .search_sources(auth, pattern)
            .await
            .map_err(map_pg_error)
    }

    async fn list_sessions(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
    ) -> Result<Vec<ChatSession>, AppError> {
        self.repo
            .list_sessions(auth, notebook_id)
            .await
            .map_err(map_pg_error)
    }

    async fn get_session(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
    ) -> Result<Option<ChatSession>, AppError> {
        self.repo
            .get_session(auth, session_id)
            .await
            .map_err(map_pg_error)
    }

    async fn create_session(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
        title: Option<&str>,
        agent_type: &str,
    ) -> Result<ChatSession, AppError> {
        self.repo
            .create_session(auth, notebook_id, title, agent_type)
            .await
            .map_err(map_pg_error)
    }

    async fn update_session(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
        title: Option<&str>,
        pinned: Option<bool>,
    ) -> Result<Option<ChatSession>, AppError> {
        self.repo
            .update_session(auth, session_id, title, pinned)
            .await
            .map_err(map_pg_error)
    }

    async fn delete_session(&self, auth: &AuthContext, session_id: Uuid) -> Result<bool, AppError> {
        self.repo
            .delete_session(auth, session_id)
            .await
            .map_err(map_pg_error)
    }

    async fn list_messages(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
    ) -> Result<Vec<ChatMessage>, AppError> {
        self.repo
            .list_messages(auth, session_id)
            .await
            .map_err(map_pg_error)
    }

    async fn get_message(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
        message_id: i64,
    ) -> Result<Option<ChatMessage>, AppError> {
        self.repo
            .get_message(auth, session_id, message_id)
            .await
            .map_err(map_pg_error)
    }

    async fn append_chat_turn(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
        turn: AppendChatTurn<'_>,
    ) -> Result<i64, AppError> {
        self.repo
            .append_chat_turn(
                auth,
                session_id,
                ChatTurn {
                    user_content: turn.user_content,
                    assistant_content: turn.assistant_content,
                    assistant_answer_blocks: turn.assistant_answer_blocks,
                    agent_type: turn.agent_type,
                    citations: turn.citations,
                    tool_results: turn.tool_results,
                    user_turn_metadata: turn.user_turn_metadata,
                    user_resolved_query: turn.user_resolved_query,
                },
            )
            .await
            .map_err(map_pg_error)
    }

    async fn get_notebook(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
    ) -> Result<Option<Notebook>, AppError> {
        self.repo
            .get_notebook(auth, notebook_id)
            .await
            .map_err(map_pg_error)
    }

    async fn get_user_profile(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
    ) -> Result<Option<UserProfileRow>, AppError> {
        self.repo
            .get_user_profile(auth, user_id)
            .await
            .map_err(map_pg_error)
            .map(|profile| profile.map(user_profile_row))
    }

    async fn load_history_by_tags(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
        tags: Option<Vec<String>>,
        limit: i64,
    ) -> Result<Vec<TaggedMessage>, AppError> {
        self.repo
            .load_history_by_tags(auth, session_id, tags, limit)
            .await
            .map_err(map_pg_error)
            .map(|rows| rows.into_iter().map(tagged_message).collect())
    }

    async fn create_notification(
        &self,
        auth: &AuthContext,
        params: NotificationCreateParams,
    ) -> Result<(), AppError> {
        self.repo
            .create_notification(auth, notification_create_params(params))
            .await
            .map(|_| ())
            .map_err(map_pg_error)
    }

    async fn record_usage_event(
        &self,
        auth: &AuthContext,
        metric_type: &str,
        quantity: i64,
        source: &str,
    ) -> Result<(), AppError> {
        self.repo
            .record_usage_event(auth, metric_type, quantity, source)
            .await
            .map_err(map_pg_error)
    }

    async fn get_document_asset_by_id(
        &self,
        auth: &AuthContext,
        asset_id: Uuid,
    ) -> Result<Option<DocumentAssetRow>, AppError> {
        self.repo
            .get_document_asset_by_id(auth, asset_id)
            .await
            .map_err(map_pg_error)
            .map(|asset| asset.map(document_asset_row))
    }

    async fn get_multimodal_chunk_by_id(
        &self,
        auth: &AuthContext,
        chunk_id: Uuid,
    ) -> Result<Option<MultimodalChunkRow>, AppError> {
        self.repo
            .get_multimodal_chunk_by_id(auth, chunk_id)
            .await
            .map_err(map_pg_error)
            .map(|chunk| chunk.map(multimodal_chunk_row))
    }

    async fn append_audit_record(&self, record: &AuditRecord) -> Result<(), AppError> {
        self.repo
            .append_audit_record(record)
            .await
            .map_err(map_pg_error)
    }

    async fn get_chunk_by_id(
        &self,
        auth: &AuthContext,
        chunk_id: Uuid,
    ) -> Result<Option<IndexedChunk>, AppError> {
        self.repo
            .get_chunk_by_id(auth, chunk_id)
            .await
            .map_err(map_pg_error)
            .map(|chunk| chunk.map(indexed_chunk))
    }

    async fn get_summary_metadata(
        &self,
        auth: &AuthContext,
        doc_ids: &[Uuid],
    ) -> Result<Vec<common::SummaryMetadata>, AppError> {
        self.repo
            .get_summary_metadata(auth, doc_ids)
            .await
            .map_err(map_pg_error)
    }
}
