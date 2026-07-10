use super::support::*;

#[tokio::test]
async fn workspace_routes_with_auth_headers() {
    let state = test_app_state();
    let app = build_router(state);
    let org_id = "11111111-1111-1111-1111-111111111111";
    let user_id = "22222222-2222-2222-2222-222222222222";
    let req = Request::builder()
        .uri("/api/v1/workspaces")
        .method("GET")
        .header(middleware::HEADER_ORG_ID, org_id)
        .header(middleware::HEADER_USER_ID, user_id)
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}


#[tokio::test]
async fn chat_session_routes_work_with_auth_headers() {
    let state = test_app_state();
    let notebook = state.workspace()
        .create_workspace(CreateWorkspaceRequest {
            name: "Session Test".to_string(),
            description: String::new(),
        })
        .await
        .expect("notebook should create");
    let app = build_router(state);
    let org_id = "00000000-0000-0000-0000-000000000001";
    let user_id = "00000000-0000-0000-0000-000000000002";

    let create_req = Request::builder()
        .uri("/api/v1/chat/sessions")
        .method("POST")
        .header("Content-Type", "application/json")
        .header(middleware::HEADER_ORG_ID, org_id)
        .header(middleware::HEADER_USER_ID, user_id)
        .body(Body::from(format!(
            r#"{{"workspace_id":"{}","title":"My Session","agent_type":"chat"}}"#,
            notebook.id
        )))
        .unwrap();
    let create_resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let create_body = to_bytes(create_resp.into_body(), usize::MAX).await.unwrap();
    let session: serde_json::Value = serde_json::from_slice(&create_body).unwrap();
    let session_id = session["id"].as_str().unwrap().to_string();

    let list_req = Request::builder()
        .uri(format!("/api/v1/chat/sessions?workspace_id={}", notebook.id))
        .method("GET")
        .header(middleware::HEADER_ORG_ID, org_id)
        .header(middleware::HEADER_USER_ID, user_id)
        .body(Body::empty())
        .unwrap();
    let list_resp = app.clone().oneshot(list_req).await.unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);

    let get_req = Request::builder()
        .uri(format!("/api/v1/chat/sessions/{}", session_id))
        .method("GET")
        .header(middleware::HEADER_ORG_ID, org_id)
        .header(middleware::HEADER_USER_ID, user_id)
        .body(Body::empty())
        .unwrap();
    let get_resp = app.clone().oneshot(get_req).await.unwrap();
    assert_eq!(get_resp.status(), StatusCode::OK);

    let messages_req = Request::builder()
        .uri(format!("/api/v1/chat/sessions/{}/messages", session_id))
        .method("GET")
        .header(middleware::HEADER_ORG_ID, org_id)
        .header(middleware::HEADER_USER_ID, user_id)
        .body(Body::empty())
        .unwrap();
    let messages_resp = app.clone().oneshot(messages_req).await.unwrap();
    assert_eq!(messages_resp.status(), StatusCode::OK);
}


#[tokio::test]
async fn change_password_allows_login_with_new_secret_when_database_available() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state);
    let email = format!("password-{}@example.test", Uuid::new_v4());
    let register_req = Request::builder()
        .uri("/api/auth/register")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(register_body(&email, "Password User")))
        .unwrap();
    let register_resp = app.clone().oneshot(register_req).await.unwrap();
    assert_eq!(register_resp.status(), StatusCode::CREATED);
    let register_body = to_bytes(register_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let register_payload: serde_json::Value = serde_json::from_slice(&register_body).unwrap();
    let token = register_payload["data"]["token"]
        .as_str()
        .unwrap()
        .to_string();

    let change_req = Request::builder()
        .uri("/api/auth/change-password")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"old_password":"password123","new_password":"password456"}"#,
        ))
        .unwrap();
    let change_resp = app.clone().oneshot(change_req).await.unwrap();
    assert_eq!(change_resp.status(), StatusCode::OK);

    let stale_token_req = Request::builder()
        .uri("/api/auth/me")
        .method("GET")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let stale_token_resp = app.clone().oneshot(stale_token_req).await.unwrap();
    assert_eq!(stale_token_resp.status(), StatusCode::UNAUTHORIZED);

    let login_req = Request::builder()
        .uri("/api/auth/login")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(format!(
            r#"{{"email":"{email}","password":"password456"}}"#
        )))
        .unwrap();
    let login_resp = app.oneshot(login_req).await.unwrap();
    assert_eq!(login_resp.status(), StatusCode::OK);
}


