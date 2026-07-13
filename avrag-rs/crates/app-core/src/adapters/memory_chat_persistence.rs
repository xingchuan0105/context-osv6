//! In-memory [`ChatPersistencePort`] backed by [`MemoryState`].
//!
//! Memory mode is an adapter choice at bootstrap — domain code always talks to
//! the port and never dual-branches on `storage.inner()`.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use common::{AppError, SourceRow, new_id, now_rfc3339};
use contracts::auth_runtime::AuthContext;
use contracts::chat::ChatMessage;
use contracts::documents::DocumentStatus;
use contracts::workspaces::{ChatSession, Workspace};
use ingestion_types::AuditRecord;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::chat_persistence::{
    AppendChatTurn, ChatCatalogPort, ChatContentPort, ChatSideEffectPort, MessagePort, ProfilePort,
    SessionPort,
};
use crate::domain_rows::{
    ConversationHistoryHit, ConversationHistoryScope, DocumentAssetRow, IndexedChunk,
    MultimodalChunkRow, NotificationCreateParams, UserProfileRow,
};
use crate::{MemoryState, current_owner_user_id};

/// Memory-backed chat persistence (sessions, messages, catalog, light side-effects).
#[derive(Clone)]
pub struct MemoryChatPersistence {
    state: Arc<RwLock<MemoryState>>,
    profiles: Arc<RwLock<BTreeMap<Uuid, UserProfileRow>>>,
}

impl MemoryChatPersistence {
    pub fn new(state: Arc<RwLock<MemoryState>>) -> Self {
        Self {
            state,
            profiles: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    fn owner_user_id(auth: &AuthContext) -> String {
        current_owner_user_id(auth)
    }

    fn session_visible(state: &MemoryState, auth: &AuthContext, session: &ChatSession) -> bool {
        state
            .workspaces
            .get(&session.workspace_id)
            .map(|nb| nb.owner_user_id == Self::owner_user_id(auth))
            .unwrap_or(false)
    }

    fn strip_like_pattern(pattern: &str) -> String {
        pattern.trim().trim_matches('%').to_lowercase()
    }

    fn status_label(status: &DocumentStatus) -> &'static str {
        match status {
            DocumentStatus::Pending => "pending",
            DocumentStatus::Enqueueing => "enqueueing",
            DocumentStatus::Queued => "queued",
            DocumentStatus::Processing => "processing",
            DocumentStatus::Completed => "completed",
            DocumentStatus::Failed => "failed",
            DocumentStatus::Deleting => "deleting",
            DocumentStatus::Deleted => "deleted",
            DocumentStatus::UploadInvalid => "upload_invalid",
        }
    }

    fn is_deleting_or_deleted(status: &DocumentStatus) -> bool {
        matches!(status, DocumentStatus::Deleting | DocumentStatus::Deleted)
    }
}

#[async_trait]
impl SessionPort for MemoryChatPersistence {
    async fn search_sessions(
        &self,
        auth: &AuthContext,
        pattern: &str,
    ) -> Result<Vec<ChatSession>, AppError> {
        let q = Self::strip_like_pattern(pattern);
        let state = self.state.read().await;
        let messages = &state.messages;
        Ok(state
            .sessions
            .values()
            .filter(|session| Self::session_visible(&state, auth, session))
            .filter(|session| {
                if session
                    .title
                    .as_ref()
                    .is_some_and(|t| t.to_lowercase().contains(&q))
                {
                    return true;
                }
                messages.get(&session.id).is_some_and(|session_messages| {
                    session_messages
                        .iter()
                        .any(|m| m.content.to_lowercase().contains(&q))
                })
            })
            .cloned()
            .collect())
    }

