use std::str::FromStr;

use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Address, AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use sha2::{Digest, Sha256};

use app_bootstrap::AppState;
use app_core::CreatePasswordResetTicketInput;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use bcrypt::DEFAULT_COST;
use bcrypt::hash;
use serde_json::json;
use tracing::warn;
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

const PASSWORD_RESET_PURPOSE: &str = "password_reset";
const RESET_CODE_TTL_MINUTES: i64 = 10;
const RESET_TICKET_TTL_MINUTES: i64 = 15;
const RESET_MAX_ATTEMPTS: i32 = 5;

#[derive(Clone)]
struct PasswordResetConfig {
    email_provider: String,
    smtp_host: String,
    smtp_port: u16,
    smtp_user: String,
    smtp_pass: String,
    smtp_from: String,
    smtp_from_name: Option<String>,
    smtp_tls: bool,
    reset_code_secret: String,
}

impl PasswordResetConfig {
    fn from_env() -> Self {
        Self {
            email_provider: env_first(&["EMAIL_PROVIDER"], "smtp"),
            smtp_host: env_first(&["MAIL_HOST", "SMTP_HOST"], "smtp.163.com"),
            smtp_port: env_first(&["MAIL_PORT", "SMTP_PORT"], "465")
                .parse()
                .unwrap_or(465),
            smtp_user: env_first(&["MAIL_USER", "SMTP_USER", "SMTP_USERNAME"], ""),
            smtp_pass: env_first(&["MAIL_PASS", "SMTP_PASS", "SMTP_PASSWORD"], ""),
            smtp_from: env_first(&["MAIL_FROM", "SMTP_FROM"], ""),
            smtp_from_name: non_empty(env_first(&["SMTP_FROM_NAME"], "")),
            smtp_tls: parse_bool(&env_first(&["SMTP_TLS"], "true"), true),
            reset_code_secret: env_first(
                &["RESET_CODE_SECRET"],
                "context-osv6-local-reset-secret",
            ),
        }
    }

    fn smtp_ready(&self) -> bool {
        self.email_provider.eq_ignore_ascii_case("smtp")
            && !self.smtp_host.trim().is_empty()
            && !self.smtp_from.trim().is_empty()
    }
}

fn env_first(keys: &[&str], default: &str) -> String {
    keys.iter()
        .find_map(|key| std::env::var(key).ok().filter(|value| !value.trim().is_empty()))
        .unwrap_or_else(|| default.to_string())
}

fn parse_bool(raw: &str, default: bool) -> bool {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => default,
    }
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn normalize_email(email: &str) -> Result<String, &'static str> {
    let trimmed = email.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return Err("email is required");
    }
    Address::from_str(&trimmed).map_err(|_| "invalid email")?;
    Ok(trimmed)
}

fn generate_reset_code() -> String {
    format!("{:06}", Uuid::new_v4().as_u128() % 1_000_000)
}

fn generate_reset_ticket() -> String {
    Uuid::new_v4().to_string()
}

fn hash_reset_value(secret: &str, scope: &str, value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.update(b":");
    hasher.update(scope.as_bytes());
    hasher.update(b":");
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
}

fn password_reset_user(email: String, user_id: Uuid) -> AuthUserDto {
    AuthUserDto {
        id: user_id.to_string(),
        email,
        full_name: String::new(),
    }
}

fn password_reset_enabled(config: &PasswordResetConfig) -> bool {
    cfg!(test) || config.smtp_ready()
}