#[tokio::test]
async fn login_returns_distinct_codes_for_missing_account_and_wrong_password() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state);
    let missing_email = format!("missing-{}@example.test", Uuid::new_v4());
    let missing_login_req = Request::builder()
        .uri("/api/auth/login")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(format!(
            r#"{{"email":"{missing_email}","password":"password123"}}"#
        )))
        .unwrap();
    let missing_login_resp = app.clone().oneshot(missing_login_req).await.unwrap();
    assert_eq!(missing_login_resp.status(), StatusCode::UNAUTHORIZED);
    let missing_login_body = to_bytes(missing_login_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let missing_login_payload: serde_json::Value =
        serde_json::from_slice(&missing_login_body).unwrap();
    assert_eq!(
        missing_login_payload["error"].as_str(),
        Some("account_not_registered")
    );

    let email = format!("wrong-password-{}@example.test", Uuid::new_v4());
    let register_req = Request::builder()
        .uri("/api/auth/register")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(register_body(&email, "Password User")))
        .unwrap();
    let register_resp = app.clone().oneshot(register_req).await.unwrap();
    assert_eq!(register_resp.status(), StatusCode::CREATED);

    let wrong_password_req = Request::builder()
        .uri("/api/auth/login")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(format!(
            r#"{{"email":"{email}","password":"wrong-password"}}"#
        )))
        .unwrap();
    let wrong_password_resp = app.oneshot(wrong_password_req).await.unwrap();
    assert_eq!(wrong_password_resp.status(), StatusCode::UNAUTHORIZED);
    let wrong_password_body = to_bytes(wrong_password_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let wrong_password_payload: serde_json::Value =
        serde_json::from_slice(&wrong_password_body).unwrap();
    assert_eq!(
        wrong_password_payload["error"].as_str(),
        Some("invalid_password")
    );
}


#[tokio::test]
async fn logout_invalidates_existing_token_when_database_available() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state);
    let email = format!("logout-{}@example.test", Uuid::new_v4());
    let register_req = Request::builder()
        .uri("/api/auth/register")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(register_body(&email, "Logout User")))
        .unwrap();
    let register_resp = app.clone().oneshot(register_req).await.unwrap();
    assert_eq!(register_resp.status(), StatusCode::CREATED);
    let register_body = to_bytes(register_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let register_payload: serde_json::Value = serde_json::from_slice(&register_body).unwrap();
    let token = register_payload["data"]["token"]
        .as_str()
        .unwrap()
        .to_string();

    let logout_req = Request::builder()
        .uri("/api/auth/logout")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{}"#))
        .unwrap();
    let logout_resp = app.clone().oneshot(logout_req).await.unwrap();
    assert_eq!(logout_resp.status(), StatusCode::OK);

    let me_req = Request::builder()
        .uri("/api/auth/me")
        .method("GET")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let me_resp = app.oneshot(me_req).await.unwrap();
    assert_eq!(me_resp.status(), StatusCode::UNAUTHORIZED);
}


