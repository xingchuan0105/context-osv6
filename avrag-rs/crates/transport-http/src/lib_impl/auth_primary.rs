use app_bootstrap::AppState;
use app_core::RegisterUserInput;
use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use bcrypt::DEFAULT_COST;
use bcrypt::hash;
use bcrypt::verify;
use tracing::warn;

use crate::auth_types::AgentTokenEnvelope;
use crate::auth_types::AgentTokenPayload;
use crate::auth_types::AgentTokenRequest;
use crate::auth_types::AuthEnvelope;
use crate::auth_types::AuthPayload;

use crate::auth_types::LoginRequest;
use crate::auth_types::RegisterRequest;
use crate::handlers;
use crate::middleware::RequestState;
use axum::extract::Extension;

use super::router_core::{
    AgentMintError, extract_bearer, issue_jwt_for_auth_version,
    record_api_product_event_if_available, reissue_agent_jwt_with_ttl, verify_jwt,
};

pub(crate) async fn auth_register_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RegisterRequest>,
) -> Response {
    if req.email.trim().is_empty() || req.password.len() < 8 {
        return handlers::error_response(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "Email and password (min 8 chars) are required",
        );
    }

    // 校验法律协议同意（P0-CON-1: 未勾选无法注册）
    let terms_version = match req.terms_version {
        Some(v) if !v.trim().is_empty() => v,
        _ => {
            return handlers::error_response(
                StatusCode::BAD_REQUEST,
                "consent_required",
                "Please agree to the Terms of Service and Privacy Policy",
            );
        }
    };
    let privacy_version = match req.privacy_version {
        Some(v) if !v.trim().is_empty() => v,
        _ => {
            return handlers::error_response(
                StatusCode::BAD_REQUEST,
                "consent_required",
                "Please agree to the Terms of Service and Privacy Policy",
            );
        }
    };

    if let Err(error) =
        app_core::validate_published_legal_versions(&terms_version, &privacy_version)
    {
        return handlers::error_response(
            StatusCode::BAD_REQUEST,
            error.code(),
            error.message(),
        );
    }

    let ip_address = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string());
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let Some(store) = state.auth_store() else {
        return handlers::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };

    let password_hash = match hash(&req.password, DEFAULT_COST) {
        Ok(h) => h,
        Err(e) => {
            warn!(error = %e, "password hashing failed");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Registration failed",
            );
        }
    };

    let result = match store
        .register_user(&RegisterUserInput {
            email: req.email.trim().to_string(),
            password_hash,
            full_name: req.full_name.clone(),
            legal_acceptance: app_core::RegisterLegalAcceptance {
                terms_version,
                privacy_version,
                context: "register".to_string(),
                ip_address,
                user_agent,
            },
        })
        .await
    {
        Ok(result) => result,
        Err(error) if error.http_status() == 409 => {
            return handlers::error_response(
                StatusCode::CONFLICT,
                "email_exists",
                "An account with this email already exists",
            );
        }
        Err(error) => {
            warn!(error = %error, "registration failed");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Registration failed",
            );
        }
    };

    // ADR-0010 PR3/PR4: signup ¥20 grant, then optional bilateral referral ¥5.
    // Failures must not block registration (ledger + invitee unique are idempotent).
    if let Some(repo) = state.postgres_repo() {
        let wallet_store: std::sync::Arc<dyn app_core::WalletStorePort> =
            std::sync::Arc::new(app_bootstrap::PgWalletStoreAdapter::new(repo.clone()));
        match avrag_billing::grant_signup_bonus(wallet_store.clone(), result.user_id).await {
            Ok(grant) => {
                if grant.applied {
                    tracing::info!(
                        user_id = %result.user_id,
                        balance_fen = grant.wallet.balance_fen,
                        "signup wallet grant applied"
                    );
                }
            }
            Err(error) => {
                warn!(
                    error = %error,
                    user_id = %result.user_id,
                    "signup wallet grant failed (idempotent retry possible)"
                );
            }
        }

        if let Some(ref code) = req.referral_code {
            let referral_store: std::sync::Arc<dyn app_core::ReferralStorePort> =
                std::sync::Arc::new(app_bootstrap::PgReferralStoreAdapter::new(repo));
            match avrag_billing::apply_referral_on_register(
                wallet_store,
                referral_store,
                result.user_id,
                Some(code.as_str()),
            )
            .await
            {
                Ok(avrag_billing::ApplyReferralOutcome::Rewarded { referral, .. }) => {
                    tracing::info!(
                        user_id = %result.user_id,
                        inviter_id = %referral.inviter_id,
                        referral_id = %referral.id,
                        "referral bilateral grant applied"
                    );
                }
                Ok(avrag_billing::ApplyReferralOutcome::RecordedRejected { referral }) => {
                    tracing::info!(
                        user_id = %result.user_id,
                        reason = ?referral.reject_reason,
                        "referral rejected (no grant)"
                    );
                }
                Ok(avrag_billing::ApplyReferralOutcome::Rejected { reason }) => {
                    tracing::info!(
                        user_id = %result.user_id,
                        reason,
                        "referral code not applied"
                    );
                }
                Ok(_) => {}
                Err(error) => {
                    warn!(
                        error = %error,
                        user_id = %result.user_id,
                        "referral apply failed (idempotent retry possible)"
                    );
                }
            }
        }
    }

    let token = issue_jwt_for_auth_version(
        &result.user_id,
        &result.owner_user_id,
        result.auth_version,
        &result.role,
    );
    record_api_product_event_if_available(
        &state,
        result.user_id,
        analytics::ProductEventName::UserRegistered,
        analytics::ResultTag::Success,
        serde_json::json!({
            "email_domain": result.email.split('@').nth(1).unwrap_or_default(),
        }),
    )
    .await;

    (
        StatusCode::CREATED,
        Json(AuthEnvelope {
            success: true,
            data: Some(AuthPayload {
                token,
                user: super::auth::profile::empty_auth_user(
                    result.user_id.to_string(),
                    result.email,
                    result.full_name,
                ),
                reset_ticket: None,
            }),
            error: None,
        }),
    )
        .into_response()
}

