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

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn mount() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

// Re-export route components for use in route definitions
pub use routes::{
    AdminShell, ApiAccessPage, AuditLogsPage, BillingPage, ConfirmResetPage, DashboardListPage,
    DegradationPage, FeatureFlagsPage, HealthPage, HelpPage, HomePage, InvitePage, LoginPage,
    OrgDetailPage, OrganizationsPage, PreviewAccountPage, PreviewDashboardPage, PreviewEntryPage,
    PreviewHelpPage, PreviewLoginPage, PreviewSettingsPage, PreviewWorkspacePage, RagHealthPage,
    RegisterPage, ResetPasswordPage, SearchPage, SettingsPage, ShareCenterPage, SharedKbPage,
    UsagePage, UsersPage, VerifyResetPage, WorkerStatusPage, WorkspacePage,
};
