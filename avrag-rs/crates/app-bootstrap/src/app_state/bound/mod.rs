//! Bound domain handles (TN Wave 3 / W2).
//!
//! Product HTTP handlers must use these faces — not raw store accessors on AppState:
//! `docs()` / `chat()` / `admin_api()` / `admin_ops()` / `share()` / `prefs()` / `billing_api()`.

mod documents;
mod admin;
mod admin_ops;
mod share;
mod billing;
mod prefs;

pub use documents::BoundDocuments;
pub use admin::BoundAdmin;
pub use admin_ops::BoundAdminOps;
pub use share::BoundShare;
pub use billing::BoundBilling;
pub use prefs::BoundPrefs;

use contracts::auth_runtime::OrgId;
use uuid::Uuid;

use super::AppState;

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
    /// Bound document/notebook/source API (`auth` + `storage` + billing/analytics wired).
    pub fn docs(&self) -> BoundDocuments<'_> {
        BoundDocuments {
            docs: &self.documents,
            auth: &self.auth,
            storage: &self.storage,
            billing: &self.billing,
            analytics: &self.analytics,
        }
    }

    /// Bound admin API-key / notification surface.
    pub fn admin_api(&self) -> BoundAdmin<'_> {
        BoundAdmin {
            admin: &self.admin,
            auth: &self.auth,
            storage: &self.storage,
            postgres: self.postgres.clone(),
        }
    }

    /// Bound share / collab surface.
    pub fn share(&self) -> BoundShare<'_> {
        BoundShare {
            auth: &self.auth,
            storage: &self.storage,
            docs: &self.documents,
        }
    }

    /// Bound user preferences surface.
    pub fn prefs(&self) -> BoundPrefs<'_> {
        BoundPrefs {
            admin: &self.admin,
            auth: &self.auth,
            storage: &self.storage,
        }
    }

    /// Bound billing / usage / checkout surface (not `billing()` → `&BillingContext`).
    pub fn billing_api(&self) -> BoundBilling<'_> {
        BoundBilling {
            auth: &self.auth,
            storage: &self.storage,
            postgres: self.postgres.clone(),
        }
    }

    /// Bound super-admin / ops console (`AdminStorePort` + auth).
    /// Prefer this over `admin_store()` in HTTP handlers.
    pub fn admin_ops(&self) -> BoundAdminOps<'_> {
        BoundAdminOps {
            auth: &self.auth,
            store: self.storage.admin_store(),
        }
    }
}
