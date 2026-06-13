use app_bootstrap::AppState;
use axum::{
    Router,
    routing::{delete, get, post, put},
};

pub(crate) fn public_router() -> Router<AppState> {
    Router::new()
        .route(
            "/capabilities",
            get(crate::auth_runtime_capabilities_handler),
        )
        .route("/register", post(crate::auth_register_handler))
        .route("/login", post(crate::auth_login_handler))
        .route("/reset-request", post(crate::auth_reset_request_handler))
        .route(
            "/verify-reset-token",
            post(crate::auth_verify_reset_token_handler),
        )
        .route("/reset-password", post(crate::auth_reset_password_handler))
        .route(
            "/reset/send-code",
            post(crate::auth_send_reset_code_handler),
        )
        .route(
            "/reset/verify-code",
            post(crate::auth_verify_reset_code_handler),
        )
        .route(
            "/reset/confirm",
            post(crate::auth_confirm_reset_password_handler),
        )
}

pub(crate) fn protected_router() -> Router<AppState> {
    Router::new()
        .route("/logout", post(crate::auth_logout_handler))
        .route("/me", get(crate::auth_me_handler))
        .route("/profile", put(crate::auth_update_profile_handler))
        .route(
            "/preferences",
            get(crate::auth_get_preferences_handler).put(crate::auth_update_preferences_handler),
        )
        .route(
            "/agent-preferences",
            get(crate::auth_get_agent_preferences_handler)
                .put(crate::auth_update_agent_preferences_handler),
        )
        .route(
            "/agent-preferences/{preference_id}",
            delete(crate::auth_delete_agent_preference_handler),
        )
        .route(
            "/change-password",
            post(crate::auth_change_password_handler),
        )
        .route(
            "/legal-acceptance",
            post(crate::auth_record_legal_acceptance_handler),
        )
        .route("/legal-status", get(crate::auth_legal_status_handler))
        .route("/usage-limit", get(crate::usage_limit_handler))
}