    async fn list_sessions(
        &self,
        auth: &AuthContext,
        workspace_id: Option<Uuid>,
    ) -> Result<Vec<ChatSession>, AppError> {
        let notebook_key = workspace_id.map(|id| id.to_string());
        let state = self.state.read().await;
        Ok(state
            .sessions
            .values()
            .filter(|session| Self::session_visible(&state, auth, session))
            .filter(|session| {
                notebook_key
                    .as_ref()
                    .map(|id| session.workspace_id == *id)
                    .unwrap_or(true)
            })
            .cloned()
            .collect())
    }

    async fn get_session(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
    ) -> Result<Option<ChatSession>, AppError> {
        let key = session_id.to_string();
        let state = self.state.read().await;
        Ok(state
            .sessions
            .get(&key)
            .filter(|session| Self::session_visible(&state, auth, session))
            .cloned())
    }

    async fn create_session(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
        title: Option<&str>,
        agent_type: &str,
    ) -> Result<ChatSession, AppError> {
        let notebook_key = workspace_id.to_string();
        let mut state = self.state.write().await;
        let notebook = state
            .workspaces
            .get(&notebook_key)
            .filter(|nb| nb.owner_user_id == Self::owner_user_id(auth))
            .cloned()
            .ok_or_else(|| AppError::not_found("workspace_not_found", "workspace not found"))?;
        let now = now_rfc3339();
        let session = ChatSession {
            id: new_id(),
            workspace_id: notebook.id,
            title: title
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(ToOwned::to_owned),
            agent_type: agent_type.to_string(),
            pinned: false,
            created_at: now.clone(),
            updated_at: now,
        };
        state.sessions.insert(session.id.clone(), session.clone());
        Ok(session)
    }

    async fn update_session(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
        title: Option<&str>,
        pinned: Option<bool>,
    ) -> Result<Option<ChatSession>, AppError> {
        let key = session_id.to_string();
        let mut state = self.state.write().await;
        let visible = state
            .sessions
            .get(&key)
            .map(|session| Self::session_visible(&state, auth, session))
            .unwrap_or(false);
        if !visible {
            return Ok(None);
        }
        let Some(session) = state.sessions.get_mut(&key) else {
            return Ok(None);
        };
        if let Some(title) = title {
            let trimmed = title.trim();
            session.title = (!trimmed.is_empty()).then(|| trimmed.to_string());
        }
        if let Some(pinned) = pinned {
            session.pinned = pinned;
        }
        session.updated_at = now_rfc3339();
        Ok(Some(session.clone()))
    }

    async fn delete_session(&self, auth: &AuthContext, session_id: Uuid) -> Result<bool, AppError> {
        let key = session_id.to_string();
        let mut state = self.state.write().await;
        let can_delete = state
            .sessions
            .get(&key)
            .map(|session| Self::session_visible(&state, auth, session))
            .unwrap_or(false);
        if !can_delete {
            return Ok(false);
        }
        state.sessions.remove(&key);
        state.messages.remove(&key);
        Ok(true)
    }
}

#[async_trait]
impl MessagePort for MemoryChatPersistence {
    async fn list_messages(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
    ) -> Result<Vec<ChatMessage>, AppError> {
        let key = session_id.to_string();
        let state = self.state.read().await;
        let session = state
            .sessions
            .get(&key)
            .ok_or_else(|| AppError::not_found("session_not_found", "session not found"))?;
        if !Self::session_visible(&state, auth, session) {
            return Err(AppError::not_found(
                "session_not_found",
                "session not found",
            ));
        }
        Ok(state.messages.get(&key).cloned().unwrap_or_default())
    }

    async fn get_message(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
        message_id: i64,
    ) -> Result<Option<ChatMessage>, AppError> {
        let messages = self.list_messages(auth, session_id).await?;
        Ok(messages.into_iter().find(|m| m.id == message_id))
    }

