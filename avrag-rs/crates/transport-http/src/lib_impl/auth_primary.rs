async fn auth_register_handler(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Response {
    if req.email.trim().is_empty() || req.password.len() < 8 {
        return handlers::error_response(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "Email and password (min 8 chars) are required",
        );
    }

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

    let token = issue_jwt_for_auth_version(&result.user_id, &result.org_id, result.auth_version);
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
                user: AuthUserDto {
                    id: result.user_id.to_string(),
                    email: result.email,
                    full_name: result.full_name,
                },
                reset_ticket: None,
            }),
            error: None,
        }),
    )
        .into_response()
}

async fn auth_login_handler(
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
        &credentials.org_id,
        credentials.auth_version,
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

    (
        StatusCode::OK,
        Json(AuthEnvelope {
            success: true,
            data: Some(AuthPayload {
                token,
                user: AuthUserDto {
                    id: credentials.user_id.to_string(),
                    email: credentials.email,
                    full_name: credentials.full_name.unwrap_or_default(),
                },
                reset_ticket: None,
            }),
            error: None,
        }),
    )
        .into_response()
}
