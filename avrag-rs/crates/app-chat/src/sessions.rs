use app_core::parse_uuid_or_app_error;
use common::{AppError, SourceRow, StatusOnlyResponse};
use contracts::chat::{ChatMessage, ChatRequest, ChatResponse};
use contracts::notebooks::{
    ChatSession, CreateChatSessionRequest, Notebook, UpdateChatSessionRequest,
};
use uuid::Uuid;

use crate::context::ChatContext;
use crate::ChatService;

impl ChatContext {
    fn require_chat_persistence(
        &self,
    ) -> Result<std::sync::Arc<dyn app_core::ChatPersistencePort>, AppError> {
        self.storage.chat_persistence().ok_or_else(|| {
            AppError::internal("chat persistence is not configured")
        })
    }

    pub async fn search(&self, pattern: &str) -> (Vec<Notebook>, Vec<ChatSession>, Vec<SourceRow>) {
        let Ok(pg) = self.require_chat_persistence() else {
            return (Vec::new(), Vec::new(), Vec::new());
        };
        let like_pattern = format!("%{}%", pattern);
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
    }

    pub async fn list_sessions(&self, notebook_id: Option<&str>) -> Vec<ChatSession> {
        let Ok(pg) = self.require_chat_persistence() else {
            return Vec::new();
        };
        let notebook_uuid = notebook_id.and_then(|value| Uuid::parse_str(value).ok());
        pg.list_sessions(&self.auth, notebook_uuid)
            .await
            .unwrap_or_default()
    }

    pub async fn create_session(
        &self,
        req: CreateChatSessionRequest,
    ) -> Result<ChatSession, AppError> {
        let pg = self.require_chat_persistence()?;
        let notebook_id = parse_uuid_or_app_error(
            &req.notebook_id,
            "notebook_not_found",
            "notebook not found",
        )?;
        let notebook = pg.get_notebook(&self.auth, notebook_id).await?;
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
            .await?;
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
        Ok(session)
    }

    pub async fn update_session(
        &self,
        session_id: &str,
        req: UpdateChatSessionRequest,
    ) -> Result<ChatSession, AppError> {
        let pg = self.require_chat_persistence()?;
        let renamed = req
            .title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some();
        let pinned = req.pinned;
        let session_id =
            parse_uuid_or_app_error(session_id, "session_not_found", "session not found")?;
        let title = req
            .title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let session = pg
            .update_session(&self.auth, session_id, title, pinned)
            .await?
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
        Ok(session)
    }

    pub async fn get_session(&self, session_id: &str) -> Option<ChatSession> {
        let pg = self.require_chat_persistence().ok()?;
        let session_id = Uuid::parse_str(session_id).ok()?;
        pg.get_session(&self.auth, session_id).await.ok().flatten()
    }

    pub async fn delete_session(&self, session_id: &str) -> Result<StatusOnlyResponse, AppError> {
        let pg = self.require_chat_persistence()?;
        let session_id =
            parse_uuid_or_app_error(session_id, "session_not_found", "session not found")?;
        let deleted = pg.delete_session(&self.auth, session_id).await?;
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
        Ok(StatusOnlyResponse {
            status: "deleted".to_string(),
        })
    }

    pub async fn list_messages(&self, session_id: &str) -> Result<Vec<ChatMessage>, AppError> {
        let pg = self.require_chat_persistence()?;
        let session_id =
            parse_uuid_or_app_error(session_id, "session_not_found", "session not found")?;
        pg.list_messages(&self.auth, session_id).await
    }

    pub async fn execute_chat(&self, req: ChatRequest) -> Result<ChatResponse, AppError> {
        if req.query.trim().is_empty() {
            return Err(AppError::validation("query_required", "query is required"));
        }

        ChatService::new(self.clone()).execute(req).await
    }
}
