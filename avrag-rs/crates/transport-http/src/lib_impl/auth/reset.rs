use app_bootstrap::AppState;
use app_bootstrap::PasswordResetError;
use app_bootstrap::PasswordResetService;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use serde_json::json;
use uuid::Uuid;

use crate::auth_types::AuthEnvelope;
use crate::auth_types::AuthPayload;
use crate::auth_types::AuthUserDto;
use crate::auth_types::ConfirmResetPasswordRequest;
use crate::auth_types::ResetPasswordRequest;
use crate::auth_types::ResetRequest;
use crate::auth_types::SendResetCodeRequest;
use crate::auth_types::VerifyResetCodeRequest;
use crate::auth_types::VerifyResetTokenRequest;
use crate::handlers;

use super::super::router_core::record_api_product_event_if_available;

fn password_reset_error_response(error: PasswordResetError) -> Response {
    match error {
        PasswordResetError::NotEnabled => handlers::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "password_reset_unavailable",
            "Password reset is not available in this environment",
        ),
        PasswordResetError::StoreLookupFailed | PasswordResetError::TicketCreateFailed => {
            handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to initiate password reset",
            )
        }
        PasswordResetError::EmailSendFailed => handlers::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Failed to send reset code",
        ),
        PasswordResetError::CodeVerifyFailed => handlers::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Failed to verify reset code",
        ),
        PasswordResetError::TicketVerifyFailed => handlers::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Failed to verify reset session",
        ),
        PasswordResetError::PasswordHashFailed | PasswordResetError::PasswordResetFailed => {
            handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to reset password",
            )
        }
        PasswordResetError::InvalidResetTicket => handlers::error_response(
            StatusCode::BAD_REQUEST,
            "invalid_reset_ticket",
            "Reset session is invalid or expired",
        ),
    }
}

fn password_reset_user(email: String, user_id: Uuid) -> AuthUserDto {
    AuthUserDto {
        id: user_id.to_string(),
        email,
        full_name: String::new(),
    }
}

pub(crate) async fn auth_runtime_capabilities_handler(
    State(state): State<AppState>,
) -> Response {
    let enabled = cfg!(test) || state.password_reset_service().smtp_ready();
    (
        StatusCode::OK,
        Json(contracts::AuthRuntimeCapabilitiesResponse {
            password_reset_enabled: enabled,
        }),
    )
        .into_response()
}

pub(crate) async fn auth_reset_request_handler(
    State(state): State<AppState>,
    Json(req): Json<ResetRequest>,
) -> Response {
    auth_send_reset_code_handler(
        State(state),
        Json(SendResetCodeRequest {
            email: req.email,
            lang: None,
        }),
    )
    .await
}

pub(crate) async fn auth_verify_reset_token_handler(
    State(state): State<AppState>,
    Json(req): Json<VerifyResetTokenRequest>,
) -> Response {
    let Some(store) = state.auth_store() else {
        return handlers::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };
    match state
        .password_reset_service()
        .verify_reset_token(store.as_ref(), &req.token)
        .await
    {
        Ok(true) => (
            StatusCode::OK,
            Json(AuthEnvelope {
                success: true,
                data: None,
                error: None,
            }),
        )
            .into_response(),
        Ok(false) => (
            StatusCode::OK,
            Json(AuthEnvelope {
                success: false,
                data: None,
                error: Some("Reset session is invalid or expired".to_string()),
            }),
        )
            .into_response(),
        Err(error) => password_reset_error_response(error),
    }
}

pub(crate) async fn auth_reset_password_handler(
    State(state): State<AppState>,
    Json(req): Json<ResetPasswordRequest>,
) -> Response {
    auth_confirm_reset_password_handler(
        State(state),
        Json(ConfirmResetPasswordRequest {
            reset_ticket: req.token,
            new_password: req.new_password,
        }),
    )
    .await
}

