async fn auth_register_handler(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Response {
    // Input validation
    if req.email.trim().is_empty() || req.password.len() < 8 {
        return handlers::error_response(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "Email and password (min 8 chars) are required",
        );
    }

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
            warn!(error = %error, "failed to start registration transaction");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Registration failed",
            );
        }
    };

    // Check email uniqueness
    match sqlx::query("SELECT id FROM users WHERE email = $1")
        .bind(req.email.trim())
        .fetch_optional(tx.as_mut())
        .await
    {
        Ok(Some(_)) => {
            return handlers::error_response(
                StatusCode::CONFLICT,
                "email_exists",
                "An account with this email already exists",
            );
        }
        Err(e) => {
            warn!(error = %e, "DB error checking email");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Registration failed",
            );
        }
        _ => {}
    }

    // Hash password
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

    let org_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    // Create org
    if let Err(e) =
        sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING")
            .bind(org_id)
            .bind(format!(
                "org-{}",
                req.email.split('@').next().unwrap_or("user")
            ))
            .execute(tx.as_mut())
            .await
    {
        warn!(error = %e, "failed to create org");
        return handlers::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Registration failed",
        );
    }

    // Create user
    if let Err(e) = sqlx::query(
        "INSERT INTO users (id, org_id, email, full_name, password_hash, role) VALUES ($1, $2, $3, $4, $5, 'user')",
    )
    .bind(user_id)
    .bind(org_id)
    .bind(req.email.trim())
    .bind(req.full_name.as_deref().unwrap_or_default())
    .bind(&password_hash)
    .execute(tx.as_mut())
    .await
    {
        warn!(error = %e, "failed to create user");
        return handlers::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Registration failed",
        );
    }
    if let Err(error) = tx.commit().await {
        warn!(error = %error, "failed to commit registration transaction");
        return handlers::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Registration failed",
        );
    }

    let token = issue_jwt(&user_id, &org_id);
    let full_name = req.full_name.unwrap_or_default();
    record_api_product_event_if_available(
        &state,
        user_id,
        analytics::ProductEventName::UserRegistered,
        analytics::ResultTag::Success,
        serde_json::json!({
            "email_domain": req.email.split('@').nth(1).unwrap_or_default(),
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
                    id: user_id.to_string(),
                    email: req.email.trim().to_string(),
                    full_name,
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
            warn!(error = %error, "failed to start login transaction");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Login failed",
            );
        }
    };

    let row = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>, String, Option<String>)>(
        "SELECT id, org_id, email, full_name, role, password_hash FROM users WHERE email = $1",
    )
    .bind(req.email.trim())
    .fetch_optional(tx.as_mut())
    .await;

    let row = match row {
        Ok(Some(r)) => r,
        Ok(None) => {
            return handlers::error_response(
                StatusCode::UNAUTHORIZED,
                "invalid_credentials",
                "Invalid email or password",
            );
        }
        Err(e) => {
            warn!(error = %e, "DB error fetching user");
            return handlers::error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Login failed",
            );
        }
    };

    let (user_id, org_id, email, full_name, _role, password_hash) = row;
    let _ = tx.commit().await;

    // Verify password
    let stored_hash = match password_hash {
        Some(h) => h,
        None => {
            return handlers::error_response(
                StatusCode::UNAUTHORIZED,
                "invalid_credentials",
                "Invalid email or password",
            );
        }
    };

    match verify(&req.password, &stored_hash) {
        Ok(true) => {}
        _ => {
            return handlers::error_response(
                StatusCode::UNAUTHORIZED,
                "invalid_credentials",
                "Invalid email or password",
            );
        }
    }

    let token = issue_jwt(&user_id, &org_id);
    record_api_product_event_if_available(
        &state,
        user_id,
        analytics::ProductEventName::UserLoggedIn,
        analytics::ResultTag::Success,
        serde_json::json!({
            "email_domain": email.split('@').nth(1).unwrap_or_default(),
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
                    id: user_id.to_string(),
                    email,
                    full_name: full_name.unwrap_or_default(),
                },
                reset_ticket: None,
            }),
            error: None,
        }),
    )
        .into_response()
}
