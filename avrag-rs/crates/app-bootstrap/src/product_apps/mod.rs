//! Product Apps (ADR-0007) — use-case entry points assembled by AppState (composition root).
//!
//! **Freeze:** do not add business methods on AppState; add them on the relevant `*App` here
//! (or in domain crates behind the App). Bound faces under `app_state::bound` are removed.

mod share;
mod workspace;
mod billing;
mod prefs;
mod admin;
mod admin_ops;
mod agent;
mod write;

pub use share::ShareApp;
pub use workspace::WorkspaceApp;
pub use billing::BillingApp;
pub use prefs::PrefsApp;
pub use admin::AdminApp;
pub use admin_ops::AdminOpsApp;
pub use agent::AgentApp;
pub use write::WriteApp;

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

    /// Alias for workspace product App (historical `docs()` name).
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

    /// Billing product App.
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

    /// Agent product App (Chat/RAG/Search). Tools via ToolCatalog only.
    pub fn agent(&self) -> AgentApp<'_> {
        AgentApp {
            chat: &self.chat,
            auth: &self.auth,
        }
    }

    /// Write product App (refine loop; never ToolCatalog).
    pub fn write_app(&self) -> WriteApp<'_> {
        WriteApp {
            chat: &self.chat,
            auth: &self.auth,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WriteApp;
    use crate::AppState;
    use app_core::AppConfig;

    #[test]
    fn composition_root_exposes_all_product_apps() {
        let state = AppState::new(AppConfig::default());
        let _ = state.workspace();
        let _ = state.docs();
        let _ = state.share();
        let _ = state.billing_api();
        let _ = state.prefs();
        let _ = state.admin_api();
        let _ = state.admin_ops();
        let _ = state.agent();
        let _ = state.write_app();
        assert!(WriteApp::WRITE_REFINE_OUTSIDE_TOOL_CATALOG);
    }
}
