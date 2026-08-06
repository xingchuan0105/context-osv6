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

use app_core::{AuthUserProfile, ProfileMediaKind, UpdateUserProfileInput};

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

pub(crate) fn auth_user_dto_from_profile(profile: &AuthUserProfile) -> AuthUserDto {
    let user_id = profile.user_id;
    AuthUserDto {
        id: user_id.to_string(),
        email: profile.email.clone(),
        full_name: profile.full_name.clone().unwrap_or_default(),
        bio: profile
            .bio
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        contact_url: profile
            .contact_url
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        avatar_url: profile
            .avatar_object_path
            .as_ref()
            .filter(|s| !s.trim().is_empty())
            .map(|_| format!("/api/public/users/{user_id}/media/avatar")),
        banner_url: profile
            .banner_object_path
            .as_ref()
            .filter(|s| !s.trim().is_empty())
            .map(|_| format!("/api/public/users/{user_id}/media/banner")),
    }
}

pub(crate) fn empty_auth_user(id: String, email: String, full_name: String) -> AuthUserDto {
    AuthUserDto {
        id,
        email,
        full_name,
        bio: None,
        contact_url: None,
        avatar_url: None,
        banner_url: None,
    }
}

fn normalize_optional_text(value: Option<String>, max_len: usize) -> Result<Option<String>, String> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > max_len {
        return Err(format!("field exceeds max length of {max_len}"));
    }
    Ok(Some(trimmed.to_string()))
}