pub(crate) async fn auth_send_reset_code_handler(
    State(state): State<AppState>,
    Json(req): Json<SendResetCodeRequest>,
) -> Response {
    let email = match PasswordResetService::normalize_email(&req.email) {
        Ok(email) => email,
        Err(message) => {
            return handlers::error_response(
                StatusCode::BAD_REQUEST,
                "validation_error",
                message,
            );
        }
    };
    let Some(store) = state.auth_store() else {
        return handlers::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };
    let svc = state.password_reset_service();
    if !(cfg!(test) || svc.smtp_ready()) {
        return handlers::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "password_reset_unavailable",
            "Password reset is not available in this environment",
        );
    }

    match svc.send_reset_code(store.as_ref(), &email).await {
        Ok(None) => (
            StatusCode::ACCEPTED,
            Json(json!({
                "success": true,
                "data": null,
                "error": null,
            })),
        )
            .into_response(),
        Ok(Some(outcome)) => {
            record_api_product_event_if_available(
                &state,
                outcome.user_id,
                analytics::ProductEventName::PasswordResetRequested,
                analytics::ResultTag::Success,
                serde_json::json!({
                    "email_domain": outcome.email.split('@').nth(1).unwrap_or_default(),
                    "delivery": outcome.delivery,
                }),
            )
            .await;
            #[allow(unused_mut)]
            let mut response = json!({
                "success": true,
                "data": null,
                "error": null,
            });
            #[cfg(test)]
            {
                response["debug_code"] = json!(outcome.code);
                response["debug_reset_ticket"] = json!(outcome.reset_ticket);
            }
            (StatusCode::ACCEPTED, Json(response)).into_response()
        }
        Err(error) => password_reset_error_response(error),
    }
}

pub(crate) async fn auth_verify_reset_code_handler(
    State(state): State<AppState>,
    Json(req): Json<VerifyResetCodeRequest>,
) -> Response {
    let email = match PasswordResetService::normalize_email(&req.email) {
        Ok(email) => email,
        Err(message) => {
            return handlers::error_response(
                StatusCode::BAD_REQUEST,
                "validation_error",
                message,
            );
        }
    };
    let Some(store) = state.auth_store() else {
        return handlers::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };
    let code = req.code.trim();
    if code.is_empty() {
        return (
            StatusCode::OK,
            Json(AuthEnvelope {
                success: false,
                data: None,
                error: Some("Reset code is required".to_string()),
            }),
        )
            .into_response();
    }

    match state
        .password_reset_service()
        .verify_reset_code(store.as_ref(), &email, code)
        .await
    {
        Ok(Some(outcome)) => {
            record_api_product_event_if_available(
                &state,
                outcome.user_id,
                analytics::ProductEventName::PasswordResetVerified,
                analytics::ResultTag::Success,
                serde_json::json!({
                    "email_domain": outcome.email.split('@').nth(1).unwrap_or_default(),
                }),
            )
            .await;
            (
                StatusCode::OK,
                Json(AuthEnvelope {
                    success: true,
                    data: Some(AuthPayload {
                        token: String::new(),
                        user: password_reset_user(outcome.email, outcome.user_id),
                        reset_ticket: Some(outcome.reset_ticket),
                    }),
                    error: None,
                }),
            )
                .into_response()
        }
        Ok(None) => (
            StatusCode::OK,
            Json(AuthEnvelope {
                success: false,
                data: None,
                error: Some("Reset code is invalid or expired".to_string()),
            }),
        )
            .into_response(),
        Err(error) => password_reset_error_response(error),
    }
}

pub(crate) async fn auth_confirm_reset_password_handler(
    State(state): State<AppState>,
    Json(req): Json<ConfirmResetPasswordRequest>,
) -> Response {
    let ticket = req.reset_ticket.trim();
    if ticket.is_empty() {
        return handlers::error_response(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "Reset ticket is required",
        );
    }
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

    match state
        .password_reset_service()
        .confirm_reset_password(store.as_ref(), ticket, &req.new_password)
        .await
    {
        Ok(user_id) => {
            record_api_product_event_if_available(
                &state,
                user_id,
                analytics::ProductEventName::PasswordResetCompleted,
                analytics::ResultTag::Success,
                serde_json::json!({
                    "flow": "reset_ticket",
                }),
            )
            .await;
            (
                StatusCode::OK,
                Json(AuthEnvelope {
                    success: true,
                    data: None,
                    error: None,
                }),
            )
                .into_response()
        }
        Err(error) => password_reset_error_response(error),
    }
}
