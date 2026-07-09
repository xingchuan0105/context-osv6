use app_core::parse_uuid_or_app_error;
use common::{AppError, SourceRow, StatusOnlyResponse};
use contracts::chat::{ChatMessage, ChatRequest, ChatResponse};
use contracts::workspaces::{
    ChatSession, CreateChatSessionRequest, Workspace, UpdateChatSessionRequest,
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

    pub async fn search(&self, pattern: &str) -> (Vec<Workspace>, Vec<ChatSession>, Vec<SourceRow>) {
        let Ok(pg) = self.require_chat_persistence() else {
            return (Vec::new(), Vec::new(), Vec::new());
        };
        let like_pattern = format!("%{}%", pattern);
        let nb = pg
            .search_workspaces(&self.auth, &like_pattern)
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

    pub async fn list_sessions(&self, workspace_id: Option<&str>) -> Vec<ChatSession> {
        let Ok(pg) = self.require_chat_persistence() else {
            return Vec::new();
        };
        let notebook_uuid = workspace_id.and_then(|value| Uuid::parse_str(value).ok());
        pg.list_sessions(&self.auth, notebook_uuid)
            .await
            .unwrap_or_default()
    }

    pub async fn create_session(
        &self,
        req: CreateChatSessionRequest,
    ) -> Result<ChatSession, AppError> {
        let pg = self.require_chat_persistence()?;
        let workspace_id = parse_uuid_or_app_error(
            &req.workspace_id,
            "workspace_not_found",
            "workspace not found",
        )?;
        let notebook = pg.get_workspace(&self.auth, workspace_id).await?;
        if notebook.is_none() {
            return Err(AppError::not_found(
                "workspace_not_found",
                "workspace not found",
            ));
        }
        let session = pg
            .create_session(
                &self.auth,
                workspace_id,
                req.title.as_deref(),
                &req.agent_type,
            )
            .await?;
        self.record_product_event_if_available(
            analytics::ProductEventName::SessionCreated,
            analytics::Surface::Workspace,
            analytics::ResultTag::Success,
            Uuid::parse_str(&session.id).ok(),
            Some(workspace_id),
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
                Uuid::parse_str(&session.workspace_id).ok(),
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
                Uuid::parse_str(&session.workspace_id).ok(),
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
