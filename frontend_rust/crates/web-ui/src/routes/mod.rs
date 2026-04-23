//! Routes module - defines all application routes using leptos_router

pub mod admin;
pub mod analyze;
pub mod api_access;
pub mod auth;
pub mod dashboard;
pub mod help;
pub mod home;
pub mod invite;
pub mod preview;
pub mod search;
pub mod settings;
pub mod shared;

// Page components with route attributes
pub use admin::{
    AdminShell, AuditLogsPage, BillingPage, DegradationPage, FeatureFlagsPage, HealthPage,
    OrgDetailPage, OrganizationsPage, RagHealthPage, UsagePage, UsersPage, WorkerStatusPage,
};
pub use analyze::WorkspaceAnalyzePage;
pub use api_access::ApiAccessPage;
pub use auth::{ConfirmResetPage, LoginPage, RegisterPage, ResetPasswordPage, VerifyResetPage};
pub use dashboard::{DashboardListPage, WorkspacePage};
pub use help::HelpPage;
pub use home::HomePage;
pub use invite::InvitePage;
pub use preview::{
    PreviewAccountPage, PreviewDashboardPage, PreviewEntryPage, PreviewHelpPage, PreviewLoginPage,
    PreviewSettingsPage, PreviewWorkspacePage,
};
pub use search::SearchPage;
pub use settings::SettingsPage;
pub use shared::{ShareCenterPage, SharedKbPage};
