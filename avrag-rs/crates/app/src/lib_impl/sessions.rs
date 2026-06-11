use crate::lib_impl::*;
use common::{
    AppError, ChatMessage, ChatRequest, ChatResponse, ChatSession, CreateChatSessionRequest,
    Notebook, SourceRow, StatusOnlyResponse, UpdateChatSessionRequest, new_id, now_rfc3339,
};
use uuid::Uuid;

impl AppState {
    pub async fn search(&self, pattern: &str) -> (Vec<Notebook>, Vec<ChatSession>, Vec<SourceRow>) {
        let like_pattern = format!("%{}%", pattern);
        if let Some(pg) = self.storage.pg() {
            let nb = pg
                .search_notebooks(&self.auth, &like_pattern)
                .await
                .unwrap_or_default();
            let sess = pg
                .search_sessions(&self.auth, &like_pattern)
                .await
                .unwrap_or_default();
            let src = pg
                .search_sources(&self.auth, &like_pattern)
                .await
                .unwrap_or_default();
            (nb, sess, src)
        } else {
            let state = self.storage.inner().read().await;
            let q = pattern.to_lowercase();
            let notebooks: Vec<Notebook> = state
                .notebooks
                .values()
                .filter(|nb| {
                    nb.org_id == self.current_org_id()
                        && (nb.title.to_lowercase().contains(&q)
                            || nb.description.to_lowercase().contains(&q))
                })
                .cloned()
                .collect();
            let sessions = state
                .sessions
                .values()
                .filter(|session| {
                    self.memory_session_visible(&state, session)
                        && session
                            .title
                            .as_ref()
                            .map(|t| t.to_lowercase().contains(&q))
                            .unwrap_or(false)
                })
                .cloned()
                .collect();
            let sources = state
                .documents
                .values()
                .filter(|stored| {
                    stored.document.org_id == self.current_org_id()
                        && stored.document.file_name.to_lowercase().contains(&q)
                        && !document_is_deleting_or_deleted(&stored.document.status)
                })
                .filter_map(|stored| {
                    let notebook = state.notebooks.get(&stored.document.notebook_id)?;
                    if notebook.org_id != self.current_org_id() {
                        return None;
                    }
                    Some(SourceRow {
                        id: stored.document.id.clone(),
                        notebook_id: notebook.id.clone(),
                        notebook_name: notebook.name.clone(),
                        title: stored.document.file_name.clone(),
                        file_name: stored.document.file_name.clone(),
                        status: status_label(&stored.document.status).to_string(),
                    })
                })
                .collect();
            (notebooks, sessions, sources)
        }
    }

    pub async fn list_sessions(&self, notebook_id: Option<&str>) -> Vec<ChatSession> {
        if let Some(pg) = self.storage.pg() {
            let notebook_uuid = notebook_id.and_then(|value| Uuid::parse_str(value).ok());
            return pg
                .list_sessions(&self.auth, notebook_uuid)
                .await
                .unwrap_or_default();
        }
        let state = self.storage.inner().read().await;
        state
            .sessions
            .values()
            .filter(|session| {
                self.memory_session_visible(&state, session)
                    && notebook_id
                        .map(|id| session.notebook_id == id)
                        .unwrap_or(true)
            })
            .cloned()
            .collect()
    }

    pub async fn create_session(
        &self,
        req: CreateChatSessionRequest,
    ) -> Result<ChatSession, AppError> {
        if let Some(pg) = self.storage.pg() {
            let notebook_id = parse_uuid_or_app_error(
                &req.notebook_id,
                "notebook_not_found",
                "notebook not found",
            )?;
            let notebook = pg
                .get_notebook(&self.auth, notebook_id)
                .await
                .map_err(map_pg_error)?;
            if notebook.is_none() {
                return Err(AppError::not_found(
                    "notebook_not_found",
                    "notebook not found",
                ));
            }
            let session = pg
                .create_session(
                    &self.auth,
                    notebook_id,
                    req.title.as_deref(),
                    &req.agent_type,
                )
                .await
                .map_err(map_pg_error)?;
            self.record_product_event_if_available(
                analytics::ProductEventName::SessionCreated,
                analytics::Surface::Workspace,
                analytics::ResultTag::Success,
                Uuid::parse_str(&session.id).ok(),
                Some(notebook_id),
                serde_json::json!({
                    "agent_type": req.agent_type,
                }),
            )
            .await;
            return Ok(session);
        }

        let notebook = self
            .get_notebook(&req.notebook_id)
            .await
            .ok_or_else(|| AppError::not_found("notebook_not_found", "notebook not found"))?;
        let now = now_rfc3339();
        let session = ChatSession {
            id: new_id(),
            notebook_id: notebook.id,
            title: req.title,
            agent_type: req.agent_type,
            summary: None,
            pinned: false,
            created_at: now.clone(),
            updated_at: now,
        };
        {
            let mut state = self.storage.inner().write().await;
            state.sessions.insert(session.id.clone(), session.clone());
        }
        self.record_product_event_if_available(
            analytics::ProductEventName::SessionCreated,
            analytics::Surface::Workspace,
            analytics::ResultTag::Success,
            Uuid::parse_str(&session.id).ok(),
            Uuid::parse_str(&session.notebook_id).ok(),
            serde_json::json!({
                "agent_type": session.agent_type.clone(),
            }),
        )
        .await;
        Ok(session)
    }

