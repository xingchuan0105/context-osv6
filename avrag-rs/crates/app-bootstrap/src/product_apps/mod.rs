//! Product Apps (ADR-0007) — use-case entry points assembled by AppState (composition root).
//!
//! **Freeze:** do not add business methods on AppState; add them on the relevant `*App` here
//! (or in domain crates behind the App).
//!
//! **Execute:** only `conversation().execute[_stream]`. AgentApp = sessions/search/tools.
//! Domain pipelines own write vs agent lanes (`PipelineLane`).
//!
//! Product accessors: `conversation()`, `workspace()`, `share()`, `billing_api()`,
//! `prefs()`, `admin_api()`, `admin_ops()`, `agent()`.

mod share;
mod workspace;
mod billing;
mod prefs;
mod admin;
mod admin_ops;
mod agent;
mod conversation;

pub use share::ShareApp;
pub use workspace::WorkspaceApp;
pub use billing::BillingApp;
pub use prefs::PrefsApp;
pub use admin::AdminApp;
pub use admin_ops::AdminOpsApp;
pub use agent::AgentApp;
pub use conversation::ConversationApp;

use contracts::auth_runtime::OrgId;
use uuid::Uuid;

use super::app_state::AppState;

/// Validated workspace / org API key (middleware auth path).
#[derive(Debug, Clone)]
pub struct WorkspaceApiKeyAuth {
    pub key_id: Uuid,
    pub org_id: OrgId,
    pub workspace_id: Option<Uuid>,
    pub permissions: Vec<String>,
    pub rate_limit_rpm: u32,
}

impl AppState {
    /// Single conversation execute entry (chat/rag/search/write).
    pub fn conversation(&self) -> ConversationApp<'_> {
        ConversationApp { chat: &self.chat }
    }

    /// Workspace / documents product App.
    pub fn workspace(&self) -> WorkspaceApp<'_> {
        WorkspaceApp {
            docs: &self.documents,
            auth: &self.auth,
            storage: &self.storage,
            billing: &self.billing,
            analytics: &self.analytics,
        }
    }

    /// Share / collab product App.
    pub fn share(&self) -> ShareApp<'_> {
        ShareApp {
            auth: &self.auth,
            storage: &self.storage,
            docs: &self.documents,
        }
    }

    /// Billing product App (`billing()` is reserved for raw BillingContext).
    pub fn billing_api(&self) -> BillingApp<'_> {
        BillingApp {
            auth: &self.auth,
            storage: &self.storage,
            postgres: self.postgres.clone(),
        }
    }

    /// Prefs product App.
    pub fn prefs(&self) -> PrefsApp<'_> {
        PrefsApp {
            admin: &self.admin,
            auth: &self.auth,
            storage: &self.storage,
        }
    }

    /// Admin API keys / notifications product App.
    pub fn admin_api(&self) -> AdminApp<'_> {
        AdminApp {
            admin: &self.admin,
            auth: &self.auth,
            storage: &self.storage,
            postgres: self.postgres.clone(),
        }
    }

    /// Super-admin / ops console product App.
    pub fn admin_ops(&self) -> AdminOpsApp<'_> {
        AdminOpsApp {
            auth: &self.auth,
            store: self.storage.admin_store(),
        }
    }

    /// Agent product App (sessions / search / citations / runtime tools / usage).
    pub fn agent(&self) -> AgentApp<'_> {
        AgentApp { chat: &self.chat }
    }
}

#[cfg(test)]
mod tests {
    use crate::AppState;
    use app_core::AppConfig;
    use contracts::chat::ChatRequest;

    fn empty_chat_req(agent_type: &str) -> ChatRequest {
        ChatRequest {
            query: String::new(),
            workspace_id: None,
            session_id: None,
            agent_type: agent_type.to_string(),
            source_type: None,
            source_token: None,
            doc_scope: vec![],
            messages: vec![],
            stream: false,
            debug: false,
            language: None,
            format_hint: None,
        }
    }

    #[test]
    fn composition_root_exposes_product_apps() {
        let state = AppState::new(AppConfig::default());
        let _ = state.workspace();
        let _ = state.share();
        let _ = state.billing_api();
        let _ = state.prefs();
        let _ = state.admin_api();
        let _ = state.admin_ops();
        let _ = state.agent();
        let _ = state.conversation();
        assert!(app_chat::is_write_agent_type("Write"));
    }

    #[tokio::test]
    async fn conversation_routes_write_and_chat_on_real_path() {
        let state = AppState::new(AppConfig::default());
        let conv = state.conversation();
        let write_err = conv
            .execute(empty_chat_req("write"))
            .await
            .expect_err("empty write query");
        assert_eq!(write_err.code(), "query_required");

        let chat_err = conv
            .execute(empty_chat_req("chat"))
            .await
            .expect_err("empty chat query");
        assert_eq!(chat_err.code(), "query_required");
    }

    #[tokio::test]
    async fn pipeline_defends_agent_lane_against_write_agent_type() {
        let state = AppState::new(AppConfig::default());
        let mut req = empty_chat_req("write");
        req.query = "hello".into();
        let err = state
            .chat
            .execute_chat(req)
            .await
            .expect_err("agent pipeline rejects write agent_type");
        assert_eq!(err.code(), "use_write_entry");
    }
}
