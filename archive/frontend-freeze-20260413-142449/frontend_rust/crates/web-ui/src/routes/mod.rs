//! Routes module - defines all application routes using leptos_router

pub mod admin;
pub mod api_access;
pub mod auth;
pub mod dashboard;
pub mod home;
pub mod invite;
pub mod search;
pub mod settings;
pub mod shared;

// Page components with route attributes
pub use admin::{
    AdminShell, AuditLogsPage, BillingPage, DegradationPage, FeatureFlagsPage, HealthPage,
    OrgDetailPage, OrganizationsPage, RagHealthPage, UsagePage, UsersPage, WorkerStatusPage,
};
pub use api_access::ApiAccessPage;
pub use auth::{ConfirmResetPage, LoginPage, RegisterPage, ResetPasswordPage, VerifyResetPage};
pub use dashboard::{DashboardListPage, WorkspacePage};
pub use home::HomePage;
pub use invite::InvitePage;
pub use search::SearchPage;
pub use settings::SettingsPage;
pub use shared::{ShareCenterPage, SharedKbPage};
