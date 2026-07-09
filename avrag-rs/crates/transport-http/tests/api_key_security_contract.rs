use app_bootstrap::AppState;
use app_core::AppConfig;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use common::{CreateApiKeyRequest, CreateNotebookRequest, default_org_id, default_user_id};
use contracts::agent_permissions::{PERM_ADMIN, USER_ROLE_ORG_ADMIN};
use contracts::notebooks::CreateChatSessionRequest;
use std::env;
use tower::ServiceExt;
use transport_http::build_router;
use uuid::Uuid;

fn test_app_state() -> AppState {
    AppState::new(AppConfig::default())
}

async fn pg_test_app_state() -> Option<AppState> {
    let database_url = env::var("DATABASE_URL").ok()?;
    let mut config = AppConfig::default();
    config.database_url = Some(database_url);
    config.auto_migrate = true;
    AppState::bootstrap(config).await.ok()
}

fn register_body(email: &str, full_name: &str) -> String {
    format!(
        r#"{{"email":"{email}","password":"password123","full_name":"{full_name}","terms_version":"{}","privacy_version":"{}"}}"#,
        app_core::PUBLISHED_TERMS_VERSION,
        app_core::PUBLISHED_PRIVACY_VERSION,
    )
}

async fn register_and_get_token(app: &axum::Router, email: &str, full_name: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(register_body(email, full_name)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    payload["data"]["token"].as_str().unwrap().to_string()
}

fn admin_app_state() -> AppState {
    let state = test_app_state();
    state.with_auth(state.auth().clone().grant(PERM_ADMIN))
}

async fn rest_json(
    app: &axum::Router,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(bearer) = bearer {
        builder = builder.header("Authorization", format!("Bearer {bearer}"));
    }
    if body.is_some() {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
    }
    let request = builder
        .body(Body::from(
            body.unwrap_or(serde_json::json!({})).to_string(),
        ))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload =
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap_or(serde_json::Value::Null);
    (status, payload)
}

#[tokio::test]
async fn workspace_api_key_cannot_create_org_api_key() {
    let state = test_app_state();
    let notebook = state.docs()
        .create_notebook(CreateNotebookRequest {
            name: "key-mgmt".to_string(),
            description: String::new(),
        })
        .await
        .unwrap();
    let key = state.admin_api()
        .create_api_key(
            &notebook.id,
            CreateApiKeyRequest {
                name: "agent".to_string(),
                permissions: vec!["query".to_string()],
                rate_limit_rpm: Some(60),
                expires_at: None,
            },
        )
        .await
        .unwrap();
    let app = build_router(state);

    let (status, payload) = rest_json(
        &app,
        "POST",
        "/api/v1/org/api-keys",
        Some(&key.plaintext_key),
        Some(serde_json::json!({ "name": "blocked-org-key" })),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        payload.get("error").and_then(|value| value.as_str()),
        Some("api_key_forbidden")
    );
}
#[tokio::test]
async fn user_without_admin_cannot_create_org_api_key() {
    let app = build_router(test_app_state());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/org/api-keys")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-org-id", default_org_id())
                .header("x-user-id", default_user_id())
                .body(Body::from(
                    serde_json::json!({ "name": "needs-admin" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    assert_eq!(
        payload.get("error").and_then(|value| value.as_str()),
        Some("admin_required")
    );
}

#[tokio::test]
async fn admin_user_can_create_org_api_key() {
    let app = build_router(test_app_state());
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/org/api-keys")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-org-id", default_org_id())
                .header("x-user-id", default_user_id())
                .header("x-permissions", "admin")
                .body(Body::from(
                    serde_json::json!({ "name": "org-admin-key" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn workspace_api_key_cannot_read_other_workspace_session() {
    let state = test_app_state();
    let notebook_a = state.docs()
        .create_notebook(CreateNotebookRequest {
            name: "session-a".to_string(),
            description: String::new(),
        })
        .await
        .unwrap();
    let notebook_b = state.docs()
        .create_notebook(CreateNotebookRequest {
            name: "session-b".to_string(),
            description: String::new(),
        })
        .await
        .unwrap();
    let session_b = state.chat()
        .create_session(CreateChatSessionRequest {
            workspace_id: notebook_b.id.clone(),
            title: Some("private".to_string()),
            agent_type: "rag".to_string(),
        })
        .await
        .unwrap();
    let key = state.admin_api()
        .create_api_key(
            &notebook_a.id,
            CreateApiKeyRequest {
                name: "scoped-a".to_string(),
                permissions: vec!["query".to_string()],
                rate_limit_rpm: Some(60),
                expires_at: None,
            },
        )
        .await
        .unwrap();
    let app = build_router(state);

    let (status, payload) = rest_json(
        &app,
        "GET",
        &format!("/api/v1/chat/sessions/{}", session_b.id),
        Some(&key.plaintext_key),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        payload.get("error").and_then(|value| value.as_str()),
        Some("notebook_scope_mismatch")
    );
}

#[tokio::test]
async fn org_api_key_cannot_call_workspace_mcp_tool() {
    let state = admin_app_state();
    let org_key = state.admin_api()
        .create_org_api_key(CreateApiKeyRequest {
            name: "org-agent".to_string(),
            permissions: vec![],
            rate_limit_rpm: Some(60),
            expires_at: None,
        })
        .await
        .unwrap();
    let notebook = state.docs()
        .create_notebook(CreateNotebookRequest {
            name: "target".to_string(),
            description: String::new(),
        })
        .await
        .unwrap();
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/mcp")
                .header(header::CONTENT_TYPE, "application/json")
                .header("Authorization", format!("Bearer {}", org_key.plaintext_key))
                .body(Body::from(
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": "1",
                        "method": "tools/call",
                        "params": {
                            "name": "workspace.rag_query",
                            "arguments": {
                                "workspace_id": notebook.id,
                                "query": "blocked"
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    assert_eq!(
        payload
            .pointer("/error/data/error")
            .and_then(|value| value.as_str()),
        Some("org_key_cannot_call_workspace_tools")
    );
}

#[tokio::test]
async fn workspace_api_key_cannot_create_workspace_api_key() {
    let state = test_app_state();
    let notebook = state.docs()
        .create_notebook(CreateNotebookRequest {
            name: "nested-keys".to_string(),
            description: String::new(),
        })
        .await
        .unwrap();
    let key = state.admin_api()
        .create_api_key(
            &notebook.id,
            CreateApiKeyRequest {
                name: "agent".to_string(),
                permissions: vec!["query".to_string()],
                rate_limit_rpm: Some(60),
                expires_at: None,
            },
        )
        .await
        .unwrap();
    let app = build_router(state);

    let (status, payload) = rest_json(
        &app,
        "POST",
        &format!("/api/v1/workspaces/{}/api-keys", notebook.id),
        Some(&key.plaintext_key),
        Some(serde_json::json!({ "name": "blocked-workspace-key" })),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        payload.get("error").and_then(|value| value.as_str()),
        Some("api_key_forbidden")
    );
}
#[tokio::test]
async fn workspace_api_key_cannot_list_notebook_notes() {
    let state = test_app_state();
    let notebook = state.docs()
        .create_notebook(CreateNotebookRequest {
            name: "notes-ui".to_string(),
            description: String::new(),
        })
        .await
        .unwrap();
    let key = state.admin_api()
        .create_api_key(
            &notebook.id,
            CreateApiKeyRequest {
                name: "agent".to_string(),
                permissions: vec!["query".to_string()],
                rate_limit_rpm: Some(60),
                expires_at: None,
            },
        )
        .await
        .unwrap();
    let app = build_router(state);

    let (status, payload) = rest_json(
        &app,
        "GET",
        &format!("/api/v1/workspaces/{}/notes", notebook.id),
        Some(&key.plaintext_key),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        payload.get("error").and_then(|value| value.as_str()),
        Some("api_key_forbidden")
    );
}

#[tokio::test]
async fn workspace_api_key_cannot_update_other_workspace_session() {
    let state = test_app_state();
    let notebook_a = state.docs()
        .create_notebook(CreateNotebookRequest {
            name: "session-update-a".to_string(),
            description: String::new(),
        })
        .await
        .unwrap();
    let notebook_b = state.docs()
        .create_notebook(CreateNotebookRequest {
            name: "session-update-b".to_string(),
            description: String::new(),
        })
        .await
        .unwrap();
    let session_b = state.chat()
        .create_session(CreateChatSessionRequest {
            workspace_id: notebook_b.id.clone(),
            title: Some("private".to_string()),
            agent_type: "rag".to_string(),
        })
        .await
        .unwrap();
    let key = state.admin_api()
        .create_api_key(
            &notebook_a.id,
            CreateApiKeyRequest {
                name: "scoped-a".to_string(),
                permissions: vec!["query".to_string()],
                rate_limit_rpm: Some(60),
                expires_at: None,
            },
        )
        .await
        .unwrap();
    let app = build_router(state);

    let (status, payload) = rest_json(
        &app,
        "PUT",
        &format!("/api/v1/chat/sessions/{}", session_b.id),
        Some(&key.plaintext_key),
        Some(serde_json::json!({ "title": "hacked" })),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        payload.get("error").and_then(|value| value.as_str()),
        Some("notebook_scope_mismatch")
    );
}

#[tokio::test]
async fn admin_user_can_list_and_revoke_org_api_keys() {
    let state = admin_app_state();
    let created = state.admin_api()
        .create_org_api_key(CreateApiKeyRequest {
            name: "listed-org-key".to_string(),
            permissions: vec![],
            rate_limit_rpm: Some(60),
            expires_at: None,
        })
        .await
        .unwrap();
    let app = build_router(state);

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/org/api-keys")
                .header("x-org-id", default_org_id())
                .header("x-user-id", default_user_id())
                .header("x-permissions", "admin")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = to_bytes(list_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let list_payload = serde_json::from_slice::<serde_json::Value>(&list_body).unwrap();
    assert!(
        list_payload
            .pointer("/api_keys")
            .and_then(|value| value.as_array())
            .is_some_and(|items| items.iter().any(|item| {
                item.get("id").and_then(|value| value.as_str()) == Some(created.api_key.id.as_str())
            }))
    );

    let revoke_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/org/api-keys/{}", created.api_key.id))
                .header("x-org-id", default_org_id())
                .header("x-user-id", default_user_id())
                .header("x-permissions", "admin")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoke_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn org_admin_jwt_can_create_org_api_key() {
    use transport_http::issue_jwt_for_auth_version;
    use uuid::Uuid;

    let user_id = Uuid::parse_str(&default_user_id()).unwrap();
    let org_id = Uuid::parse_str(&default_org_id()).unwrap();
    let token = issue_jwt_for_auth_version(&user_id, &org_id, 1, USER_ROLE_ORG_ADMIN);
    let app = build_router(test_app_state());

    let (status, _) = rest_json(
        &app,
        "POST",
        "/api/v1/org/api-keys",
        Some(&token),
        Some(serde_json::json!({ "name": "jwt-org-admin-key" })),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn workspace_api_key_cannot_read_user_preferences() {
    let state = test_app_state();
    let notebook = state.docs()
        .create_notebook(CreateNotebookRequest {
            name: "prefs-ui".to_string(),
            description: String::new(),
        })
        .await
        .unwrap();
    let key = state.admin_api()
        .create_api_key(
            &notebook.id,
            CreateApiKeyRequest {
                name: "agent".to_string(),
                permissions: vec!["query".to_string()],
                rate_limit_rpm: Some(60),
                expires_at: None,
            },
        )
        .await
        .unwrap();
    let app = build_router(state);

    let (status, payload) = rest_json(
        &app,
        "GET",
        "/api/auth/preferences",
        Some(&key.plaintext_key),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        payload.get("error").and_then(|value| value.as_str()),
        Some("api_key_forbidden")
    );
}
#[tokio::test]
async fn workspace_api_key_cannot_update_profile() {
    let state = test_app_state();
    let notebook = state.docs()
        .create_notebook(CreateNotebookRequest {
            name: "profile-ui".to_string(),
            description: String::new(),
        })
        .await
        .unwrap();
    let key = state.admin_api()
        .create_api_key(
            &notebook.id,
            CreateApiKeyRequest {
                name: "agent".to_string(),
                permissions: vec!["query".to_string()],
                rate_limit_rpm: Some(60),
                expires_at: None,
            },
        )
        .await
        .unwrap();
    let app = build_router(state);

    let (status, payload) = rest_json(
        &app,
        "PUT",
        "/api/auth/profile",
        Some(&key.plaintext_key),
        Some(serde_json::json!({ "full_name": "Blocked" })),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        payload.get("error").and_then(|value| value.as_str()),
        Some("api_key_forbidden")
    );
}

#[tokio::test]
async fn user_without_notebook_access_cannot_list_workspace_api_keys() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state);
    let owner_email = format!("owner-{}@example.test", Uuid::new_v4());
    let outsider_email = format!("outsider-{}@example.test", Uuid::new_v4());
    let owner_token = register_and_get_token(&app, &owner_email, "Owner").await;
    let outsider_token = register_and_get_token(&app, &outsider_email, "Outsider").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workspaces")
                .header(header::CONTENT_TYPE, "application/json")
                .header("Authorization", format!("Bearer {owner_token}"))
                .body(Body::from(
                    serde_json::json!({
                        "name": "private-workspace",
                        "description": ""
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let workspace_id =
        serde_json::from_slice::<serde_json::Value>(&create_body).unwrap()["notebook"]["id"]
            .as_str()
            .unwrap()
            .to_string();

    let (status, payload) = rest_json(
        &app,
        "GET",
        &format!("/api/v1/workspaces/{workspace_id}/api-keys"),
        Some(&outsider_token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        payload.get("error").and_then(|value| value.as_str()),
        Some("notebook_access_required")
    );
}

#[tokio::test]
async fn user_without_notebook_access_cannot_revoke_workspace_api_key() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state);
    let owner_email = format!("owner-revoke-{}@example.test", Uuid::new_v4());
    let outsider_email = format!("outsider-revoke-{}@example.test", Uuid::new_v4());
    let owner_token = register_and_get_token(&app, &owner_email, "Owner").await;
    let outsider_token = register_and_get_token(&app, &outsider_email, "Outsider").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workspaces")
                .header(header::CONTENT_TYPE, "application/json")
                .header("Authorization", format!("Bearer {owner_token}"))
                .body(Body::from(
                    serde_json::json!({
                        "name": "revoke-workspace",
                        "description": ""
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let workspace_id =
        serde_json::from_slice::<serde_json::Value>(&create_body).unwrap()["notebook"]["id"]
            .as_str()
            .unwrap()
            .to_string();

    let key_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workspaces/{workspace_id}/api-keys"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("Authorization", format!("Bearer {owner_token}"))
                .body(Body::from(
                    serde_json::json!({ "name": "owner-key" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(key_response.status(), StatusCode::CREATED);
    let key_body = to_bytes(key_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let key_id = serde_json::from_slice::<serde_json::Value>(&key_body).unwrap()["api_key"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let (status, payload) = rest_json(
        &app,
        "DELETE",
        &format!("/api/v1/workspaces/{workspace_id}/api-keys/{key_id}"),
        Some(&outsider_token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        payload.get("error").and_then(|value| value.as_str()),
        Some("notebook_access_required")
    );
}

#[tokio::test]
async fn org_api_key_create_strips_admin_permission() {
    let state = admin_app_state();
    let created = state.admin_api()
        .create_org_api_key(CreateApiKeyRequest {
            name: "org-key".to_string(),
            permissions: vec![
                PERM_ADMIN.to_string(),
                contracts::PERM_WORKSPACE_CREATE.to_string(),
                contracts::PERM_INDEX.to_string(),
            ],
            rate_limit_rpm: Some(60),
            expires_at: None,
        })
        .await
        .unwrap();

    assert_eq!(
        created.api_key.permissions,
        vec![contracts::PERM_WORKSPACE_CREATE.to_string()]
    );
}

#[tokio::test]
async fn workspace_api_key_create_strips_admin_permission() {
    let state = test_app_state();
    let notebook = state.docs()
        .create_notebook(CreateNotebookRequest {
            name: "strip-admin".to_string(),
            description: String::new(),
        })
        .await
        .unwrap();
    let created = state.admin_api()
        .create_api_key(
            &notebook.id,
            CreateApiKeyRequest {
                name: "agent".to_string(),
                permissions: vec![PERM_ADMIN.to_string(), contracts::PERM_QUERY.to_string()],
                rate_limit_rpm: Some(60),
                expires_at: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(
        created.api_key.permissions,
        vec![contracts::PERM_QUERY.to_string()]
    );
}