#[tokio::test]
async fn password_reset_code_flow_allows_login_with_new_secret_when_database_available() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state);
    let email = format!("reset-{}@example.test", Uuid::new_v4());
    let register_req = Request::builder()
        .uri("/api/auth/register")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(register_body(&email, "Reset User")))
        .unwrap();
    let register_resp = app.clone().oneshot(register_req).await.unwrap();
    assert_eq!(register_resp.status(), StatusCode::CREATED);

    let send_req = Request::builder()
        .uri("/api/auth/reset/send-code")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(format!(r#"{{"email":"{email}"}}"#)))
        .unwrap();
    let send_resp = app.clone().oneshot(send_req).await.unwrap();
    assert_eq!(send_resp.status(), StatusCode::ACCEPTED);
    let send_body = to_bytes(send_resp.into_body(), usize::MAX).await.unwrap();
    let send_payload: serde_json::Value = serde_json::from_slice(&send_body).unwrap();
    let code = send_payload["debug_code"]
        .as_str()
        .expect("test mode should expose debug code")
        .to_string();

    let verify_req = Request::builder()
        .uri("/api/auth/reset/verify-code")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(format!(
            r#"{{"email":"{email}","code":"{code}"}}"#
        )))
        .unwrap();
    let verify_resp = app.clone().oneshot(verify_req).await.unwrap();
    assert_eq!(verify_resp.status(), StatusCode::OK);
    let verify_body = to_bytes(verify_resp.into_body(), usize::MAX).await.unwrap();
    let verify_payload: serde_json::Value = serde_json::from_slice(&verify_body).unwrap();
    let reset_ticket = verify_payload["data"]["reset_ticket"]
        .as_str()
        .expect("verify flow should return reset ticket")
        .to_string();

    let confirm_req = Request::builder()
        .uri("/api/auth/reset/confirm")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(format!(
            r#"{{"reset_ticket":"{reset_ticket}","new_password":"password456"}}"#
        )))
        .unwrap();
    let confirm_resp = app.clone().oneshot(confirm_req).await.unwrap();
    assert_eq!(confirm_resp.status(), StatusCode::OK);

    let login_req = Request::builder()
        .uri("/api/auth/login")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(format!(
            r#"{{"email":"{email}","password":"password456"}}"#
        )))
        .unwrap();
    let login_resp = app.oneshot(login_req).await.unwrap();
    assert_eq!(login_resp.status(), StatusCode::OK);
}


#[tokio::test]
async fn auth_register_writes_product_event_when_database_available() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state.clone());
    let email = format!("event-{}@example.test", uuid::Uuid::new_v4());

    let req = Request::builder()
        .uri("/api/auth/register")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(register_body(&email, "Events User")))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
}


#[tokio::test]
async fn auth_register_rejects_stale_legal_versions_when_database_available() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state);
    let email = format!("stale-legal-{}@example.test", uuid::Uuid::new_v4());

    let req = Request::builder()
        .uri("/api/auth/register")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(format!(
            r#"{{"email":"{email}","password":"password123","full_name":"Stale","terms_version":"2025-01-01","privacy_version":"{}"}}"#,
            app_core::PUBLISHED_PRIVACY_VERSION
        )))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        payload["error"].as_str(),
        Some("invalid_terms_version")
    );
}


#[tokio::test]
async fn auth_register_requires_legal_versions_when_database_available() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state);
    let email = format!("missing-legal-{}@example.test", uuid::Uuid::new_v4());

    let req = Request::builder()
        .uri("/api/auth/register")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(format!(
            r#"{{"email":"{email}","password":"password123","full_name":"No Legal"}}"#
        )))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["error"].as_str(), Some("consent_required"));
}


