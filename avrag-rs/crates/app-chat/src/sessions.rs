use app_core::parse_uuid_or_app_error;
use common::{AppError, SourceRow, StatusOnlyResponse};
use contracts::auth_runtime::SubjectKind;
use contracts::chat::{ChatMessage, ChatRequest, ChatResponse};
use contracts::workspaces::{
    ChatSession, CreateChatSessionRequest, UpdateChatSessionRequest, Workspace,
};
use uuid::Uuid;

use crate::ChatService;
use crate::context::ChatContext;

fn session_surface(workspace_id: Option<&str>) -> analytics::Surface {
    if workspace_id.is_some() {
        analytics::Surface::Workspace
    } else {
        analytics::Surface::Chat
    }
}

fn session_workspace_uuid(session: &ChatSession) -> Option<Uuid> {
    session
        .workspace_id
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok())
}

fn initial_model_role_for_creation(workspace_id: Option<Uuid>) -> &'static str {
    if workspace_id.is_some() {
        "agent"
    } else {
        "quick_chat"
    }
}

impl ChatContext {
    fn require_chat_persistence(
        &self,
    ) -> Result<std::sync::Arc<dyn app_core::ChatPersistencePort>, AppError> {
        self.storage
            .chat_persistence()
            .ok_or_else(|| AppError::internal("chat persistence is not configured"))
    }

    pub async fn search(
        &self,
        pattern: &str,
    ) -> (Vec<Workspace>, Vec<ChatSession>, Vec<SourceRow>) {
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
        let workspace_uuid = match workspace_id {
            Some(value) => match Uuid::parse_str(value) {
                Ok(workspace_id) => Some(workspace_id),
                Err(_) => return Vec::new(),
            },
            None => None,
        };
        pg.list_sessions(&self.auth, workspace_uuid)
            .await
            .unwrap_or_default()
    }

    pub async fn create_session(
        &self,
        req: CreateChatSessionRequest,
    ) -> Result<ChatSession, AppError> {
        let pg = self.require_chat_persistence()?;
        let agent_type = req.agent_type.as_deref().unwrap_or("chat");
        let workspace_id = match req.workspace_id.as_deref() {
            None => {
                if matches!(self.auth.subject_kind(), SubjectKind::ApiKey) {
                    return Err(AppError::validation(
                        "workspace_id_required",
                        "workspace_id is required for workspace API keys",
                    ));
                }
                None
            }
            Some(workspace_id) => {
                let workspace_id = parse_uuid_or_app_error(
                    workspace_id,
                    "invalid_workspace_id",
                    "workspace_id must be a valid UUID",
                )?;
                let notebook = pg.get_workspace(&self.auth, workspace_id).await?;
                if notebook.is_none() {
                    return Err(AppError::not_found(
                        "workspace_not_found",
                        "workspace not found",
                    ));
                }
                Some(workspace_id)
            }
        };
        // The creation surface selects the initial default once. The persisted
        // role is subsequently authoritative and is never recomputed from scope.
        let model_role = initial_model_role_for_creation(workspace_id);
        let session = pg
            .create_session(
                &self.auth,
                workspace_id,
                req.title.as_deref(),
                agent_type,
                model_role,
            )
            .await?;
        self.record_product_event_if_available(
            analytics::ProductEventName::SessionCreated,
            session_surface(session.workspace_id.as_deref()),
            analytics::ResultTag::Success,
            Uuid::parse_str(&session.id).ok(),
            session_workspace_uuid(&session),
            serde_json::json!({
                "agent_type": agent_type,
                "model_role": model_role,
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
                session_surface(session.workspace_id.as_deref()),
                analytics::ResultTag::Success,
                Some(session_id),
                session_workspace_uuid(&session),
                serde_json::json!({
                    "title": session.title.clone(),
                }),
            )
            .await;
        }
        if pinned == Some(true) {
            self.record_product_event_if_available(
                analytics::ProductEventName::SessionPinned,
                session_surface(session.workspace_id.as_deref()),
                analytics::ResultTag::Success,
                Some(session_id),
                session_workspace_uuid(&session),
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
        let session = pg
            .get_session(&self.auth, session_id)
            .await?
            .ok_or_else(|| AppError::not_found("session_not_found", "session not found"))?;
        let deleted = pg.delete_session(&self.auth, session_id).await?;
        if !deleted {
            return Err(AppError::not_found(
                "session_not_found",
                "session not found",
            ));
        }
        self.record_product_event_if_available(
            analytics::ProductEventName::SessionDeleted,
            session_surface(session.workspace_id.as_deref()),
            analytics::ResultTag::Success,
            Some(session_id),
            session_workspace_uuid(&session),
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

        // Owner-pays remount happens inside execute_chat_pipeline.
        ChatService::new(self.clone()).execute(req).await
    }

    /// Write-lane product entry (does not use agent-lane `dispatch_agent_mode`).
    pub async fn execute_write(&self, req: ChatRequest) -> Result<ChatResponse, AppError> {
        if req.query.trim().is_empty() {
            return Err(AppError::validation("query_required", "query is required"));
        }
        let workspace_id = req
            .workspace_id
            .as_deref()
            .map(|id| {
                parse_uuid_or_app_error(
                    id,
                    "invalid_workspace_id",
                    "workspace_id must be a valid UUID",
                )
            })
            .transpose()?
            .or_else(|| self.auth.workspace_id());
        let state = self.with_owner_pays_auth(workspace_id).await;
        crate::chat::execute_pipeline(
            state,
            req,
            crate::chat::PipelineLane::Write,
            Default::default(),
        )
        .await
    }
}
