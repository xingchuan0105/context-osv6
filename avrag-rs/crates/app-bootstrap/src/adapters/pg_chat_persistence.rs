use std::sync::Arc;

use crate::domain_row_convert::{
    conversation_history_hit, document_asset_row, indexed_chunk, multimodal_chunk_row,
    notification_create_params, user_profile_row, user_profile_row_to_pg,
};
use crate::pg_error::map_pg_error;
use app_core::{
    chat_persistence::{
        AppendChatTurn, ChatCatalogPort, ChatContentPort, ChatSideEffectPort, MessagePort,
        ProfilePort, SessionPort,
    },
    domain_rows::{
        ConversationHistoryHit, ConversationHistoryScope, DocumentAssetRow, IndexedChunk,
        MultimodalChunkRow, NotificationCreateParams, UserProfileRow,
    },
};
use async_trait::async_trait;
use contracts::auth_runtime::AuthContext;
use avrag_storage_pg::{ChatTurn, PgAppRepository};
use common::{AppError, SourceRow};
use contracts::chat::ChatMessage;
use contracts::notebooks::{ChatSession, Notebook};
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
impl SessionPort for PgChatPersistenceAdapter {
    async fn search_sessions(
        &self,
        auth: &AuthContext,
        pattern: &str,
    ) -> Result<Vec<ChatSession>, AppError> {
        self.repo
            .chunks()
            .search_sessions(auth, pattern)
            .await
            .map_err(map_pg_error)
    }

    async fn list_sessions(
        &self,
        auth: &AuthContext,
        notebook_id: Option<Uuid>,
    ) -> Result<Vec<ChatSession>, AppError> {
        self.repo
            .sessions()
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
            .sessions()
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
            .sessions()
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
            .sessions()
            .update_session(auth, session_id, title, pinned)
            .await
            .map_err(map_pg_error)
    }

    async fn delete_session(&self, auth: &AuthContext, session_id: Uuid) -> Result<bool, AppError> {
        self.repo
            .sessions()
            .delete_session(auth, session_id)
            .await
            .map_err(map_pg_error)
    }

}

#[async_trait]
impl MessagePort for PgChatPersistenceAdapter {
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
            .sessions()
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
            .sessions()
            .append_chat_turn(
                auth,
                session_id,
                &ChatTurn {
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

    async fn search_conversation_history(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
        query: &str,
        scope: ConversationHistoryScope,
        limit: i64,
        exclude_message_ids: &[i64],
    ) -> Result<Vec<ConversationHistoryHit>, AppError> {
        let pg_scope = match scope {
            ConversationHistoryScope::Session => {
                avrag_storage_pg::ConversationHistoryScope::Session
            }
            ConversationHistoryScope::Notebook => {
                avrag_storage_pg::ConversationHistoryScope::Notebook
            }
        };
        self.repo
            .conversation_memory()
            .search_conversation_history(
                auth,
                session_id,
                query,
                pg_scope,
                limit,
                exclude_message_ids,
            )
            .await
            .map_err(map_pg_error)
            .map(|rows| rows.into_iter().map(conversation_history_hit).collect())
    }

}

#[async_trait]
impl ChatCatalogPort for PgChatPersistenceAdapter {
    async fn search_notebooks(
        &self,
        auth: &AuthContext,
        pattern: &str,
    ) -> Result<Vec<Notebook>, AppError> {
        self.repo
            .chunks()
            .search_notebooks(auth, pattern)
            .await
            .map_err(map_pg_error)
    }

    async fn search_sources(
        &self,
        auth: &AuthContext,
        pattern: &str,
    ) -> Result<Vec<SourceRow>, AppError> {
        self.repo
            .chunks()
            .search_sources(auth, pattern)
            .await
            .map_err(map_pg_error)
    }

    async fn get_notebook(
        &self,
        auth: &AuthContext,
        notebook_id: Uuid,
    ) -> Result<Option<Notebook>, AppError> {
        self.repo
            .bootstrap()
            .get_notebook(auth, notebook_id)
            .await
            .map_err(map_pg_error)
    }

}

#[async_trait]
impl ProfilePort for PgChatPersistenceAdapter {
    async fn get_user_profile(
        &self,
        auth: &AuthContext,
        user_id: Uuid,
    ) -> Result<Option<UserProfileRow>, AppError> {
        self.repo
            .auth()
            .get_user_profile(auth, user_id)
            .await
            .map_err(map_pg_error)
            .map(|profile| profile.map(user_profile_row))
    }

    async fn upsert_user_profile(
        &self,
        auth: &AuthContext,
        profile: &UserProfileRow,
    ) -> Result<(), AppError> {
        self.repo
            .auth()
            .upsert_user_profile(auth, &user_profile_row_to_pg(profile))
            .await
            .map_err(map_pg_error)
    }

}

#[async_trait]
impl ChatContentPort for PgChatPersistenceAdapter {
    async fn get_document_asset_by_id(
        &self,
        auth: &AuthContext,
        asset_id: Uuid,
    ) -> Result<Option<DocumentAssetRow>, AppError> {
        self.repo
            .assets()
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
            .assets()
            .get_multimodal_chunk_by_id(auth, chunk_id)
            .await
            .map_err(map_pg_error)
            .map(|chunk| chunk.map(multimodal_chunk_row))
    }

    async fn get_chunk_by_id(
        &self,
        auth: &AuthContext,
        chunk_id: Uuid,
    ) -> Result<Option<IndexedChunk>, AppError> {
        self.repo
            .chunks()
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
            .chunks()
            .get_summary_metadata(auth, doc_ids)
            .await
            .map_err(map_pg_error)
    }

}

#[async_trait]
impl ChatSideEffectPort for PgChatPersistenceAdapter {
    async fn create_notification(
        &self,
        auth: &AuthContext,
        params: NotificationCreateParams,
    ) -> Result<(), AppError> {
        self.repo
            .auth()
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
            .sessions()
            .record_usage_event(auth, metric_type, quantity, source)
            .await
            .map_err(map_pg_error)
    }

    async fn append_audit_record(&self, record: &AuditRecord) -> Result<(), AppError> {
        self.repo
            .audit()
            .append_audit_record(record)
            .await
            .map_err(map_pg_error)
    }

}