    async fn append_chat_turn(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
        turn: AppendChatTurn<'_>,
    ) -> Result<i64, AppError> {
        let key = session_id.to_string();
        let mut state = self.state.write().await;
        let session = state
            .sessions
            .get(&key)
            .cloned()
            .ok_or_else(|| AppError::not_found("session_not_found", "session not found"))?;
        if !Self::session_visible(&state, auth, &session) {
            return Err(AppError::not_found(
                "session_not_found",
                "session not found",
            ));
        }
        let now = now_rfc3339();
        let user_id = {
            state.next_message_id += 1;
            state.next_message_id
        };
        let assistant_id = {
            state.next_message_id += 1;
            state.next_message_id
        };
        let user_msg = ChatMessage {
            id: user_id,
            session_id: key.clone(),
            role: "user".to_string(),
            content: turn.user_content.to_string(),
            answer_blocks: vec![],
            agent_id: None,
            agent_name: None,
            agent_icon: None,
            citations: vec![],
            tool_results: vec![],
            turn_metadata: turn.user_turn_metadata.clone(),
            resolved_query: turn.user_resolved_query.map(ToOwned::to_owned),
            created_at: now.clone(),
        };
        let assistant_msg = ChatMessage {
            id: assistant_id,
            session_id: key.clone(),
            role: "assistant".to_string(),
            content: turn.assistant_content.to_string(),
            answer_blocks: turn.assistant_answer_blocks.to_vec(),
            agent_id: Some(turn.agent_type.to_string()),
            agent_name: None,
            agent_icon: None,
            citations: turn.citations.to_vec(),
            tool_results: turn.tool_results.to_vec(),
            turn_metadata: turn.assistant_turn_metadata.clone(),
            resolved_query: None,
            created_at: now.clone(),
        };
        let entry = state.messages.entry(key.clone()).or_default();
        entry.push(user_msg);
        entry.push(assistant_msg);
        if let Some(session) = state.sessions.get_mut(&key) {
            session.updated_at = now;
        }
        Ok(assistant_id)
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
        let q = query.trim().to_lowercase();
        if q.is_empty() || limit <= 0 {
            return Ok(Vec::new());
        }
        let state = self.state.read().await;
        let session_key = session_id.to_string();
        let session = state
            .sessions
            .get(&session_key)
            .ok_or_else(|| AppError::not_found("session_not_found", "session not found"))?;
        if !Self::session_visible(&state, auth, session) {
            return Err(AppError::not_found(
                "session_not_found",
                "session not found",
            ));
        }
        let workspace_id = session.workspace_id.clone();
        let session_keys: Vec<String> = match scope {
            ConversationHistoryScope::Session => vec![session_key],
            ConversationHistoryScope::Workspace => state
                .sessions
                .values()
                .filter(|s| s.workspace_id == workspace_id)
                .filter(|s| Self::session_visible(&state, auth, s))
                .map(|s| s.id.clone())
                .collect(),
        };
        let exclude: std::collections::HashSet<i64> =
            exclude_message_ids.iter().copied().collect();
        let mut hits = Vec::new();
        for sk in session_keys {
            let Some(messages) = state.messages.get(&sk) else {
                continue;
            };
            let sid = Uuid::parse_str(&sk).unwrap_or(session_id);
            for msg in messages {
                if exclude.contains(&msg.id) {
                    continue;
                }
                if !msg.content.to_lowercase().contains(&q) {
                    continue;
                }
                hits.push(ConversationHistoryHit {
                    message_id: msg.id,
                    session_id: sid,
                    role: msg.role.clone(),
                    content: msg.content.clone(),
                    created_at: Utc::now(),
                });
                if hits.len() as i64 >= limit {
                    return Ok(hits);
                }
            }
        }
        Ok(hits)
    }
}

#[async_trait]
impl ChatCatalogPort for MemoryChatPersistence {
    async fn search_workspaces(
        &self,
        auth: &AuthContext,
        pattern: &str,
    ) -> Result<Vec<Workspace>, AppError> {
        let q = Self::strip_like_pattern(pattern);
        let org = Self::owner_user_id(auth);
        let state = self.state.read().await;
        Ok(state
            .workspaces
            .values()
            .filter(|nb| nb.owner_user_id == org)
            .filter(|nb| {
                nb.title.to_lowercase().contains(&q) || nb.description.to_lowercase().contains(&q)
            })
            .cloned()
            .collect())
    }

