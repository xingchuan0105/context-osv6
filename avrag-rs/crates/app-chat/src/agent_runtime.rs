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
        let pg = self
            .storage
            .chat_persistence()
            .ok_or_else(|| AppError::internal("chat persistence is not configured"))?;

        let doc_uuids: Vec<Uuid> = doc_scope
            .iter()
            .filter_map(|id| Uuid::parse_str(id).ok())
            .collect();

        let metadata = pg.get_summary_metadata(&self.auth, &doc_uuids).await?;

        Ok(build_docscope_metadata(metadata))
    }

    pub async fn build_session_context(
        &self,
        session: &contracts::workspaces::ChatSession,
    ) -> Result<Option<RagSessionContext>, AppError> {
        let session_uuid = Uuid::parse_str(&session.id).map_err(|_| {
            AppError::validation("invalid_session_id", "invalid session UUID format")
        })?;

        let pg = self
            .storage
            .chat_persistence()
            .ok_or_else(|| AppError::internal("chat persistence is not configured"))?;

        let messages = pg
            .list_messages(&self.auth, session_uuid)
            .await
            .unwrap_or_default();
        if messages.is_empty() {
            return Ok(None);
        }

        Ok(Self::build_rag_session_context(messages))
    }

    pub async fn get_workspace(
        &self,
        workspace_id: &str,
    ) -> Option<contracts::workspaces::Workspace> {
        let pg = self.storage.chat_persistence()?;
        let workspace_id = Uuid::parse_str(workspace_id).ok()?;
        let notebook = pg
            .get_workspace(&self.auth, workspace_id)
            .await
            .ok()
            .flatten()?;
        (notebook.owner_user_id == self.current_owner_user_id()).then_some(notebook)
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

        // Keep only the recent prior-user window. Older turns stay in PG and are
        // loaded on demand via memory tools (`client.history`) — no host-side
        // MEMORY_LLM session summary (L2 removed; migration 0044).
        let recent_count = agent_loop::runtime::MAX_PROMPT_HISTORY_TURNS;
        agent_loop::runtime::recent_messages(&history, recent_count).to_vec()
    }

    pub async fn build_agent_request(
        &self,
        req: &contracts::chat::ChatRequest,
        kind: agents::AgentKind,
        session_id_override: Option<String>,
    ) -> agent_loop::runtime::AgentRequest {
        let workspace_id = req.workspace_id.clone();
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
        let user_preferences = memory_context.as_ref().and_then(|memory| {
            memory
                .layer3
                .as_ref()
                .map(agent_loop::runtime::AgentUserPreferences::from_layer3)
        });
        let messages = self.resolve_agent_messages(req).await;
        let mut metadata = BTreeMap::new();
        if let Some(ip) = req.client_ip.as_ref().filter(|s| !s.is_empty()) {
            metadata.insert("client_ip".to_string(), serde_json::json!(ip));
        }
        if let Some(ctx) = req.client_context.as_ref() {
            if let Some(t) = ctx.local_time.as_ref() {
                metadata.insert("client_local_time".to_string(), serde_json::json!(t));
            }
            if let Some(tz) = ctx.timezone.as_ref() {
                metadata.insert("client_timezone".to_string(), serde_json::json!(tz));
            }
        }
        // E2E budget baseline: product request path never exposes max_iterations
        // on the public HTTP body. Full-149 baseline measurement sets
        // `E2E_UNLIMITED_BUDGET=1` (→ u8::MAX rounds, token wall stays off for
        // rag/search YAML) or `E2E_MAX_ITERATIONS=N` for a fixed ceiling.
        // Production processes leave both unset → YAML mode budget only.
        let max_iterations = e2e_max_iterations_override();
        let debug = req.debug || e2e_force_debug_observe();
        agent_loop::runtime::AgentRequest {
            kind,
            query: req.query.clone(),
            workspace_id,
            session_id,
            doc_scope,
            messages,
            user_preferences,
            debug,
            stream,
            language: req.language.clone(),
            auth: self.auth.clone(),
            docscope_metadata: None,
            metadata,
            cancellation_token: None,
            guard_pipeline: None,
            preferred_tools: vec![],
            format_hint: req.format_hint.clone(),
            max_iterations,
        }
    }

    pub fn build_general_agent_debug(
        &self,
        agent_request: &agent_loop::runtime::AgentRequest,
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

/// Full-149 budget baseline only. `E2E_UNLIMITED_BUDGET=1|true` → rounds
/// ceiling `u8::MAX` (255). `E2E_MAX_ITERATIONS=N` → fixed N (1..=255).
/// Unset in production → `None` (YAML mode budget).
fn e2e_max_iterations_override() -> Option<u8> {
    if e2e_env_truthy("E2E_UNLIMITED_BUDGET") {
        return Some(u8::MAX);
    }
    std::env::var("E2E_MAX_ITERATIONS")
        .ok()
        .and_then(|v| v.trim().parse::<u8>().ok())
        .filter(|&n| n >= 1)
}

/// When measuring multi-agent packs, force `request.debug` so DebugTrace
/// (full pack payloads) is collected even if the HTTP body omitted `debug`.
fn e2e_force_debug_observe() -> bool {
    e2e_env_truthy("E2E_OBSERVE_DEBUG") || e2e_env_truthy("E2E_UNLIMITED_BUDGET")
}

fn e2e_env_truthy(name: &str) -> bool {
    match std::env::var(name) {
        Ok(v) => {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        }
        Err(_) => false,
    }
}