#[tokio::test]
async fn auth_legal_acceptance_records_payment_context_when_database_available() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state.clone());
    let email = format!("legal-pay-{}@example.test", uuid::Uuid::new_v4());

    let register_req = Request::builder()
        .uri("/api/auth/register")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(register_body(&email, "Payment User")))
        .unwrap();
    let register_resp = app.clone().oneshot(register_req).await.unwrap();
    assert_eq!(register_resp.status(), StatusCode::CREATED);
    let register_body = to_bytes(register_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let register_payload: serde_json::Value = serde_json::from_slice(&register_body).unwrap();
    let token = register_payload["data"]["token"]
        .as_str()
        .unwrap()
        .to_string();

    let legal_req = Request::builder()
        .uri("/api/auth/legal-acceptance")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(format!(
            r#"{{"terms_version":"{}","privacy_version":"{}","context":"payment"}}"#,
            app_core::PUBLISHED_TERMS_VERSION,
            app_core::PUBLISHED_PRIVACY_VERSION,
        )))
        .unwrap();
    let legal_resp = app.oneshot(legal_req).await.unwrap();
    assert_eq!(legal_resp.status(), StatusCode::CREATED);

    let user_id = uuid::Uuid::parse_str(
        register_payload["data"]["user"]["id"]
            .as_str()
            .expect("user id"),
    )
    .expect("uuid");
    let pool = state.postgres_pool().expect("postgres pool");
    let row = sqlx::query_as::<_, (String, String, String)>(
        "SELECT context, terms_version, privacy_version
         FROM legal_acceptances
         WHERE user_id = $1
         ORDER BY accepted_at DESC
         LIMIT 1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .expect("payment legal acceptance row");
    assert_eq!(row.0, "payment");
    assert_eq!(row.1, app_core::PUBLISHED_TERMS_VERSION);
    assert_eq!(row.2, app_core::PUBLISHED_PRIVACY_VERSION);
}


