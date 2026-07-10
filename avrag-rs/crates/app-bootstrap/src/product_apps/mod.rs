//! Product Apps (ADR-0007) — use-case entry points assembled by AppState (composition root).
//!
//! **Freeze:** do not add business methods on AppState; add them on the relevant `*App` here
//! (or in domain crates behind the App).
//!
//! Product accessors: `conversation()`, `workspace()`, `share()`, `billing()`, `prefs()`,
//! `admin_api()`, `admin_ops()`, `agent()`, `write()`. Historical aliases may be deprecated.

mod share;
mod workspace;
mod billing;
mod prefs;
mod admin;
mod admin_ops;
mod agent;
mod write;
mod conversation;

pub use share::ShareApp;
pub use workspace::WorkspaceApp;
pub use billing::BillingApp;
pub use prefs::PrefsApp;
pub use admin::AdminApp;
pub use admin_ops::AdminOpsApp;
pub use agent::AgentApp;
pub use write::WriteApp;
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
    /// Single conversation execute entry (chat/rag/search/write). Prefer over agent/write split.
    pub fn conversation(&self) -> ConversationApp<'_> {
        ConversationApp {
            chat: &self.chat,
            auth: &self.auth,
        }
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

    /// Alias for workspace (historical `docs()` name).
    #[deprecated(note = "use workspace()")]
    pub fn docs(&self) -> WorkspaceApp<'_> {
        self.workspace()
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

    /// Agent product App (sessions / search / non-write execute helpers).
    pub fn agent(&self) -> AgentApp<'_> {
        AgentApp {
            chat: &self.chat,
            auth: &self.auth,
        }
    }

    /// Write product App (refine loop). Prefer `conversation().execute` from transport.
    pub fn write(&self) -> WriteApp<'_> {
        WriteApp {
            chat: &self.chat,
            auth: &self.auth,
        }
    }

    /// Alias for write (historical name).
    #[deprecated(note = "use write()")]
    pub fn write_app(&self) -> WriteApp<'_> {
        self.write()
    }
}

#[cfg(test)]
mod tests {
    use super::{ConversationApp, WriteApp};
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
        let _ = state.write();
        let _ = state.conversation();
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
    async fn write_app_rejects_non_write_and_uses_write_pipeline() {
        let state = AppState::new(AppConfig::default());
        let err = state
            .write()
            .execute(empty_chat_req("chat"))
            .await
            .expect_err("WriteApp rejects chat");
        assert_eq!(err.code(), "write_mode_required");

        let empty = state
            .write()
            .execute(empty_chat_req("write"))
            .await
            .expect_err("empty write");
        assert_eq!(empty.code(), "query_required");
        assert!(WriteApp::is_write_agent_type("Write"));
        let _ = ConversationApp {
            chat: &state.chat,
            auth: &state.auth,
        };
    }

    #[tokio::test]
    async fn agent_rejects_write_on_agent_lane() {
        let state = AppState::new(AppConfig::default());
        let err = state
            .agent()
            .execute_chat(empty_chat_req("write"))
            .await
            .expect_err("agent lane rejects write");
        assert_eq!(err.code(), "use_write_entry");
    }
}
