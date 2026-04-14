#![recursion_limit = "4096"]

//! web-ui — Leptos SSR frontend for Avrag

pub mod api;
pub mod app;
pub mod components;
pub mod i18n;
pub mod load;
pub mod platform;
pub mod routes;
pub mod state;

pub use app::{App, Shell};

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}

// Re-export route components for use in route definitions
pub use routes::{
    AdminShell, ApiAccessPage, AuditLogsPage, BillingPage, ConfirmResetPage, DashboardListPage,
    DegradationPage, FeatureFlagsPage, HealthPage, HomePage, InvitePage, LoginPage, OrgDetailPage,
    OrganizationsPage, RagHealthPage, RegisterPage, ResetPasswordPage, SearchPage, SettingsPage,
    ShareCenterPage, SharedKbPage, UsagePage, UsersPage, VerifyResetPage, WorkerStatusPage,
    WorkspacePage,
};