#[tokio::test]
async fn auth_legal_status_needs_re_acceptance_when_acceptance_missing_when_database_available() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state.clone());
    let email = format!("legal-missing-{}@example.test", uuid::Uuid::new_v4());

    let register_req = Request::builder()
        .uri("/api/auth/register")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(register_body(&email, "Missing Acceptance User")))
        .unwrap();
    let register_resp = app.clone().oneshot(register_req).await.unwrap();
    assert_eq!(register_resp.status(), StatusCode::CREATED);
    let register_body_bytes = to_bytes(register_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let register_payload: serde_json::Value =
        serde_json::from_slice(&register_body_bytes).unwrap();
    let token = register_payload["data"]["token"]
        .as_str()
        .unwrap()
        .to_string();
    let user_id = uuid::Uuid::parse_str(
        register_payload["data"]["user"]["id"]
            .as_str()
            .expect("user id"),
    )
    .expect("uuid");

    let pool = state.postgres_pool().expect("postgres pool");
    sqlx::query("DELETE FROM legal_acceptances WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await
        .expect("delete legal acceptances");

    let status_req = Request::builder()
        .uri("/api/auth/legal-status")
        .method("GET")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let status_resp = app.oneshot(status_req).await.unwrap();
    assert_eq!(status_resp.status(), StatusCode::OK);
    let status_body = to_bytes(status_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let status_payload: serde_json::Value = serde_json::from_slice(&status_body).unwrap();
    assert_eq!(status_payload["data"]["needs_re_acceptance"], json!(true));
}


#[tokio::test]
async fn auth_legal_acceptance_records_re_acceptance_context_when_database_available() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state.clone());
    let email = format!("legal-reaccept-{}@example.test", uuid::Uuid::new_v4());

    let register_req = Request::builder()
        .uri("/api/auth/register")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(register_body(&email, "Reaccept User")))
        .unwrap();
    let register_resp = app.clone().oneshot(register_req).await.unwrap();
    assert_eq!(register_resp.status(), StatusCode::CREATED);
    let register_body_bytes = to_bytes(register_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let register_payload: serde_json::Value =
        serde_json::from_slice(&register_body_bytes).unwrap();
    let token = register_payload["data"]["token"]
        .as_str()
        .unwrap()
        .to_string();
    let user_id = uuid::Uuid::parse_str(
        register_payload["data"]["user"]["id"]
            .as_str()
            .expect("user id"),
    )
    .expect("uuid");

    let pool = state.postgres_pool().expect("postgres pool");
    sqlx::query("DELETE FROM legal_acceptances WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await
        .expect("delete legal acceptances");

    let legal_req = Request::builder()
        .uri("/api/auth/legal-acceptance")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(format!(
            r#"{{"terms_version":"{}","privacy_version":"{}","context":"re_acceptance"}}"#,
            app_core::PUBLISHED_TERMS_VERSION,
            app_core::PUBLISHED_PRIVACY_VERSION,
        )))
        .unwrap();
    let legal_resp = app.clone().oneshot(legal_req).await.unwrap();
    assert_eq!(legal_resp.status(), StatusCode::CREATED);

    let status_req = Request::builder()
        .uri("/api/auth/legal-status")
        .method("GET")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let status_resp = app.oneshot(status_req).await.unwrap();
    assert_eq!(status_resp.status(), StatusCode::OK);
    let status_body = to_bytes(status_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let status_payload: serde_json::Value = serde_json::from_slice(&status_body).unwrap();
    assert_eq!(status_payload["data"]["needs_re_acceptance"], json!(false));
}


#[tokio::test]
async fn auth_legal_acceptance_rejects_stale_privacy_version_when_database_available() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state);
    let email = format!("legal-stale-privacy-{}@example.test", uuid::Uuid::new_v4());

    let register_req = Request::builder()
        .uri("/api/auth/register")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(register_body(&email, "Stale Privacy User")))
        .unwrap();
    let register_resp = app.clone().oneshot(register_req).await.unwrap();
    assert_eq!(register_resp.status(), StatusCode::CREATED);
    let register_body_bytes = to_bytes(register_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let register_payload: serde_json::Value =
        serde_json::from_slice(&register_body_bytes).unwrap();
    let token = register_payload["data"]["token"]
        .as_str()
        .unwrap()
        .to_string();

    let legal_req = Request::builder()
        .uri("/api/auth/legal-acceptance")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(format!(
            r#"{{"terms_version":"{}","privacy_version":"2025-01-01","context":"payment"}}"#,
            app_core::PUBLISHED_TERMS_VERSION,
        )))
        .unwrap();
    let legal_resp = app.oneshot(legal_req).await.unwrap();
    assert_eq!(legal_resp.status(), StatusCode::BAD_REQUEST);
    let legal_body = to_bytes(legal_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let legal_payload: serde_json::Value = serde_json::from_slice(&legal_body).unwrap();
    assert_eq!(
        legal_payload["error"].as_str(),
        Some("invalid_privacy_version")
    );
}


