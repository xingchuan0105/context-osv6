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
async fn auth_record_legal_acceptance_handler(
    Extension(RequestState(state)): Extension<RequestState>,
    headers: HeaderMap,
    Json(req): Json<RecordLegalAcceptanceRequest>,
) -> Response {
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
