use axum::Json;
use axum::extract::Extension;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use bcrypt::DEFAULT_COST;
use bcrypt::hash;
use bcrypt::verify;
use serde_json::json;
use tracing::warn;

use crate::auth_types::AuthEnvelope;
use crate::auth_types::AuthPayload;
use crate::auth_types::AuthUserDto;
use crate::auth_types::ChangePasswordRequest;
use crate::auth_types::LegalStatusEnvelope;
use crate::auth_types::LegalStatusPayload;
use crate::auth_types::RecordLegalAcceptanceRequest;
use crate::auth_types::UpdateProfileRequest;
use crate::handlers;
use crate::middleware::RequestState;

pub(crate) async fn auth_logout_handler(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Response {
    if let Err(error) = crate::auth_guard::forbid_api_key(
        state.auth(),
        "logout requires a signed-in user session",
    ) {
        return handlers::app_error_response(error);
    }
    let Some(user_id) = state.auth().actor_id() else {
        return handlers::error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Not authenticated",
        );
    };
    let Some(store) = state.auth_store() else {
        return handlers::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };

    match store.invalidate_session(user_id.into_uuid()).await {
        Ok(true) => (
            StatusCode::OK,
            Json(AuthEnvelope {
                success: true,
                data: None,
                error: None,
            }),
        )
            .into_response(),
        Ok(false) => handlers::error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Not authenticated",
        ),
        Err(error) => {
            warn!(error = %error, "failed to invalidate session on logout");
            handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Logout failed",
            )
        }
    }
}

pub(crate) async fn auth_me_handler(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Response {
    if let Err(error) = crate::auth_guard::forbid_api_key(
        state.auth(),
        "profile requires a signed-in user session",
    ) {
        return handlers::app_error_response(error);
    }
    let Some(user_id) = state.auth().actor_id() else {
        return handlers::error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Not authenticated",
        );
    };
    let user_uuid = user_id.into_uuid();

    let Some(store) = state.auth_store() else {
        return handlers::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };

    match store.get_user_profile(user_uuid).await {
        Ok(Some(profile)) => (
            StatusCode::OK,
            Json(AuthEnvelope {
                success: true,
                data: Some(AuthPayload {
                    token: String::new(),
                    user: AuthUserDto {
                        id: profile.user_id.to_string(),
                        email: profile.email,
                        full_name: profile.full_name.unwrap_or_default(),
                    },
                    reset_ticket: None,
                }),
                error: None,
            }),
        )
            .into_response(),
        Ok(None) => handlers::error_response(
            StatusCode::NOT_FOUND,
            "user_not_found",
            "User profile not found",
        ),
        Err(error) => {
            warn!(error = %error, "failed to load profile");
            handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to load profile",
            )
        }
    }
}