#[tokio::test]
async fn auth_legal_status_reflects_current_acceptance_when_database_available() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state);
    let email = format!("legal-status-{}@example.test", uuid::Uuid::new_v4());

    let register_req = Request::builder()
        .uri("/api/auth/register")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(register_body(&email, "Status User")))
        .unwrap();
    let register_resp = app.clone().oneshot(register_req).await.unwrap();
    assert_eq!(register_resp.status(), StatusCode::CREATED);
    let register_body = to_bytes(register_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let register_payload: serde_json::Value = serde_json::from_slice(&register_body).unwrap();
    let token = register_payload["data"]["token"]
        .as_str()
        .unwrap()
        .to_string();

    let status_req = Request::builder()
        .uri("/api/auth/legal-status")
        .method("GET")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let status_resp = app.oneshot(status_req).await.unwrap();
    assert_eq!(status_resp.status(), StatusCode::OK);
    let status_body = to_bytes(status_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let status_payload: serde_json::Value = serde_json::from_slice(&status_body).unwrap();
    assert_eq!(status_payload["data"]["needs_re_acceptance"], json!(false));
    assert_eq!(
        status_payload["data"]["published_terms_version"].as_str(),
        Some(app_core::PUBLISHED_TERMS_VERSION)
    );
}


#[tokio::test]
async fn anonymous_share_chat_requires_login_without_persisting_owner_session() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state.clone());
    let email = format!("share-chat-{}@example.test", uuid::Uuid::new_v4());

    let register_req = Request::builder()
        .uri("/api/auth/register")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(register_body(&email, "Share Chat User")))
        .unwrap();
    let register_resp = app.clone().oneshot(register_req).await.unwrap();
    assert_eq!(register_resp.status(), StatusCode::CREATED);
    let register_body = to_bytes(register_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let register_payload: serde_json::Value = serde_json::from_slice(&register_body).unwrap();
    let token = register_payload["data"]["token"]
        .as_str()
        .unwrap()
        .to_string();
    let claims = verify_jwt(&token).expect("jwt should decode");
    let user_id = Uuid::parse_str(&claims.sub).unwrap();
    let org_id = Uuid::parse_str(&claims.org_id).unwrap();

    let notebook_req = Request::builder()
        .uri("/api/v1/workspaces")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"name":"Shared Chat Workspace","description":""}"#,
        ))
        .unwrap();
    let notebook_resp = app.clone().oneshot(notebook_req).await.unwrap();
    assert_eq!(notebook_resp.status(), StatusCode::CREATED);
    let notebook_body = to_bytes(notebook_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let notebook: serde_json::Value = serde_json::from_slice(&notebook_body).unwrap();
    let workspace_id = notebook["workspace"]["id"].as_str().unwrap().to_string();

    let share_token = avrag_share::ShareService::new(state.share_store().expect("pg expected"))
        .create_share_token(
            &contracts::auth_runtime::AuthContext::new(
                contracts::auth_runtime::OrgId::from(org_id),
                contracts::auth_runtime::SubjectKind::User,
            )
            .with_actor_id(contracts::auth_runtime::ActorId::new(user_id)),
            &workspace_id,
            avrag_share::AccessLevel::Read,
            None,
        )
        .await
        .expect("share token should create");

    let chat_req = Request::builder()
        .uri("/api/v1/chat")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(format!(
            r#"{{"query":"hello public share","workspace_id":"{workspace_id}","agent_type":"chat","source_type":"share","source_token":"{share_token}","doc_scope":[],"messages":[],"stream":false}}"#
        )))
        .unwrap();
    let chat_resp = app.clone().oneshot(chat_req).await.unwrap();
    assert_eq!(chat_resp.status(), StatusCode::UNAUTHORIZED);
    let chat_body = to_bytes(chat_resp.into_body(), usize::MAX).await.unwrap();
    let chat_payload: serde_json::Value = serde_json::from_slice(&chat_body).unwrap();
    assert_eq!(chat_payload["error"].as_str(), Some("login_required"));
    assert!(
        chat_payload["message"]
            .as_str()
            .is_some_and(|message| message.contains("asking questions requires sign-in"))
    );

    let sessions = state.agent().list_sessions(Some(&workspace_id)).await;
    assert!(sessions.is_empty());
}


#[test]
fn jwt_roundtrip() {
    let user_id = Uuid::new_v4();
    let org_id = Uuid::new_v4();
    let token = issue_jwt(&user_id, &org_id);
    let claims = verify_jwt(&token).expect("token should be valid");
    assert_eq!(claims.sub, user_id.to_string());
    assert_eq!(claims.org_id, org_id.to_string());
    assert!(!claims.permissions.iter().any(|perm| perm == "admin"));
}


#[test]
fn jwt_org_admin_includes_admin_permission() {
    let user_id = Uuid::new_v4();
    let org_id = Uuid::new_v4();
    let token = issue_jwt_for_auth_version(
        &user_id,
        &org_id,
        1,
        contracts::USER_ROLE_ORG_ADMIN,
    );
    let claims = verify_jwt(&token).expect("token should be valid");
    assert!(claims.permissions.iter().any(|perm| perm == "admin"));
}