fn is_safe_http_url(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://"))
        && !value.chars().any(|c| c.is_control() || c.is_whitespace())
}

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
                    user: auth_user_dto_from_profile(&profile),
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

    let full_name = req.full_name.unwrap_or_default().trim().to_string();
    if full_name.chars().count() > 120 {
        return handlers::error_response(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "full_name exceeds max length of 120",
        );
    }
    let bio = match normalize_optional_text(req.bio, 500) {
        Ok(value) => value,
        Err(message) => {
            return handlers::error_response(StatusCode::BAD_REQUEST, "validation_error", &message);
        }
    };
    let contact_url = match normalize_optional_text(req.contact_url, 500) {
        Ok(value) => value,
        Err(message) => {
            return handlers::error_response(StatusCode::BAD_REQUEST, "validation_error", &message);
        }
    };
    if let Some(ref url) = contact_url {
        if !is_safe_http_url(url) {
            return handlers::error_response(
                StatusCode::BAD_REQUEST,
                "validation_error",
                "contact_url must be http(s)",
            );
        }
    }
    let user_uuid = user_id.into_uuid();
    let input = UpdateUserProfileInput {
        full_name,
        bio,
        contact_url,
    };

    match store.update_user_profile(user_uuid, &input).await {
        Ok(Some(profile)) => (
            StatusCode::OK,
            Json(AuthEnvelope {
                success: true,
                data: Some(AuthPayload {
                    token: String::new(),
                    user: auth_user_dto_from_profile(&profile),
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
        Ok(()) => {
            crate::notification_emit::emit_user_notification(
                &state,
                state.auth(),
                user_uuid,
                "security.password_changed",
                "Password changed",
                "Your account password was updated successfully.",
                serde_json::json!({ "user_id": user_uuid.to_string() }),
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

const MAX_AVATAR_BYTES: usize = 2 * 1024 * 1024;
const MAX_BANNER_BYTES: usize = 5 * 1024 * 1024;

fn media_content_type_ok(content_type: &str) -> Option<&'static str> {
    match content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        _ => None,
    }
}

/// PUT /api/auth/profile/media/{kind} — raw image body to object store.
pub(crate) async fn auth_upload_profile_media_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    axum::extract::Path(kind): axum::extract::Path<String>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if let Err(error) = crate::auth_guard::forbid_api_key(
        state.auth(),
        "profile media updates require a signed-in user session",
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
    let Some(kind) = ProfileMediaKind::parse(&kind) else {
        return handlers::error_response(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "kind must be avatar or banner",
        );
    };
    let max_bytes = match kind {
        ProfileMediaKind::Avatar => MAX_AVATAR_BYTES,
        ProfileMediaKind::Banner => MAX_BANNER_BYTES,
    };
    if body.is_empty() {
        return handlers::error_response(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "empty body",
        );
    }
    if body.len() > max_bytes {
        return handlers::error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            "validation_error",
            "image too large",
        );
    }
    let content_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let Some(ext) = media_content_type_ok(content_type) else {
        return handlers::error_response(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "content-type must be image/jpeg, image/png, image/webp, or image/gif",
        );
    };
    let user_uuid = user_id.into_uuid();
    let object_path = format!(
        "user-profile/{}/{}.{}",
        user_uuid,
        kind.as_str(),
        ext
    );
    if let Err(error) = state
        .storage()
        .objects()
        .object_store
        .put(&object_path, body.as_ref())
        .await
    {
        warn!(error = %error, "failed to put profile media");
        return handlers::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Failed to store image",
        );
    }

    match store
        .set_user_profile_media_path(user_uuid, kind, Some(&object_path))
        .await
    {
        Ok(Some(profile)) => (
            StatusCode::OK,
            Json(AuthEnvelope {
                success: true,
                data: Some(AuthPayload {
                    token: String::new(),
                    user: auth_user_dto_from_profile(&profile),
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
            warn!(error = %error, "failed to update profile media path");
            handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Profile media update failed",
            )
        }
    }
}

/// DELETE /api/auth/profile/media/{kind}
pub(crate) async fn auth_delete_profile_media_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    axum::extract::Path(kind): axum::extract::Path<String>,
) -> Response {
    if let Err(error) = crate::auth_guard::forbid_api_key(
        state.auth(),
        "profile media updates require a signed-in user session",
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
    let Some(kind) = ProfileMediaKind::parse(&kind) else {
        return handlers::error_response(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "kind must be avatar or banner",
        );
    };
    let user_uuid = user_id.into_uuid();
    // Best-effort remove object before clearing path.
    if let Ok(Some(existing)) = store.get_user_profile(user_uuid).await {
        let path = match kind {
            ProfileMediaKind::Avatar => existing.avatar_object_path,
            ProfileMediaKind::Banner => existing.banner_object_path,
        };
        if let Some(path) = path.filter(|p| !p.trim().is_empty()) {
            if let Err(error) = state
                .storage()
                .objects()
                .object_store
                .delete(&path)
                .await
            {
                warn!(error = %error, %path, "failed to delete profile media object");
            }
        }
    }
    match store
        .set_user_profile_media_path(user_uuid, kind, None)
        .await
    {
        Ok(Some(profile)) => (
            StatusCode::OK,
            Json(AuthEnvelope {
                success: true,
                data: Some(AuthPayload {
                    token: String::new(),
                    user: auth_user_dto_from_profile(&profile),
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
            warn!(error = %error, "failed to clear profile media path");
            handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Profile media clear failed",
            )
        }
    }
}

/// GET /api/public/users/{user_id}/media/{kind} — public avatar/banner for share cards.
pub(crate) async fn public_user_media_handler(
    axum::extract::State(state): axum::extract::State<app_bootstrap::AppState>,
    axum::extract::Path((user_id, kind)): axum::extract::Path<(String, String)>,
) -> Response {
    let Some(store) = state.auth_store() else {
        return handlers::error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "Database not available",
        );
    };
    let Ok(user_uuid) = uuid::Uuid::parse_str(&user_id) else {
        return handlers::error_response(StatusCode::BAD_REQUEST, "validation_error", "invalid user id");
    };
    let Some(kind) = ProfileMediaKind::parse(&kind) else {
        return handlers::error_response(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "kind must be avatar or banner",
        );
    };
    let profile = match store.get_user_profile(user_uuid).await {
        Ok(Some(profile)) => profile,
        Ok(None) => {
            return handlers::error_response(StatusCode::NOT_FOUND, "not_found", "Not found");
        }
        Err(error) => {
            warn!(error = %error, "failed to load profile for public media");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Failed to load media",
            );
        }
    };
    let object_path = match kind {
        ProfileMediaKind::Avatar => profile.avatar_object_path,
        ProfileMediaKind::Banner => profile.banner_object_path,
    };
    let Some(object_path) = object_path.filter(|p| !p.trim().is_empty()) else {
        return handlers::error_response(StatusCode::NOT_FOUND, "not_found", "Not found");
    };
    let bytes = match state
        .storage()
        .objects()
        .object_store
        .get(&object_path)
        .await
    {
        Ok(bytes) => bytes,
        Err(error) => {
            warn!(error = %error, path = %object_path, "failed to read profile media");
            return handlers::error_response(StatusCode::NOT_FOUND, "not_found", "Not found");
        }
    };
    let content_type = if object_path.ends_with(".png") {
        "image/png"
    } else if object_path.ends_with(".webp") {
        "image/webp"
    } else if object_path.ends_with(".gif") {
        "image/gif"
    } else {
        "image/jpeg"
    };
    (
        StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, content_type),
            (
                axum::http::header::CACHE_CONTROL,
                "public, max-age=3600",
            ),
        ],
        bytes,
    )
        .into_response()
}