pub(crate) async fn auth_update_profile_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(req): Json<UpdateProfileRequest>,
) -> Response {
    if let Err(error) = crate::auth_guard::forbid_api_key(
        state.auth(),
        "profile updates require a signed-in user session",
    ) {
        return handlers::app_error_response(error);
    }
    let Some(user_id) = state.auth().actor_id() else {
        return handlers::error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Not authenticated",
        );
    };
    let Some(store) = state.auth_store() else {
        return handlers::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };

    let full_name = req.full_name.unwrap_or_default();
    let user_uuid = user_id.into_uuid();

    match store
        .update_user_profile(user_uuid, &full_name)
        .await
    {
        Ok(Some(profile)) => (
            StatusCode::OK,
            Json(AuthEnvelope {
                success: true,
                data: Some(AuthPayload {
                    token: String::new(),
                    user: AuthUserDto {
                        id: profile.user_id.to_string(),
                        email: profile.email,
                        full_name: profile.full_name.unwrap_or_default(),
                    },
                    reset_ticket: None,
                }),
                error: None,
            }),
        )
            .into_response(),
        Ok(None) => handlers::error_response(
            StatusCode::NOT_FOUND,
            "user_not_found",
            "User profile not found",
        ),
        Err(error) => {
            warn!(error = %error, "failed to update profile");
            handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Profile update failed",
            )
        }
    }
}
pub(crate) async fn auth_change_password_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(req): Json<ChangePasswordRequest>,
) -> Response {
    if let Err(error) = crate::auth_guard::forbid_api_key(
        state.auth(),
        "password changes require a signed-in user session",
    ) {
        return handlers::app_error_response(error);
    }
    let Some(user_id) = state.auth().actor_id() else {
        return handlers::error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Not authenticated",
        );
    };
    if req.new_password.len() < 8 {
        return handlers::error_response(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "New password must be at least 8 characters",
        );
    }
    let Some(store) = state.auth_store() else {
        return handlers::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };

    let user_uuid = user_id.into_uuid();

    let stored_hash = match store.get_password_hash(user_uuid).await {
        Ok(Some(hash)) => hash,
        Ok(None) => {
            return handlers::error_response(
                StatusCode::NOT_FOUND,
                "user_not_found",
                "User profile not found",
            );
        }
        Err(error) => {
            warn!(error = %error, "failed to load password hash");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Password update failed",
            );
        }
    };

    match verify(&req.old_password, &stored_hash) {
        Ok(true) => {}
        _ => {
            return handlers::error_response(
                StatusCode::UNAUTHORIZED,
                "invalid_credentials",
                "Current password is incorrect",
            );
        }
    }

    let new_hash = match hash(&req.new_password, DEFAULT_COST) {
        Ok(value) => value,
        Err(error) => {
            warn!(error = %error, "password hashing failed");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Password update failed",
            );
        }
    };

    match store.change_password(user_uuid, &new_hash).await {
        Ok(()) => (
            StatusCode::OK,
            Json(AuthEnvelope {
                success: true,
                data: None,
                error: None,
            }),
        )
            .into_response(),
        Err(error) => {
            warn!(error = %error, "failed to update password");
            handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Password update failed",
            )
        }
    }
}
pub(crate) async fn auth_legal_status_handler(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Response {
    if let Err(error) = crate::auth_guard::forbid_api_key(
        state.auth(),
        "legal status requires a signed-in user session",
    ) {
        return handlers::app_error_response(error);
    }
    let Some(user_id) = state.auth().actor_id() else {
        return handlers::error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Not authenticated",
        );
    };

    let Some(store) = state.auth_store() else {
        return handlers::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };

    match store.get_user_legal_status(user_id.into_uuid()).await {
        Ok(status) => (
            StatusCode::OK,
            Json(LegalStatusEnvelope {
                success: true,
                data: Some(LegalStatusPayload {
                    needs_re_acceptance: status.needs_re_acceptance,
                    accepted_terms_version: status.accepted_terms_version,
                    accepted_privacy_version: status.accepted_privacy_version,
                    published_terms_version: status.published_terms_version,
                    published_privacy_version: status.published_privacy_version,
                }),
                error: None,
            }),
        )
            .into_response(),
        Err(error) => {
            warn!(error = %error, "failed to load legal status");
            handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to load legal status",
            )
        }
    }
}

pub(crate) async fn auth_record_legal_acceptance_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    headers: HeaderMap,
    Json(req): Json<RecordLegalAcceptanceRequest>,
) -> Response {
    if let Err(error) = crate::auth_guard::forbid_api_key(
        state.auth(),
        "legal acceptance requires a signed-in user session",
    ) {
        return handlers::app_error_response(error);
    }
    let Some(user_id) = state.auth().actor_id() else {
        return handlers::error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Not authenticated",
        );
    };

    let context = req.context.trim();
    if context != "payment" && context != "re_acceptance" {
        return handlers::error_response(
            StatusCode::BAD_REQUEST,
            "invalid_context",
            "context must be payment or re_acceptance",
        );
    }

    if let Err(error) =
        app_core::validate_published_legal_versions(&req.terms_version, &req.privacy_version)
    {
        return handlers::error_response(
            StatusCode::BAD_REQUEST,
            error.code(),
            error.message(),
        );
    }

    let Some(store) = state.auth_store() else {
        return handlers::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };

    let ip_address = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string());
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    match store
        .record_legal_acceptance(&app_core::RecordLegalAcceptanceInput {
            user_id: user_id.into_uuid(),
            terms_version: req.terms_version,
            privacy_version: req.privacy_version,
            context: context.to_string(),
            ip_address,
            user_agent,
        })
        .await
    {
        Ok(()) => (
            StatusCode::CREATED,
            Json(AuthEnvelope {
                success: true,
                data: None,
                error: None,
            }),
        )
            .into_response(),
        Err(error) => {
            warn!(error = %error, "failed to record legal acceptance");
            handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to record legal acceptance",
            )
        }
    }
}

pub(crate) async fn usage_limit_handler(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Response {
    if let Err(error) = crate::auth_guard::forbid_api_key(
        state.auth(),
        "usage limits require a signed-in user session",
    ) {
        return handlers::app_error_response(error);
    }
    match state.agent().get_user_usage_limit().await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "internal_error", "message": "Usage limit service unavailable"})),
        )
            .into_response(),
    }
}
