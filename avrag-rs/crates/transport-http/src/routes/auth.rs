use app_bootstrap::AppState;
use axum::{
    Router,
    routing::{delete, get, post, put},
};

pub(crate) fn public_router() -> Router<AppState> {
    Router::new()
        .route(
            "/capabilities",
            get(crate::lib_impl::auth_runtime_capabilities_handler),
        )
        .route("/register", post(crate::lib_impl::auth_register_handler))
        .route("/login", post(crate::lib_impl::auth_login_handler))
        .route("/reset-request", post(crate::lib_impl::auth_reset_request_handler))
        .route(
            "/verify-reset-token",
            post(crate::lib_impl::auth_verify_reset_token_handler),
        )
        .route("/reset-password", post(crate::lib_impl::auth_reset_password_handler))
        .route(
            "/reset/send-code",
            post(crate::lib_impl::auth_send_reset_code_handler),
        )
        .route(
            "/reset/verify-code",
            post(crate::lib_impl::auth_verify_reset_code_handler),
        )
        .route(
            "/reset/confirm",
            post(crate::lib_impl::auth_confirm_reset_password_handler),
        )
}

pub(crate) fn protected_router() -> Router<AppState> {
    Router::new()
        .route("/logout", post(crate::lib_impl::auth_logout_handler))
        .route("/me", get(crate::lib_impl::auth_me_handler))
        .route("/profile", put(crate::lib_impl::auth_update_profile_handler))
        .route(
            "/preferences",
            get(crate::lib_impl::auth_get_preferences_handler).put(crate::lib_impl::auth_update_preferences_handler),
        )
        .route(
            "/agent-preferences",
            get(crate::lib_impl::auth_get_agent_preferences_handler)
                .put(crate::lib_impl::auth_update_agent_preferences_handler),
        )
        .route(
            "/agent-preferences/{preference_id}",
            delete(crate::lib_impl::auth_delete_agent_preference_handler),
        )
        .route(
            "/change-password",
            post(crate::lib_impl::auth_change_password_handler),
        )
        .route(
            "/legal-acceptance",
            post(crate::lib_impl::auth_record_legal_acceptance_handler),
        )
        .route("/legal-status", get(crate::lib_impl::auth_legal_status_handler))
        .route("/usage-limit", get(crate::lib_impl::usage_limit_handler))
}
