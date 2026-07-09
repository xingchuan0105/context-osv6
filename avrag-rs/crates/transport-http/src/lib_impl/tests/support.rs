//! Shared fixtures for lib_impl HTTP tests (W3).

pub(super) use app_bootstrap::AppState;
pub(super) use axum::body::{Body, to_bytes};
pub(super) use axum::http::Request;
pub(super) use axum::http::StatusCode;
pub(super) use common::CreateNotebookRequest;
pub(super) use serde_json::json;
pub(super) use std::env;
pub(super) use tower::ServiceExt;
pub(super) use uuid::Uuid;

pub(super) use super::super::{
    build_router, issue_jwt, issue_jwt_for_auth_version, verify_jwt,
};
pub(super) use crate::middleware;

pub(super) fn test_app_state() -> AppState {
    let mut config = app_core::AppConfig::default();
    config.org_id = "00000000-0000-0000-0000-000000000001".to_string();
    config.user_id = "00000000-0000-0000-0000-000000000002".to_string();
    AppState::new(config)
}

pub(super) async fn pg_test_app_state() -> Option<AppState> {
    let database_url = env::var("DATABASE_URL").ok()?;
    let mut config = app_core::AppConfig::default();
    config.database_url = Some(database_url);
    config.auto_migrate = true;
    AppState::bootstrap(config).await.ok()
}

pub(super) fn register_body(email: &str, full_name: &str) -> String {
    format!(
        r#"{{"email":"{email}","password":"password123","full_name":"{full_name}","terms_version":"{}","privacy_version":"{}"}}"#,
        app_core::PUBLISHED_TERMS_VERSION,
        app_core::PUBLISHED_PRIVACY_VERSION,
    )
}