async fn send_reset_email(
    config: &PasswordResetConfig,
    to: &str,
    code: &str,
    expires_at: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<()> {
    let from_address = Address::from_str(config.smtp_from.trim())?;
    let to_address = Address::from_str(to.trim())?;
    let from = Mailbox::new(config.smtp_from_name.clone(), from_address);
    let email = Message::builder()
        .from(from)
        .to(Mailbox::new(None, to_address))
        .subject("Context OSv6 password reset code")
        .body(format!(
            "Your password reset code is: {code}\n\nThis code expires at {}.\n",
            expires_at.to_rfc3339()
        ))?;

    let mut transport = if config.smtp_tls {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_host)?
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.smtp_host)
    };
    transport = transport.port(config.smtp_port);
    if !config.smtp_user.trim().is_empty() {
        transport = transport.credentials(Credentials::new(
            config.smtp_user.clone(),
            config.smtp_pass.clone(),
        ));
    }
    transport.build().send(email).await?;
    Ok(())
}
pub(crate) async fn auth_runtime_capabilities_handler() -> Response {
    let config = PasswordResetConfig::from_env();
    (
        StatusCode::OK,
        Json(contracts::AuthRuntimeCapabilitiesResponse {
            password_reset_enabled: password_reset_enabled(&config),
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
    let config = PasswordResetConfig::from_env();
    let ticket_hash = hash_reset_value(
        &config.reset_code_secret,
        PASSWORD_RESET_PURPOSE,
        req.token.trim(),
    );
    match store.verify_reset_ticket_exists(&ticket_hash).await {
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
        Err(error) => {
            warn!(error = %error, "failed to verify reset ticket");
            handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to verify reset session",
            )
        }
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
    let email = match normalize_email(&req.email) {
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

    let config = PasswordResetConfig::from_env();

    if !password_reset_enabled(&config) {
        return handlers::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "password_reset_unavailable",
            "Password reset is not available in this environment",
        );
    }

    let user_row = match store.find_user_by_email_for_reset(&email).await {
        Ok(row) => row,
        Err(error) => {
            warn!(error = %error, "failed to resolve password reset user");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to initiate password reset",
            );
        }
    };

    let Some(user_row) = user_row else {
        return (
            StatusCode::ACCEPTED,
            Json(json!({
                "success": true,
                "data": null,
                "error": null,
            })),
        )
            .into_response();
    };

    let user_id = user_row.user_id;
    let org_id = user_row.org_id;
    let resolved_email = user_row.email;
    let code = generate_reset_code();
    let reset_ticket = generate_reset_ticket();
    let code_hash = hash_reset_value(
        &config.reset_code_secret,
        PASSWORD_RESET_PURPOSE,
        &format!("{resolved_email}:{code}"),
    );
    let ticket_hash =
        hash_reset_value(&config.reset_code_secret, PASSWORD_RESET_PURPOSE, &reset_ticket);
    let code_expires_at = chrono::Utc::now() + chrono::Duration::minutes(RESET_CODE_TTL_MINUTES);
    let ticket_expires_at =
        chrono::Utc::now() + chrono::Duration::minutes(RESET_TICKET_TTL_MINUTES);

    if let Err(error) = store
        .create_password_reset_ticket(&CreatePasswordResetTicketInput {
            org_id,
            user_id,
            email: resolved_email.clone(),
            purpose: PASSWORD_RESET_PURPOSE.to_string(),
            ticket_hash,
            code_hash,
            expires_at: ticket_expires_at,
            code_expires_at,
        })
        .await
    {
        warn!(error = %error, "failed to persist password reset ticket");
        return handlers::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Failed to initiate password reset",
        );
    }

    if config.smtp_ready()
        && let Err(error) = send_reset_email(&config, &resolved_email, &code, code_expires_at).await
    {
        warn!(error = %error, "failed to send reset code email");
        return handlers::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Failed to send reset code",
        );
    }

    #[allow(unused_mut)]
    let mut response = json!({
        "success": true,
        "data": null,
        "error": null,
    });
    #[cfg(test)]
    {
        response["debug_code"] = json!(code);
        response["debug_reset_ticket"] = json!(reset_ticket);
    }
    record_api_product_event_if_available(
        &state,
        user_id,
        analytics::ProductEventName::PasswordResetRequested,
        analytics::ResultTag::Success,
        serde_json::json!({
            "email_domain": resolved_email.split('@').nth(1).unwrap_or_default(),
            "delivery": if config.smtp_ready() { "smtp" } else { "debug" },
        }),
    )
    .await;
    (StatusCode::ACCEPTED, Json(response)).into_response()
}

pub(crate) async fn auth_verify_reset_code_handler(
    State(state): State<AppState>,
    Json(req): Json<VerifyResetCodeRequest>,
) -> Response {
    let email = match normalize_email(&req.email) {
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
    let config = PasswordResetConfig::from_env();
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

    let reset_ticket = generate_reset_ticket();
    let ticket_hash =
        hash_reset_value(&config.reset_code_secret, PASSWORD_RESET_PURPOSE, &reset_ticket);
    let verified = match store
        .verify_and_rotate_reset_code(
            &email,
            PASSWORD_RESET_PURPOSE,
            code,
            &config.reset_code_secret,
            &ticket_hash,
            RESET_MAX_ATTEMPTS,
        )
        .await
    {
        Ok(verified) => verified,
        Err(error) => {
            warn!(error = %error, "failed to verify reset code");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to verify reset code",
            );
        }
    };

    let Some((user_id, resolved_email)) = verified else {
        return (
            StatusCode::OK,
            Json(AuthEnvelope {
                success: false,
                data: None,
                error: Some("Reset code is invalid or expired".to_string()),
            }),
        )
            .into_response();
    };

    record_api_product_event_if_available(
        &state,
        user_id,
        analytics::ProductEventName::PasswordResetVerified,
        analytics::ResultTag::Success,
        serde_json::json!({
            "email_domain": resolved_email.split('@').nth(1).unwrap_or_default(),
        }),
    )
    .await;

    (
        StatusCode::OK,
        Json(AuthEnvelope {
            success: true,
            data: Some(AuthPayload {
                token: String::new(),
                user: password_reset_user(resolved_email, user_id),
                reset_ticket: Some(reset_ticket),
            }),
            error: None,
        }),
    )
        .into_response()
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
    let config = PasswordResetConfig::from_env();
    let ticket_hash = hash_reset_value(&config.reset_code_secret, PASSWORD_RESET_PURPOSE, ticket);
    let password_hash = match hash(&req.new_password, DEFAULT_COST) {
        Ok(value) => value,
        Err(error) => {
            warn!(error = %error, "password hashing failed");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to reset password",
            );
        }
    };

    let user_id = match store
        .reset_password_with_ticket_hash(&ticket_hash, PASSWORD_RESET_PURPOSE, &password_hash)
        .await
    {
        Ok(user_id) => user_id,
        Err(error) if error.http_status() == 400 => {
            return handlers::error_response(
                StatusCode::BAD_REQUEST,
                "invalid_reset_ticket",
                "Reset session is invalid or expired",
            );
        }
        Err(error) => {
            warn!(error = %error, "failed to reset password");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to reset password",
            );
        }
    };

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