    async fn search_sources(
        &self,
        auth: &AuthContext,
        pattern: &str,
    ) -> Result<Vec<SourceRow>, AppError> {
        let q = Self::strip_like_pattern(pattern);
        let org = Self::owner_user_id(auth);
        let state = self.state.read().await;
        Ok(state
            .documents
            .values()
            .filter(|stored| {
                stored.document.owner_user_id == org
                    && stored.document.file_name.to_lowercase().contains(&q)
                    && !Self::is_deleting_or_deleted(&stored.document.status)
            })
            .filter_map(|stored| {
                let notebook = state.workspaces.get(&stored.document.workspace_id)?;
                if notebook.owner_user_id != org {
                    return None;
                }
                Some(SourceRow {
                    id: stored.document.id.clone(),
                    workspace_id: notebook.id.clone(),
                    workspace_name: notebook.name.clone(),
                    title: stored.document.file_name.clone(),
                    file_name: stored.document.file_name.clone(),
                    status: Self::status_label(&stored.document.status).to_string(),
                    last_error: None,
                })
            })
            .collect())
    }

    async fn get_workspace(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<Option<Workspace>, AppError> {
        let key = workspace_id.to_string();
        let org = Self::owner_user_id(auth);
        let state = self.state.read().await;
        Ok(state
            .workspaces
            .get(&key)
            .filter(|nb| nb.owner_user_id == org)
            .cloned())
    }
}

#[async_trait]
impl ProfilePort for MemoryChatPersistence {
    async fn get_user_profile(
        &self,
        _auth: &AuthContext,
        user_id: Uuid,
    ) -> Result<Option<UserProfileRow>, AppError> {
        let profiles = self.profiles.read().await;
        Ok(profiles.get(&user_id).cloned())
    }

    async fn upsert_user_profile(
        &self,
        _auth: &AuthContext,
        profile: &UserProfileRow,
    ) -> Result<(), AppError> {
        let mut profiles = self.profiles.write().await;
        profiles.insert(profile.user_id, profile.clone());
        Ok(())
    }
}

#[async_trait]
impl ChatContentPort for MemoryChatPersistence {
    async fn get_document_asset_by_id(
        &self,
        _auth: &AuthContext,
        _asset_id: Uuid,
    ) -> Result<Option<DocumentAssetRow>, AppError> {
        Ok(None)
    }

    async fn get_multimodal_chunk_by_id(
        &self,
        _auth: &AuthContext,
        _chunk_id: Uuid,
    ) -> Result<Option<MultimodalChunkRow>, AppError> {
        Ok(None)
    }

    async fn get_chunk_by_id(
        &self,
        _auth: &AuthContext,
        _chunk_id: Uuid,
    ) -> Result<Option<IndexedChunk>, AppError> {
        Ok(None)
    }

    async fn get_summary_metadata(
        &self,
        _auth: &AuthContext,
        _doc_ids: &[Uuid],
    ) -> Result<Vec<common::SummaryMetadata>, AppError> {
        Ok(Vec::new())
    }
}

#[async_trait]
impl ChatSideEffectPort for MemoryChatPersistence {
    async fn create_notification(
        &self,
        auth: &AuthContext,
        params: NotificationCreateParams,
    ) -> Result<(), AppError> {
        let now = now_rfc3339();
        let data = match params.data {
            serde_json::Value::Object(map) => map.into_iter().collect(),
            _ => BTreeMap::new(),
        };
        let row = common::NotificationRow {
            id: new_id(),
            owner_user_id: Self::owner_user_id(auth),
            user_id: params.user_id.to_string(),
            event_type: params.event_type,
            title: params.title,
            body: params.body,
            data,
            read_at: None,
            created_at: now.clone(),
            updated_at: now,
        };
        let mut state = self.state.write().await;
        state.notifications.push(row);
        Ok(())
    }

