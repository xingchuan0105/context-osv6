use std::str::FromStr;

use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Address, AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use sha2::{Digest, Sha256};

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

async fn begin_auth_admin_tx<'a>(
    pool: &'a sqlx::PgPool,
) -> Result<sqlx::Transaction<'a, sqlx::Postgres>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("select set_config('app.current_role', 'super_admin', true)")
        .execute(tx.as_mut())
        .await?;
    Ok(tx)
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

async fn auth_runtime_capabilities_handler() -> Response {
    let config = PasswordResetConfig::from_env();
    (
        StatusCode::OK,
        Json(contracts::AuthRuntimeCapabilitiesResponse {
            password_reset_enabled: password_reset_enabled(&config),
        }),
    )
        .into_response()
}

async fn auth_logout_handler(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Response {
    let Some(user_id) = state.auth().actor_id() else {
        return handlers::error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Not authenticated",
        );
    };
    let Some(pg) = state.pg() else {
        return handlers::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };
    let pool = pg.raw();
    let mut tx = match begin_auth_admin_tx(pool).await {
        Ok(tx) => tx,
        Err(error) => {
            warn!(error = %error, "failed to start logout transaction");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Logout failed",
            );
        }
    };

    match sqlx::query(
        r#"
        update users
        set auth_version = auth_version + 1
        where id = $1
        "#,
    )
    .bind(user_id.into_uuid())
    .execute(tx.as_mut())
    .await
    {
        Ok(result) => {
            if result.rows_affected() == 0 {
                return handlers::error_response(
                    StatusCode::UNAUTHORIZED,
                    "unauthorized",
                    "Not authenticated",
                );
            }
            if let Err(error) = tx.commit().await {
                warn!(error = %error, "failed to commit logout transaction");
                return handlers::error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "Logout failed",
                );
            }
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

async fn auth_me_handler(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Response {
    let Some(user_id) = state.auth().actor_id() else {
        return handlers::error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Not authenticated",
        );
    };
    let user_uuid = user_id.into_uuid();

    let Some(pg) = state.pg() else {
        return handlers::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };
    let pool = pg.raw();
    let mut tx = match begin_auth_admin_tx(pool).await {
        Ok(tx) => tx,
        Err(error) => {
            warn!(error = %error, "failed to start auth me transaction");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to load profile",
            );
        }
    };

    let result = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>)>(
        "SELECT id, org_id, email, full_name FROM users WHERE id = $1",
    )
    .bind(user_uuid)
    .fetch_optional(tx.as_mut())
    .await;
    let _ = tx.commit().await;

    match result {
        Ok(Some((id, _org_id, email, full_name))) => (
            StatusCode::OK,
            Json(AuthEnvelope {
                success: true,
                data: Some(AuthPayload {
                    token: String::new(),
                    user: AuthUserDto {
                        id: id.to_string(),
                        email,
                        full_name: full_name.unwrap_or_default(),
                    },
                    reset_ticket: None,
                }),
                error: None,
            }),
        )
            .into_response(),
        _ => handlers::error_response(
            StatusCode::NOT_FOUND,
            "user_not_found",
            "User profile not found",
        ),
    }
}