    pub async fn update_session(
        &self,
        session_id: &str,
        req: UpdateChatSessionRequest,
    ) -> Result<ChatSession, AppError> {
        let renamed = req
            .title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some();
        let pinned = req.pinned;
        if let Some(pg) = self.storage.pg() {
            let session_id =
                parse_uuid_or_app_error(session_id, "session_not_found", "session not found")?;
            let title = req
                .title
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let session = pg
                .update_session(&self.auth, session_id, title, pinned)
                .await
                .map_err(map_pg_error)?;
            let session = session
                .ok_or_else(|| AppError::not_found("session_not_found", "session not found"))?;
            if renamed {
                self.record_product_event_if_available(
                    analytics::ProductEventName::SessionRenamed,
                    analytics::Surface::Workspace,
                    analytics::ResultTag::Success,
                    Some(session_id),
                    Uuid::parse_str(&session.notebook_id).ok(),
                    serde_json::json!({
                        "title": session.title.clone(),
                    }),
                )
                .await;
            }
            if pinned == Some(true) {
                self.record_product_event_if_available(
                    analytics::ProductEventName::SessionPinned,
                    analytics::Surface::Workspace,
                    analytics::ResultTag::Success,
                    Some(session_id),
                    Uuid::parse_str(&session.notebook_id).ok(),
                    serde_json::json!({}),
                )
                .await;
            }
            return Ok(session);
        }

        let mut state = self.storage.inner().write().await;
        let visible = state
            .sessions
            .get(session_id)
            .map(|session| self.memory_session_visible(&state, session))
            .unwrap_or(false);
        if !visible {
            return Err(AppError::not_found(
                "session_not_found",
                "session not found",
            ));
        }
        let Some(session) = state.sessions.get_mut(session_id) else {
            return Err(AppError::not_found(
                "session_not_found",
                "session not found",
            ));
        };
        if let Some(title) = req.title {
            let trimmed = title.trim().to_string();
            session.title = (!trimmed.is_empty()).then_some(trimmed);
        }
        if let Some(pinned) = pinned {
            session.pinned = pinned;
        }
        session.updated_at = now_rfc3339();
        let updated = session.clone();
        drop(state);
        if renamed {
            self.record_product_event_if_available(
                analytics::ProductEventName::SessionRenamed,
                analytics::Surface::Workspace,
                analytics::ResultTag::Success,
                Uuid::parse_str(&updated.id).ok(),
                Uuid::parse_str(&updated.notebook_id).ok(),
                serde_json::json!({
                    "title": updated.title,
                }),
            )
            .await;
        }
        if pinned == Some(true) {
            self.record_product_event_if_available(
                analytics::ProductEventName::SessionPinned,
                analytics::Surface::Workspace,
                analytics::ResultTag::Success,
                Uuid::parse_str(&updated.id).ok(),
                Uuid::parse_str(&updated.notebook_id).ok(),
                serde_json::json!({}),
            )
            .await;
        }
        Ok(updated)
    }

    pub async fn get_session(&self, session_id: &str) -> Option<ChatSession> {
        if let Some(pg) = self.storage.pg() {
            let session_id = Uuid::parse_str(session_id).ok()?;
            return pg.get_session(&self.auth, session_id).await.ok().flatten();
        }
        let state = self.storage.inner().read().await;
        state
            .sessions
            .get(session_id)
            .filter(|session| self.memory_session_visible(&state, session))
            .cloned()
    }

    pub async fn delete_session(&self, session_id: &str) -> Result<StatusOnlyResponse, AppError> {
        if let Some(pg) = self.storage.pg() {
            let session_id =
                parse_uuid_or_app_error(session_id, "session_not_found", "session not found")?;
            let deleted = pg
                .delete_session(&self.auth, session_id)
                .await
                .map_err(map_pg_error)?;
            if !deleted {
                return Err(AppError::not_found(
                    "session_not_found",
                    "session not found",
                ));
            }
            self.record_product_event_if_available(
                analytics::ProductEventName::SessionDeleted,
                analytics::Surface::Workspace,
                analytics::ResultTag::Success,
                Some(session_id),
                None,
                serde_json::json!({}),
            )
            .await;
            return Ok(StatusOnlyResponse {
                status: "deleted".to_string(),
            });
        }

        let mut state = self.storage.inner().write().await;
        let can_delete = state
            .sessions
            .get(session_id)
            .map(|session| self.memory_session_visible(&state, session))
            .unwrap_or(false);
        if !can_delete {
            return Err(AppError::not_found(
                "session_not_found",
                "session not found",
            ));
        }
        state.sessions.remove(session_id);
        state.messages.remove(session_id);
        drop(state);
        self.record_product_event_if_available(
            analytics::ProductEventName::SessionDeleted,
            analytics::Surface::Workspace,
            analytics::ResultTag::Success,
            Uuid::parse_str(session_id).ok(),
            None,
            serde_json::json!({}),
        )
        .await;
        Ok(StatusOnlyResponse {
            status: "deleted".to_string(),
        })
    }

    pub async fn list_messages(&self, session_id: &str) -> Result<Vec<ChatMessage>, AppError> {
        if let Some(pg) = self.storage.pg() {
            let session_id =
                parse_uuid_or_app_error(session_id, "session_not_found", "session not found")?;
            return pg
                .list_messages(&self.auth, session_id)
                .await
                .map_err(map_pg_error);
        }

        let state = self.storage.inner().read().await;
        let Some(session) = state.sessions.get(session_id) else {
            return Err(AppError::not_found(
                "session_not_found",
                "session not found",
            ));
        };
        if !self.memory_session_visible(&state, session) {
            return Err(AppError::not_found(
                "session_not_found",
                "session not found",
            ));
        }
        Ok(state.messages.get(session_id).cloned().unwrap_or_default())
    }

    pub async fn execute_chat(&self, req: ChatRequest) -> Result<ChatResponse, AppError> {
        if req.query.trim().is_empty() {
            return Err(AppError::validation("query_required", "query is required"));
        }

        crate::services::chat_service::ChatService::new(self.clone())
            .execute(req)
            .await
    }
}