    async fn record_usage_event(
        &self,
        _auth: &AuthContext,
        _metric_type: &str,
        _quantity: i64,
        _source: &str,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn append_audit_record(&self, _record: &AuditRecord) -> Result<(), AppError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::auth_runtime::{ActorId, AuthContext, UserId, SubjectKind};
    use crate::chat_persistence::{MessagePort, SessionPort};

    fn auth(org: Uuid, user: Uuid) -> AuthContext {
        AuthContext::new(UserId::from(org), SubjectKind::User).with_actor_id(ActorId::new(user))
    }

    #[tokio::test]
    async fn session_round_trip_and_message_append() {
        let state = Arc::new(RwLock::new(MemoryState::default()));
        let org = Uuid::from_u128(1);
        let user = Uuid::from_u128(2);
        let workspace_id = Uuid::from_u128(10);
        let now = now_rfc3339();
        {
            let mut s = state.write().await;
            s.workspaces.insert(
                workspace_id.to_string(),
                Workspace {
                    id: workspace_id.to_string(),
                    owner_user_id: org.to_string(),
                    owner_id: user.to_string(),
                    name: "nb".into(),
                    title: "nb".into(),
                    description: String::new(),
                    document_count: 0,
                    status_summary: Default::default(),
                    shared: false,
                    created_at: now.clone(),
                    updated_at: now,
                },
            );
        }
        let store = MemoryChatPersistence::new(state);
        let a = auth(org, user);
        let session = store
            .create_session(&a, workspace_id, Some("hello"), "chat")
            .await
            .expect("create");
        let session_uuid = Uuid::parse_str(&session.id).unwrap();
        let listed = store.list_sessions(&a, Some(workspace_id)).await.unwrap();
        assert_eq!(listed.len(), 1);
        let assistant_id = store
            .append_chat_turn(
                &a,
                session_uuid,
                AppendChatTurn {
                    user_content: "hi",
                    assistant_content: "hello there",
                    assistant_answer_blocks: &[],
                    agent_type: "chat",
                    citations: &[],
                    tool_results: &[],
                    user_turn_metadata: None,
                    user_resolved_query: None,
                    assistant_turn_metadata: None,
                },
            )
            .await
            .expect("append");
        assert!(assistant_id > 0);
        let messages = store.list_messages(&a, session_uuid).await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].content, "hello there");
    }

    #[tokio::test]
    async fn other_org_cannot_see_session() {
        let state = Arc::new(RwLock::new(MemoryState::default()));
        let org = Uuid::from_u128(1);
        let other = Uuid::from_u128(99);
        let user = Uuid::from_u128(2);
        let workspace_id = Uuid::from_u128(10);
        let now = now_rfc3339();
        {
            let mut s = state.write().await;
            s.workspaces.insert(
                workspace_id.to_string(),
                Workspace {
                    id: workspace_id.to_string(),
                    owner_user_id: org.to_string(),
                    owner_id: user.to_string(),
                    name: "nb".into(),
                    title: "nb".into(),
                    description: String::new(),
                    document_count: 0,
                    status_summary: Default::default(),
                    shared: false,
                    created_at: now.clone(),
                    updated_at: now,
                },
            );
        }
        let store = MemoryChatPersistence::new(state);
        let a = auth(org, user);
        let session = store
            .create_session(&a, workspace_id, None, "chat")
            .await
            .unwrap();
        let sid = Uuid::parse_str(&session.id).unwrap();
        let other_auth = auth(other, user);
        assert!(store.get_session(&other_auth, sid).await.unwrap().is_none());
        assert!(store.list_sessions(&other_auth, None).await.unwrap().is_empty());
    }
}