pub(crate) async fn auth_login_handler(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Response {
    let Some(store) = state.auth_store() else {
        return handlers::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };

    let credentials = match store.find_user_for_login(req.email.trim()).await {
        Ok(credentials) => credentials,
        Err(error) => {
            warn!(error = %error, "DB error fetching user");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Login failed",
            );
        }
    };

    let Some(credentials) = credentials else {
        return handlers::error_response(
            StatusCode::UNAUTHORIZED,
            "account_not_registered",
            "This account is not registered",
        );
    };

    let stored_hash = match credentials.password_hash {
        Some(h) => h,
        None => {
            return handlers::error_response(
                StatusCode::UNAUTHORIZED,
                "invalid_password",
                "Incorrect password",
            );
        }
    };

    match verify(&req.password, &stored_hash) {
        Ok(true) => {}
        _ => {
            return handlers::error_response(
                StatusCode::UNAUTHORIZED,
                "invalid_password",
                "Incorrect password",
            );
        }
    }

    let token = issue_jwt_for_auth_version(
        &credentials.user_id,
        &credentials.owner_user_id,
        credentials.auth_version,
        &credentials.role,
    );
    record_api_product_event_if_available(
        &state,
        credentials.user_id,
        analytics::ProductEventName::UserLoggedIn,
        analytics::ResultTag::Success,
        serde_json::json!({
            "email_domain": credentials.email.split('@').nth(1).unwrap_or_default(),
        }),
    )
    .await;

    let user = match store.get_user_profile(credentials.user_id).await {
        Ok(Some(profile)) => super::auth::profile::auth_user_dto_from_profile(&profile),
        Ok(None) | Err(_) => super::auth::profile::empty_auth_user(
            credentials.user_id.to_string(),
            credentials.email,
            credentials.full_name.unwrap_or_default(),
        ),
    };

    (
        StatusCode::OK,
        Json(AuthEnvelope {
            success: true,
            data: Some(AuthPayload {
                token,
                user,
                reset_ticket: None,
            }),
            error: None,
        }),
    )
        .into_response()
}

/// POST /api/auth/agent-token — mint a short-lived user JWT for MCP/CLI agents.
///
/// Requires a signed-in **user** session (workspace API keys → `api_key_forbidden`).
/// Revocation: password change bumps `auth_version`; short TTL limits leak window.
pub(crate) async fn auth_agent_token_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    headers: HeaderMap,
    Json(req): Json<AgentTokenRequest>,
) -> Response {
    if let Err(error) = crate::auth_guard::forbid_api_key(
        state.auth(),
        "agent tokens require a signed-in user session, not a workspace API key",
    ) {
        return handlers::app_error_response(error);
    }

    let Some(bearer) = extract_bearer(&headers) else {
        return handlers::error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Bearer user JWT required",
        );
    };
    let Some(claims) = verify_jwt(bearer) else {
        return handlers::error_response(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Invalid or expired user JWT",
        );
    };

    let requested_minutes = req.ttl_minutes.unwrap_or(120).clamp(5, 24 * 60);
    let requested_ttl = chrono::Duration::minutes(i64::from(requested_minutes));
    let (token, effective_ttl) = match reissue_agent_jwt_with_ttl(&claims, requested_ttl) {
        Ok(v) => v,
        Err(AgentMintError::AgentCannotRemint) => {
            return handlers::error_response(
                StatusCode::FORBIDDEN,
                "agent_token_cannot_remint",
                "Agent tokens cannot mint further agent tokens. Use a full login session JWT \
(or `context-os auth login` / desktop session), then call agent-token again.",
            );
        }
        Err(AgentMintError::ParentExpired) => {
            return handlers::error_response(
                StatusCode::UNAUTHORIZED,
                "token_expired",
                "Session JWT has no remaining lifetime to mint an agent token",
            );
        }
    };
    let ttl_minutes = effective_ttl.num_minutes().max(1) as u32;
    let expires_at = (chrono::Utc::now() + effective_ttl).to_rfc3339();

    (
        StatusCode::OK,
        Json(AgentTokenEnvelope {
            success: true,
            data: Some(AgentTokenPayload {
                token,
                expires_at,
                ttl_minutes,
                token_kind: super::router_core::TOKEN_KIND_AGENT.to_string(),
            }),
            error: None,
            message: Some(
                "Short-lived agent JWT (token_kind=agent). Export as CONTEXT_OS_USER_TOKEN. \
TTL is capped by the parent session expiry; agent tokens cannot re-mint. \
Workspace API keys remain for index/query automation."
                    .to_string(),
            ),
        }),
    )
        .into_response()
}
