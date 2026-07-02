use std::collections::BTreeMap;
use std::sync::Arc;

use crate::agents::service::UnifiedAgentService;
use avrag_rag_core::context::SessionContext as RagSessionContext;
use common::AppError;
use contracts::chat::ChatTurnInput;
use uuid::Uuid;

use crate::agents;
use crate::context::ChatContext;
use app_documents::build_docscope_metadata;

impl ChatContext {
    pub fn agent_service(&self) -> Option<Arc<UnifiedAgentService>> {
        self.orchestrator.agent_service()
    }

    pub async fn load_docscope_metadata(
        &self,
        doc_scope: &[String],
    ) -> Result<common::DocScopeMetadata, AppError> {
        let pg_opt = self.storage.chat_persistence();
        let pg = pg_opt
            .as_ref()
            .ok_or_else(|| AppError::internal("postgres backend is not configured"))?;

        let doc_uuids: Vec<Uuid> = doc_scope
            .iter()
            .filter_map(|id| Uuid::parse_str(id).ok())
            .collect();

        let metadata = pg.get_summary_metadata(&self.auth, &doc_uuids).await?;

        Ok(build_docscope_metadata(metadata))
    }

    pub async fn build_session_context(
        &self,
        session: &contracts::notebooks::ChatSession,
    ) -> Result<Option<RagSessionContext>, AppError> {
        let session_uuid = Uuid::parse_str(&session.id).map_err(|_| {
            AppError::validation("invalid_session_id", "invalid session UUID format")
        })?;

        let pg_opt = self.storage.chat_persistence();
        let pg = pg_opt
            .as_ref()
            .ok_or_else(|| AppError::internal("postgres backend is not configured"))?;

        let messages = pg
            .list_messages(&self.auth, session_uuid)
            .await
            .unwrap_or_default();
        if messages.is_empty() {
            return Ok(None);
        }

        Ok(Self::build_rag_session_context(messages))
    }

    pub async fn get_notebook(&self, notebook_id: &str) -> Option<contracts::notebooks::Notebook> {
        if let Some(pg) = self.storage.chat_persistence() {
            let notebook_id = Uuid::parse_str(notebook_id).ok()?;
            let notebook = pg
                .get_notebook(&self.auth, notebook_id)
                .await
                .ok()
                .flatten()?;
            return (notebook.org_id == self.current_org_id()).then_some(notebook);
        }
        let state = self.storage.inner().read().await;
        state
            .notebooks
            .get(notebook_id)
            .filter(|notebook| notebook.org_id == self.current_org_id())
            .cloned()
    }

    pub async fn remember_explicit_agent_preference(&self, query: &str) -> Result<(), AppError> {
        self.admin
            .remember_explicit_agent_preference(&self.auth, &self.storage, query)
            .await
    }

    pub async fn current_user_preferences(&self) -> Result<contracts::UserPreferences, AppError> {
        self.admin
            .current_user_preferences(&self.auth, &self.storage)
            .await
    }

    /// Resolve conversation history for agent prompts.
    pub async fn resolve_agent_messages(
        &self,
        req: &contracts::chat::ChatRequest,
    ) -> Vec<ChatTurnInput> {
        if !req.messages.is_empty() {
            return req.messages.clone();
        }

        let Some(session_id) = req.session_id.as_ref() else {
            return Vec::new();
        };
        let Ok(session_uuid) = Uuid::parse_str(session_id) else {
            return Vec::new();
        };
        let Some(pg) = self.storage.chat_persistence() else {
            return Vec::new();
        };

        let Ok(stored) = pg.list_messages(&self.auth, session_uuid).await else {
            return Vec::new();
        };

        let current_query = req.query.trim();
        let history: Vec<ChatTurnInput> = stored
            .into_iter()
            .filter(|message| message.role == "user")
            .filter(|message| !message.content.trim().is_empty())
            .filter(|message| message.content.trim() != current_query)
            .map(|message| ChatTurnInput {
                role: message.role,
                content: message.content,
                // ADR-0010: resolved_query no longer computed; field retained
                // for backward-compatible deserialization of older clients.
                resolved_query: None,
            })
            .collect();

        agents::runtime::recent_messages(&history, agents::runtime::MAX_PROMPT_HISTORY_TURNS)
            .to_vec()
    }

    pub async fn build_agent_request(
        &self,
        req: &contracts::chat::ChatRequest,
        kind: agents::AgentKind,
        session_id_override: Option<String>,
    ) -> agents::runtime::AgentRequest {
        let notebook_id = req.notebook_id.clone();
        let session_id = session_id_override.or_else(|| req.session_id.clone());
        let doc_scope = req.doc_scope.clone();
        let stream = req.stream;

        let memory_context =
            if let (Some(sid), Some(cm)) = (&session_id, self.orchestrator.chatmemory()) {
                if let Ok(session_uuid) = Uuid::parse_str(sid) {
                    cm.load(&self.auth, session_uuid).await.ok()
                } else {
                    None
                }
            } else {
                None
            };
        let user_preferences = memory_context
            .as_ref()
            .and_then(|memory| memory.layer3.as_ref().map(agent_user_preferences_json));
        let messages = self.resolve_agent_messages(req).await;
        agents::runtime::AgentRequest {
            kind,
            query: req.query.clone(),
            notebook_id,
            session_id,
            doc_scope,
            messages,
            user_preferences,
            debug: req.debug,
            stream,
            language: req.language.clone(),
            auth_context: serde_json::to_value(&self.auth)
                .unwrap_or_else(|_| serde_json::json!({})),
            docscope_metadata: None,
            metadata: BTreeMap::new(),
            cancellation_token: None,
            guard_pipeline: None,
            preferred_tools: vec![],
            format_hint: req.format_hint.clone(),
            max_iterations: None,
        }
    }

    pub fn build_general_agent_debug(
        &self,
        agent_request: &agents::runtime::AgentRequest,
    ) -> BTreeMap<String, serde_json::Value> {
        let mut general_debug = BTreeMap::new();
        general_debug.insert(
            "agent_kind".to_string(),
            serde_json::json!(agents::AgentKind::Chat.as_canonical_str()),
        );
        general_debug.insert(
            "memory_loaded".to_string(),
            serde_json::json!(
                !agent_request.messages.is_empty() || agent_request.user_preferences.is_some()
            ),
        );
        general_debug.insert("summary_updated".to_string(), serde_json::json!(false));
        general_debug.insert(
            "has_profile".to_string(),
            serde_json::json!(agent_request.user_preferences.is_some()),
        );
        general_debug
    }
}

fn agent_user_preferences_json(profile: &avrag_chatmemory::Layer3Profile) -> serde_json::Value {
    let mut base = serde_json::json!({
        "expertise_domains": profile.expertise_domains.clone(),
        "preferred_answer_style": profile.preferred_answer_style.clone(),
        "frequently_asked_topics": profile.frequently_asked_topics.clone(),
        "custom_preferences": profile.custom_preferences.clone(),
        "inference_version": profile.inference_version.clone(),
    });
    if let (Some(base_obj), Some(profile_obj)) =
        (base.as_object_mut(), profile.structured_profile.as_object())
    {
        for (key, value) in profile_obj {
            base_obj.insert(key.clone(), value.clone());
        }
    }
    base
}