async fn auth_update_profile_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(req): Json<UpdateProfileRequest>,
) -> Response {
    let Some(user_id) = state.auth().actor_id() else {
        return handlers::error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Not authenticated",
        );
    };
    let Some(pg) = state.pg() else {
        return handlers::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };

    let full_name = req.full_name.unwrap_or_default();
    let user_uuid = user_id.into_uuid();
    let pool = pg.raw();
    let mut tx = match begin_auth_admin_tx(pool).await {
        Ok(tx) => tx,
        Err(error) => {
            warn!(error = %error, "failed to start profile update transaction");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Profile update failed",
            );
        }
    };
    let result = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>)>(
        r#"
        update users
        set full_name = $2
        where id = $1
        returning id, org_id, email, full_name
    "#,
    )
    .bind(user_uuid)
    .bind(full_name)
    .fetch_optional(tx.as_mut())
    .await;
    if result.is_ok() {
        let _ = tx.commit().await;
    }
    match result {
        Ok(Some((id, _org_id, email, full_name))) => (
            StatusCode::OK,
            Json(AuthEnvelope {
                success: true,
                data: Some(AuthPayload {
                    token: String::new(),
                    user: AuthUserDto {
                        id: id.to_string(),
                        email,
                        full_name: full_name.unwrap_or_default(),
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

async fn auth_get_preferences_handler(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Response {
    if state.auth().actor_id().is_none() {
        return handlers::error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Not authenticated",
        );
    }

    match state.current_user_preferences().await {
        Ok(preferences) => {
            let payload = serde_json::to_value(preferences)
                .ok()
                .and_then(|value| serde_json::from_value::<UserPreferencesPayload>(value).ok())
                .unwrap_or_default();
            (StatusCode::OK, Json(payload)).into_response()
        }
        Err(error) => {
            warn!(error = %error, "failed to load user preferences");
            handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to load preferences",
            )
        }
    }
}

async fn auth_update_preferences_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(req): Json<UserPreferencesPayload>,
) -> Response {
    if state.auth().actor_id().is_none() {
        return handlers::error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Not authenticated",
        );
    }

    let previous_preferences = match state.current_user_preferences().await {
        Ok(preferences) => serde_json::to_value(preferences)
            .ok()
            .and_then(|value| serde_json::from_value::<UserPreferencesPayload>(value).ok())
            .unwrap_or_default(),
        Err(error) => {
            warn!(error = %error, "failed to load existing preferences");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to save preferences",
            );
        }
    };

    let next_preferences = match serde_json::to_value(&req)
        .ok()
        .and_then(|value| serde_json::from_value::<common::UserPreferences>(value).ok())
    {
        Some(preferences) => preferences,
        None => {
            return handlers::error_response(
                StatusCode::BAD_REQUEST,
                "validation_error",
                "Invalid preferences payload",
            );
        }
    };

    if let Err(error) = state.save_current_user_preferences(&next_preferences).await {
        warn!(error = %error, "failed to persist user preferences");
        return handlers::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Failed to save preferences",
        );
    }

    for (notebook_id, notes) in changed_workspace_drafts(&previous_preferences, &req) {
        let metadata = serde_json::json!({
            "notes_length": notes.chars().count(),
            "synced": true,
        });
        state
            .record_product_event_if_available(
                analytics::ProductEventName::NoteEdited,
                analytics::Surface::Workspace,
                analytics::ResultTag::Success,
                None,
                uuid::Uuid::parse_str(&notebook_id).ok(),
                metadata.clone(),
            )
            .await;
        state
            .record_product_event_if_available(
                analytics::ProductEventName::NoteSynced,
                analytics::Surface::Workspace,
                analytics::ResultTag::Success,
                None,
                uuid::Uuid::parse_str(&notebook_id).ok(),
                metadata,
            )
            .await;
    }

    (StatusCode::OK, Json(req)).into_response()
}

fn changed_workspace_drafts(
    previous: &UserPreferencesPayload,
    next: &UserPreferencesPayload,
) -> Vec<(String, String)> {
    let previous_map = previous
        .dashboard
        .workspace_drafts
        .iter()
        .map(|draft| (draft.notebook_id.clone(), draft.notes.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let next_map = next
        .dashboard
        .workspace_drafts
        .iter()
        .map(|draft| (draft.notebook_id.clone(), draft.notes.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();

    let mut notebook_ids = previous_map.keys().cloned().collect::<std::collections::BTreeSet<_>>();
    notebook_ids.extend(next_map.keys().cloned());

    notebook_ids
        .into_iter()
        .filter_map(|notebook_id| {
            let before = previous_map.get(&notebook_id).cloned().unwrap_or_default();
            let after = next_map.get(&notebook_id).cloned().unwrap_or_default();
            (before != after).then_some((notebook_id, after))
        })
        .collect()
}

async fn auth_change_password_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    Json(req): Json<ChangePasswordRequest>,
) -> Response {
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
    let Some(pg) = state.pg() else {
        return handlers::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };

    let user_uuid = user_id.into_uuid();
    let pool = pg.raw();
    let mut tx = match begin_auth_admin_tx(pool).await {
        Ok(tx) => tx,
        Err(error) => {
            warn!(error = %error, "failed to start password change transaction");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Password update failed",
            );
        }
    };
    let row = match sqlx::query_as::<_, (String, )>(
        "SELECT password_hash FROM users WHERE id = $1",
    )
    .bind(user_uuid)
    .fetch_optional(tx.as_mut())
    .await
    {
        Ok(Some(row)) => row,
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

    match verify(&req.old_password, &row.0) {
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

    match sqlx::query(
        r#"
        update users
        set password_hash = $2,
            password_updated_at = now(),
            auth_version = auth_version + 1
        where id = $1
        "#,
    )
    .bind(user_uuid)
    .bind(new_hash)
    .execute(tx.as_mut())
    .await
    {
        Ok(_) => {
            if let Err(error) = tx.commit().await {
                warn!(error = %error, "failed to commit password change transaction");
                return handlers::error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "Password update failed",
                );
            }
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

async fn auth_reset_request_handler(
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

async fn auth_verify_reset_token_handler(
    State(state): State<AppState>,
    Json(req): Json<VerifyResetTokenRequest>,
) -> Response {
    let Some(pg) = state.pg() else {
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
    let mut tx = match begin_auth_admin_tx(pg.raw()).await {
        Ok(tx) => tx,
        Err(error) => {
            warn!(error = %error, "failed to start reset token verification transaction");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to verify reset session",
            );
        }
    };
    let result = sqlx::query(
        r#"
        select 1
        from password_reset_tickets
        where ticket_hash = $1
          and used_at is null
          and expires_at > now()
        limit 1
        "#,
    )
    .bind(ticket_hash)
    .fetch_optional(tx.as_mut())
    .await;
    let _ = tx.commit().await;
    match result {
        Ok(Some(_)) => (
            StatusCode::OK,
            Json(AuthEnvelope {
                success: true,
                data: None,
                error: None,
            }),
        )
            .into_response(),
        Ok(None) => (
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

async fn auth_reset_password_handler(
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

async fn auth_send_reset_code_handler(
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
    let Some(pg) = state.pg() else {
        return handlers::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };
    let pool = pg.raw();
    let config = PasswordResetConfig::from_env();

    if !password_reset_enabled(&config) {
        return handlers::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "password_reset_unavailable",
            "Password reset is not available in this environment",
        );
    }

    let mut tx = match begin_auth_admin_tx(pool).await {
        Ok(tx) => tx,
        Err(error) => {
            warn!(error = %error, "failed to start password reset request transaction");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to initiate password reset",
            );
        }
    };

    let user_row = match sqlx::query_as::<_, (Uuid, Uuid, String)>(
        r#"
        select id, org_id, email
        from users
        where lower(email) = lower($1)
        order by created_at desc
        limit 1
        "#,
    )
    .bind(&email)
    .fetch_optional(tx.as_mut())
    .await
    {
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

    let Some((user_id, org_id, resolved_email)) = user_row else {
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

    if let Err(error) = sqlx::query(
        r#"
        insert into password_reset_tickets (
            org_id, user_id, email, purpose, ticket_hash, code_hash,
            expires_at, code_expires_at, attempts, used_at, created_at, updated_at
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, 0, null, now(), now())
        "#,
    )
    .bind(org_id)
    .bind(user_id)
    .bind(&resolved_email)
    .bind(PASSWORD_RESET_PURPOSE)
    .bind(ticket_hash)
    .bind(code_hash)
    .bind(ticket_expires_at)
    .bind(code_expires_at)
    .execute(tx.as_mut())
    .await
    {
        warn!(error = %error, "failed to persist password reset ticket");
        return handlers::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Failed to initiate password reset",
        );
    }

    if let Err(error) = tx.commit().await {
        warn!(error = %error, "failed to commit password reset request");
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

async fn auth_verify_reset_code_handler(
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
    let Some(pg) = state.pg() else {
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

    let mut tx = match begin_auth_admin_tx(pg.raw()).await {
        Ok(tx) => tx,
        Err(error) => {
            warn!(error = %error, "failed to start password reset verification transaction");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to verify reset code",
            );
        }
    };

    let row = match sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>, i32, Option<chrono::DateTime<chrono::Utc>>)>(
        r#"
        select id, user_id, email, code_hash, attempts, code_expires_at
        from password_reset_tickets
        where lower(email) = lower($1)
          and purpose = $2
          and used_at is null
        order by created_at desc
        limit 1
        for update
        "#,
    )
    .bind(&email)
    .bind(PASSWORD_RESET_PURPOSE)
    .fetch_optional(tx.as_mut())
    .await
    {
        Ok(row) => row,
        Err(error) => {
            warn!(error = %error, "failed to load password reset ticket");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to verify reset code",
            );
        }
    };

    let Some((ticket_id, user_id, resolved_email, code_hash, attempts, code_expires_at)) = row else {
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

    if attempts >= RESET_MAX_ATTEMPTS {
        return (
            StatusCode::OK,
            Json(AuthEnvelope {
                success: false,
                data: None,
                error: Some("Reset code is invalid or expired".to_string()),
            }),
        )
            .into_response();
    }

    if code_expires_at.map(|value| value < chrono::Utc::now()).unwrap_or(true) {
        return (
            StatusCode::OK,
            Json(AuthEnvelope {
                success: false,
                data: None,
                error: Some("Reset code is invalid or expired".to_string()),
            }),
        )
            .into_response();
    }

    let expected = hash_reset_value(
        &config.reset_code_secret,
        PASSWORD_RESET_PURPOSE,
        &format!("{resolved_email}:{code}"),
    );
    if code_hash.as_deref() != Some(expected.as_str()) {
        let _ = sqlx::query(
            "update password_reset_tickets set attempts = attempts + 1, updated_at = now() where id = $1",
        )
        .bind(ticket_id)
        .execute(tx.as_mut())
        .await;
        let _ = tx.commit().await;
        return (
            StatusCode::OK,
            Json(AuthEnvelope {
                success: false,
                data: None,
                error: Some("Reset code is invalid or expired".to_string()),
            }),
        )
            .into_response();
    }

    let reset_ticket = generate_reset_ticket();
    let ticket_hash =
        hash_reset_value(&config.reset_code_secret, PASSWORD_RESET_PURPOSE, &reset_ticket);
    if let Err(error) = sqlx::query(
        r#"
        update password_reset_tickets
        set ticket_hash = $2,
            updated_at = now()
        where id = $1
        "#,
    )
    .bind(ticket_id)
    .bind(ticket_hash)
    .execute(tx.as_mut())
    .await
    {
        warn!(error = %error, "failed to persist reset ticket");
        return handlers::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Failed to verify reset code",
        );
    }

    if let Err(error) = tx.commit().await {
        warn!(error = %error, "failed to commit reset verification");
        return handlers::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Failed to verify reset code",
        );
    }
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

async fn auth_confirm_reset_password_handler(
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
    let Some(pg) = state.pg() else {
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

    let mut tx = match begin_auth_admin_tx(pg.raw()).await {
        Ok(tx) => tx,
        Err(error) => {
            warn!(error = %error, "failed to start password reset confirmation transaction");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to reset password",
            );
        }
    };

    let row = match sqlx::query_as::<_, (Uuid, Uuid)>(
        r#"
        select id, user_id
        from password_reset_tickets
        where ticket_hash = $1
          and purpose = $2
          and used_at is null
          and expires_at > now()
        limit 1
        for update
        "#,
    )
    .bind(ticket_hash)
    .bind(PASSWORD_RESET_PURPOSE)
    .fetch_optional(tx.as_mut())
    .await
    {
        Ok(row) => row,
        Err(error) => {
            warn!(error = %error, "failed to load reset ticket");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to reset password",
            );
        }
    };

    let Some((ticket_id, user_id)) = row else {
        return handlers::error_response(
            StatusCode::BAD_REQUEST,
            "invalid_reset_ticket",
            "Reset session is invalid or expired",
        );
    };

    if let Err(error) = sqlx::query(
        r#"
        update users
        set password_hash = $2,
            password_updated_at = now(),
            auth_version = auth_version + 1
        where id = $1
        "#,
    )
    .bind(user_id)
    .bind(password_hash)
    .execute(tx.as_mut())
    .await
    {
        warn!(error = %error, "failed to update password from reset");
        return handlers::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Failed to reset password",
        );
    }

    if let Err(error) = sqlx::query(
        "update password_reset_tickets set used_at = now(), updated_at = now() where id = $1",
    )
    .bind(ticket_id)
    .execute(tx.as_mut())
    .await
    {
        warn!(error = %error, "failed to consume reset ticket");
        return handlers::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Failed to reset password",
        );
    }

    if let Err(error) = tx.commit().await {
        warn!(error = %error, "failed to commit password reset");
        return handlers::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Failed to reset password",
        );
    }
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

// ---------------------------------------------------------------------------
// Usage limit handler
// ---------------------------------------------------------------------------

async fn usage_limit_handler(
    Extension(RequestState(state)): Extension<RequestState>,
) -> Response {
    match state.get_user_usage_limit().await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "internal_error", "message": "Usage limit service unavailable"})),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Health / Ready / Infra
// ---------------------------------------------------------------------------
