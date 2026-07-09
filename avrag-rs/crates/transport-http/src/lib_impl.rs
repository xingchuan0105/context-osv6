pub(crate) mod router_core;
mod auth_primary;
mod auth {
    pub(crate) mod profile;
    pub(crate) mod preferences;
    pub(crate) mod reset;
}
mod infra_handlers;

#[cfg(test)]
mod tests;

pub use router_core::build_router;
pub(crate) use router_core::{extract_bearer, verify_jwt};
pub use router_core::{issue_jwt, issue_jwt_for_auth_version};
pub(crate) use auth_primary::{auth_login_handler, auth_register_handler};
pub(crate) use auth::preferences::{
    auth_delete_agent_preference_handler, auth_get_agent_preferences_handler,
    auth_get_preferences_handler, auth_update_agent_preferences_handler,
    auth_update_preferences_handler,
};
pub(crate) use auth::profile::{
    auth_change_password_handler, auth_legal_status_handler, auth_logout_handler, auth_me_handler,
    auth_record_legal_acceptance_handler, auth_update_profile_handler, usage_limit_handler,
};
pub(crate) use auth::reset::{
    auth_confirm_reset_password_handler, auth_reset_password_handler, auth_reset_request_handler,
    auth_runtime_capabilities_handler, auth_send_reset_code_handler, auth_verify_reset_code_handler,
    auth_verify_reset_token_handler,
};
pub(crate) use infra_handlers::{
    billing_webhook_handler, docs_handler, health_handler,
    metrics_handler, object_storage_webhook_handler, openai_chat_completions_handler,
    openapi_handler, ready_handler, shared_notebook_handler, signed_upload_handler,
};
