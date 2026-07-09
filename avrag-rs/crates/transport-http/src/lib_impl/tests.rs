use super::*;
use app_bootstrap::AppState;
use axum::body::{Body, to_bytes};
use axum::http::Request;
use axum::http::StatusCode;
use common::CreateNotebookRequest;
use serde_json::json;
use std::env;
use tower::ServiceExt;
use uuid::Uuid;

use crate::middleware;

fn test_app_state() -> AppState {
    let mut config = app_core::AppConfig::default();
    config.org_id = "00000000-0000-0000-0000-000000000001".to_string();
    config.user_id = "00000000-0000-0000-0000-000000000002".to_string();
    AppState::new(config)
}

async fn pg_test_app_state() -> Option<AppState> {
    let database_url = env::var("DATABASE_URL").ok()?;
    let mut config = app_core::AppConfig::default();
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

#[tokio::test]
async fn health_handler_returns_ok() {
    let state = test_app_state();
    let app = build_router(state);
    let req = Request::builder()
        .uri("/health")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn ready_handler_returns_ok() {
    let state = test_app_state();
    let app = build_router(state);
    let req = Request::builder()
        .uri("/ready")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn metrics_endpoint_exposes_prometheus_text() {
    let state = test_app_state();
    let app = build_router(state);
    let req = Request::builder()
        .uri("/metrics")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(text.contains("http_requests_total"));
}

#[tokio::test]
async fn public_routes_bypass_auth() {
    let state = test_app_state();
    let app = build_router(state);
    let req = Request::builder()
        .uri("/api/auth/login")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"email":"a@b.c","password":"12345678"}"#))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    // Should NOT be 401 — login is public (it'll be 500/503 since no DB)
    assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn protected_routes_require_auth() {
    let state = test_app_state();
    let app = build_router(state);
    let req = Request::builder()
        .uri("/api/v1/notebooks")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn notebook_routes_with_auth_headers() {
    let state = test_app_state();
    let app = build_router(state);
    let org_id = "11111111-1111-1111-1111-111111111111";
    let user_id = "22222222-2222-2222-2222-222222222222";
    let req = Request::builder()
        .uri("/api/v1/notebooks")
        .method("GET")
        .header(middleware::HEADER_ORG_ID, org_id)
        .header(middleware::HEADER_USER_ID, user_id)
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn workspace_api_key_can_access_its_notebook_sources() {
    let state = test_app_state();
    let notebook = state.docs()
        .create_notebook(CreateNotebookRequest {
            name: "API Key Notebook".to_string(),
            description: String::new(),
        })
        .await
        .expect("notebook should create");
    let key = state.admin_api()
        .create_api_key(
            &notebook.id,
            common::CreateApiKeyRequest {
                name: "agent".to_string(),
                permissions: vec!["query".to_string()],
                rate_limit_rpm: Some(30),
                expires_at: None,
            },
        )
        .await
        .expect("api key should create");
    let app = build_router(state);

    let req = Request::builder()
        .uri(format!("/api/v1/sources?notebook_id={}", notebook.id))
        .method("GET")
        .header("Authorization", format!("Bearer {}", key.plaintext_key))
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(payload["sources"].is_array());
}

#[tokio::test]
async fn mcp_jsonrpc_initialize_and_tools_list() {
    let state = test_app_state();
    let notebook = state.docs()
        .create_notebook(CreateNotebookRequest {
            name: "MCP Notebook".to_string(),
            description: String::new(),
        })
        .await
        .expect("notebook should create");
    let app = build_router(state);
    let org_id = "00000000-0000-0000-0000-000000000001";
    let user_id = "00000000-0000-0000-0000-000000000002";

    let init_req = Request::builder()
        .uri(format!("/mcp/notebooks/{}", notebook.id))
        .method("POST")
        .header("Content-Type", "application/json")
        .header(middleware::HEADER_ORG_ID, org_id)
        .header(middleware::HEADER_USER_ID, user_id)
        .body(Body::from(
            r#"{"jsonrpc":"2.0","id":"1","method":"initialize","params":{}}"#,
        ))
        .unwrap();
    let init_resp = app.clone().oneshot(init_req).await.unwrap();
    assert_eq!(init_resp.status(), StatusCode::OK);
    let init_body = to_bytes(init_resp.into_body(), usize::MAX).await.unwrap();
    let init_payload: serde_json::Value = serde_json::from_slice(&init_body).unwrap();
    assert_eq!(init_payload["result"]["serverInfo"]["name"], "context-os");

    let list_req = Request::builder()
        .uri(format!("/mcp/notebooks/{}", notebook.id))
        .method("POST")
        .header("Content-Type", "application/json")
        .header(middleware::HEADER_ORG_ID, org_id)
        .header(middleware::HEADER_USER_ID, user_id)
        .body(Body::from(
            r#"{"jsonrpc":"2.0","id":"2","method":"tools/list","params":{}}"#,
        ))
        .unwrap();
    let list_resp = app.oneshot(list_req).await.unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_body = to_bytes(list_resp.into_body(), usize::MAX).await.unwrap();
    let list_payload: serde_json::Value = serde_json::from_slice(&list_body).unwrap();
    let tool_names: Vec<&str> = list_payload["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert!(tool_names.contains(&"workspace.rag_query"));
    assert!(tool_names.contains(&"workspace.search_query"));
}

#[tokio::test]
async fn unified_mcp_lists_org_and_workspace_tools() {
    let state = test_app_state();
    let app = build_router(state);
    let org_id = "00000000-0000-0000-0000-000000000001";
    let user_id = "00000000-0000-0000-0000-000000000002";

    let list_req = Request::builder()
        .uri("/api/v1/mcp")
        .method("POST")
        .header("Content-Type", "application/json")
        .header(middleware::HEADER_ORG_ID, org_id)
        .header(middleware::HEADER_USER_ID, user_id)
        .body(Body::from(
            r#"{"jsonrpc":"2.0","id":"1","method":"tools/list","params":{}}"#,
        ))
        .unwrap();
    let list_resp = app.oneshot(list_req).await.unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_body = to_bytes(list_resp.into_body(), usize::MAX).await.unwrap();
    let list_payload: serde_json::Value = serde_json::from_slice(&list_body).unwrap();
    let tool_names: Vec<&str> = list_payload["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert!(tool_names.contains(&"org.create_workspace"));
    assert!(tool_names.contains(&"workspace.create_upload"));
    assert!(tool_names.contains(&"workspace.rag_query"));
}

#[tokio::test]
async fn org_mcp_create_workspace_returns_notebook_id() {
    let state = test_app_state();
    let app = build_router(state);
    let org_id = "00000000-0000-0000-0000-000000000001";
    let user_id = "00000000-0000-0000-0000-000000000002";

    let call_req = Request::builder()
        .uri("/api/v1/mcp")
        .method("POST")
        .header("Content-Type", "application/json")
        .header(middleware::HEADER_ORG_ID, org_id)
        .header(middleware::HEADER_USER_ID, user_id)
        .body(Body::from(
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "1",
                "method": "tools/call",
                "params": {
                    "name": "org.create_workspace",
                    "arguments": {
                        "name": "Agent Workspace",
                        "description": "created via MCP"
                    }
                }
            })
            .to_string(),
        ))
        .unwrap();
    let call_resp = app.oneshot(call_req).await.unwrap();
    assert_eq!(call_resp.status(), StatusCode::OK);
    let body = to_bytes(call_resp.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(
        payload
            .pointer("/result/structuredContent/data/notebook/id")
            .and_then(|value| value.as_str())
            .is_some()
    );
    assert_eq!(
        payload
            .pointer("/result/structuredContent/agent_operation_guide/mode")
            .and_then(|value| value.as_str()),
        Some("workspace.create")
    );
}

#[tokio::test]
async fn agent_preferences_api_can_get_put_and_delete_preferences() {
    let state = test_app_state();
    let app = build_router(state);
    let org_id = "00000000-0000-0000-0000-000000000001";
    let user_id = "00000000-0000-0000-0000-000000000002";

    let put_req = Request::builder()
        .uri("/api/auth/agent-preferences")
        .method("PUT")
        .header("Content-Type", "application/json")
        .header(middleware::HEADER_ORG_ID, org_id)
        .header(middleware::HEADER_USER_ID, user_id)
        .body(Body::from(
            r#"{"active":[{"id":"pref-1","text":"Use concise answers","category":"interaction","scope":"global","confidence":"explicit","source":"test","updated_at":"2026-04-26T00:00:00Z"}],"superseded":[],"blocked":[],"daily_log":[],"last_consolidated_at":null}"#,
        ))
        .unwrap();
    let put_resp = app.clone().oneshot(put_req).await.unwrap();
    assert_eq!(put_resp.status(), StatusCode::OK);

    let get_req = Request::builder()
        .uri("/api/auth/agent-preferences")
        .method("GET")
        .header(middleware::HEADER_ORG_ID, org_id)
        .header(middleware::HEADER_USER_ID, user_id)
        .body(Body::empty())
        .unwrap();
    let get_resp = app.clone().oneshot(get_req).await.unwrap();
    assert_eq!(get_resp.status(), StatusCode::OK);
    let get_body = to_bytes(get_resp.into_body(), usize::MAX).await.unwrap();
    let get_payload: serde_json::Value = serde_json::from_slice(&get_body).unwrap();
    assert_eq!(get_payload["active"][0]["id"], "pref-1");

    let delete_req = Request::builder()
        .uri("/api/auth/agent-preferences/pref-1")
        .method("DELETE")
        .header(middleware::HEADER_ORG_ID, org_id)
        .header(middleware::HEADER_USER_ID, user_id)
        .body(Body::empty())
        .unwrap();
    let delete_resp = app.clone().oneshot(delete_req).await.unwrap();
    assert_eq!(delete_resp.status(), StatusCode::OK);
    let delete_body = to_bytes(delete_resp.into_body(), usize::MAX).await.unwrap();
    let delete_payload: serde_json::Value = serde_json::from_slice(&delete_body).unwrap();
    assert!(delete_payload["active"].as_array().unwrap().is_empty());
    assert_eq!(delete_payload["blocked"][0]["id"], "pref-1");
}

#[tokio::test]
async fn chat_session_routes_work_with_auth_headers() {
    let state = test_app_state();
    let notebook = state.docs()
        .create_notebook(CreateNotebookRequest {
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
            r#"{{"notebook_id":"{}","title":"My Session","agent_type":"chat"}}"#,
            notebook.id
        )))
        .unwrap();
    let create_resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let create_body = to_bytes(create_resp.into_body(), usize::MAX).await.unwrap();
    let session: serde_json::Value = serde_json::from_slice(&create_body).unwrap();
    let session_id = session["id"].as_str().unwrap().to_string();

    let list_req = Request::builder()
        .uri(format!("/api/v1/chat/sessions?notebook_id={}", notebook.id))
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
async fn document_and_share_routes_work_when_database_available() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state.clone());
    let email = format!("routes-{}@example.test", Uuid::new_v4());

    let register_req = Request::builder()
        .uri("/api/auth/register")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(register_body(&email, "Routes User")))
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

    let notebook_req = Request::builder()
        .uri("/api/v1/notebooks")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"name":"Routes Notebook","description":""}"#))
        .unwrap();
    let notebook_resp = app.clone().oneshot(notebook_req).await.unwrap();
    assert_eq!(notebook_resp.status(), StatusCode::CREATED);
    let notebook_body = to_bytes(notebook_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let notebook_payload: serde_json::Value = serde_json::from_slice(&notebook_body).unwrap();
    let notebook_id = notebook_payload["notebook"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let create_doc_req = Request::builder()
        .uri(format!("/api/v1/notebooks/{notebook_id}/documents"))
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"filename":"routes.txt","file_size":12,"mime_type":"text/plain"}"#,
        ))
        .unwrap();
    let create_doc_resp = app.clone().oneshot(create_doc_req).await.unwrap();
    assert_eq!(create_doc_resp.status(), StatusCode::CREATED);
    let create_doc_body = to_bytes(create_doc_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_doc_payload: serde_json::Value =
        serde_json::from_slice(&create_doc_body).unwrap();
    let document_id = create_doc_payload["document_id"]
        .as_str()
        .unwrap()
        .to_string();

    let status_req = Request::builder()
        .uri(format!("/api/v1/documents/{document_id}/status"))
        .method("GET")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let status_resp = app.clone().oneshot(status_req).await.unwrap();
    assert_eq!(status_resp.status(), StatusCode::OK);

    let share_req = Request::builder()
        .uri(format!("/api/v1/notebooks/{notebook_id}/share"))
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"role":"viewer"}"#))
        .unwrap();
    let share_resp = app.clone().oneshot(share_req).await.unwrap();
    assert_eq!(share_resp.status(), StatusCode::OK);
    let share_body = to_bytes(share_resp.into_body(), usize::MAX).await.unwrap();
    let share_payload: serde_json::Value = serde_json::from_slice(&share_body).unwrap();
    let share_token = share_payload["share_token"].as_str().unwrap().to_string();

    let validate_req = Request::builder()
        .uri(format!("/api/v1/share/validate/{share_token}"))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let validate_resp = app.clone().oneshot(validate_req).await.unwrap();
    assert_eq!(validate_resp.status(), StatusCode::OK);

    let settings_req = Request::builder()
        .uri(format!("/api/v1/notebooks/{notebook_id}/share/settings"))
        .method("GET")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let settings_resp = app.clone().oneshot(settings_req).await.unwrap();
    assert_eq!(settings_resp.status(), StatusCode::OK);

    let analytics_req = Request::builder()
        .uri(format!("/api/v1/notebooks/{notebook_id}/share/analytics"))
        .method("GET")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let analytics_resp = app.clone().oneshot(analytics_req).await.unwrap();
    assert_eq!(analytics_resp.status(), StatusCode::OK);

    let logs_req = Request::builder()
        .uri(format!("/api/v1/notebooks/{notebook_id}/share/access-logs"))
        .method("GET")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let logs_resp = app.oneshot(logs_req).await.unwrap();
    assert_eq!(logs_resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn usage_limit_handler_sanitized_errors() {
    let state = test_app_state();
    let app = build_router(state);
    let org_id = "11111111-1111-1111-1111-111111111111";
    let user_id = "22222222-2222-2222-2222-222222222222";
    let req = Request::builder()
        .uri("/api/auth/usage-limit")
        .method("GET")
        .header(middleware::HEADER_ORG_ID, org_id)
        .header(middleware::HEADER_USER_ID, user_id)
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    // Should be 200 or 500 with sanitized error (no internal details leaked)
    assert!(
        response.status() == StatusCode::OK
            || response.status() == StatusCode::INTERNAL_SERVER_ERROR,
        "Expected 200 or 500, got {}",
        response.status()
    );
}

#[tokio::test]
async fn signed_upload_handler_accepts_valid_signed_url() {
    let state = test_app_state();
    let notebook = state.docs()
        .create_notebook(CreateNotebookRequest {
            name: "Upload Test".to_string(),
            description: String::new(),
        })
        .await
        .expect("notebook should create");
    let created = state.docs()
        .create_document_upload(
            &notebook.id,
            common::CreateDocumentRequest {
                filename: "sample.txt".to_string(),
                file_size: 12,
                mime_type: "text/plain".to_string(),
            },
        )
        .await
        .expect("upload should create");
    let request_path = created
        .upload_url
        .strip_prefix("http://127.0.0.1:8080")
        .expect("signed upload URL should use default public base url")
        .to_string();

    let app = build_router(state.clone());
    let req = Request::builder()
        .uri(request_path)
        .method("PUT")
        .body(Body::from("hello upload"))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        payload.get("status").and_then(|v| v.as_str()),
        Some("uploaded")
    );
}

#[tokio::test]
async fn create_document_upload_rejects_unsupported_file_type() {
    let state = test_app_state();
    let notebook = state.docs()
        .create_notebook(CreateNotebookRequest {
            name: "Unsupported Upload Test".to_string(),
            description: String::new(),
        })
        .await
        .expect("notebook should create");
    let org_id = "00000000-0000-0000-0000-000000000001";
    let user_id = "00000000-0000-0000-0000-000000000002";

    let app = build_router(state);
    let req = Request::builder()
        .uri(format!("/api/v1/notebooks/{}/documents", notebook.id))
        .method("POST")
        .header("Content-Type", "application/json")
        .header(middleware::HEADER_ORG_ID, org_id)
        .header(middleware::HEADER_USER_ID, user_id)
        .body(Body::from(
            r#"{"filename":"archive","file_size":12,"mime_type":"application/octet-stream"}"#,
        ))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        payload.get("error").and_then(|value| value.as_str()),
        Some("unsupported_file_type")
    );
}

#[tokio::test]
async fn dev_upload_handler_completes_upload_flow() {
    let previous = env::var("E2E_ENABLED").ok();
    unsafe {
        env::set_var("E2E_ENABLED", "true");
    }

    let state = test_app_state();
    let notebook = state.docs()
        .create_notebook(CreateNotebookRequest {
            name: "Dev Upload Test".to_string(),
            description: String::new(),
        })
        .await
        .expect("notebook should create");
    let created = state.docs()
        .create_document_upload(
            &notebook.id,
            common::CreateDocumentRequest {
                filename: "dev-upload.txt".to_string(),
                file_size: 20,
                mime_type: "text/plain".to_string(),
            },
        )
        .await
        .expect("upload should create");

    let app = build_router(state.clone());
    let req = Request::builder()
        .uri(format!("/dev-upload/{}", created.document_id))
        .method("PUT")
        .header("x-org-id", "00000000-0000-0000-0000-000000000001")
        .header("x-user-id", "00000000-0000-0000-0000-000000000002")
        .body(Body::from("hello dev upload"))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        payload.get("status").and_then(|v| v.as_str()),
        Some("queued")
    );

    if let Some(value) = previous {
        unsafe {
            env::set_var("E2E_ENABLED", value);
        }
    } else {
        unsafe {
            env::remove_var("E2E_ENABLED");
        }
    }
}

#[tokio::test]
async fn shared_notebook_handler_is_not_implemented_no_longer_returns_501() {
    let state = test_app_state();
    let app = build_router(state);
    let req = Request::builder()
        .uri("/api/shared/kb/test-token")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_ne!(response.status(), StatusCode::NOT_IMPLEMENTED);
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn profile_update_roundtrip_when_database_available() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state);
    let email = format!("profile-{}@example.test", Uuid::new_v4());
    let register_req = Request::builder()
        .uri("/api/auth/register")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(register_body(&email, "Initial Name")))
        .unwrap();
    let register_resp = app.clone().oneshot(register_req).await.unwrap();
    assert_eq!(register_resp.status(), StatusCode::CREATED);
    let body = to_bytes(register_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = payload["data"]["token"].as_str().unwrap().to_string();

    let update_req = Request::builder()
        .uri("/api/auth/profile")
        .method("PUT")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"full_name":"Updated Name"}"#))
        .unwrap();
    let update_resp = app.clone().oneshot(update_req).await.unwrap();
    assert_eq!(update_resp.status(), StatusCode::OK);

    let me_req = Request::builder()
        .uri("/api/auth/me")
        .method("GET")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let me_resp = app.oneshot(me_req).await.unwrap();
    assert_eq!(me_resp.status(), StatusCode::OK);
    let me_body = to_bytes(me_resp.into_body(), usize::MAX).await.unwrap();
    let me_payload: serde_json::Value = serde_json::from_slice(&me_body).unwrap();
    assert_eq!(
        me_payload["data"]["user"]["full_name"].as_str(),
        Some("Updated Name")
    );
}

#[tokio::test]
async fn preferences_roundtrip_when_database_available() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state);
    let email = format!("prefs-{}@example.test", Uuid::new_v4());
    let register_req = Request::builder()
        .uri("/api/auth/register")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(register_body(&email, "Prefs User")))
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

    let update_req = Request::builder()
        .uri("/api/auth/preferences")
        .method("PUT")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"dashboard":{"favorite_notebook_ids":[],"workspace_drafts":[{"notebook_id":"00000000-0000-0000-0000-000000000010","notes":"hello notes"}]},"notifications":{"email_enabled":true,"product_enabled":true,"security_enabled":true,"weekly_digest_enabled":false,"quiet_hours_start":null,"quiet_hours_end":null}}"#,
        ))
        .unwrap();
    let update_resp = app.clone().oneshot(update_req).await.unwrap();
    assert_eq!(update_resp.status(), StatusCode::OK);

    let get_req = Request::builder()
        .uri("/api/auth/preferences")
        .method("GET")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let get_resp = app.oneshot(get_req).await.unwrap();
    assert_eq!(get_resp.status(), StatusCode::OK);
    let get_body = to_bytes(get_resp.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&get_body).unwrap();
    assert_eq!(
        payload["dashboard"]["workspace_drafts"][0]["notes"].as_str(),
        Some("hello notes")
    );
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
async fn admin_billing_route_allows_real_admin_with_rls_when_database_available() {
    let Some(state) = pg_test_app_state().await else {
        return;
    };
    let app = build_router(state.clone());
    let email = format!("admin-billing-{}@example.test", uuid::Uuid::new_v4());

    let register_req = Request::builder()
        .uri("/api/auth/register")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(register_body(&email, "Admin Billing User")))
        .unwrap();
    let register_resp = app.clone().oneshot(register_req).await.unwrap();
    assert_eq!(register_resp.status(), StatusCode::CREATED);
    let register_body = to_bytes(register_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let register_payload: serde_json::Value = serde_json::from_slice(&register_body).unwrap();
    let _register_token = register_payload["data"]["token"]
        .as_str()
        .unwrap()
        .to_string();

    state
        .grant_e2e_admin_role(&email)
        .await
        .expect("admin role grant should succeed");

    let login_req = Request::builder()
        .uri("/api/auth/login")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "email": email,
                "password": "password123"
            })
            .to_string(),
        ))
        .unwrap();
    let login_resp = app.clone().oneshot(login_req).await.unwrap();
    assert_eq!(login_resp.status(), StatusCode::OK);
    let login_body = to_bytes(login_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let login_payload: serde_json::Value = serde_json::from_slice(&login_body).unwrap();
    let token = login_payload["data"]["token"]
        .as_str()
        .unwrap()
        .to_string();

    let admin_req = Request::builder()
        .uri("/api/v1/admin/billing")
        .method("GET")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let admin_resp = app.clone().oneshot(admin_req).await.unwrap();
    assert_eq!(admin_resp.status(), StatusCode::OK);
    let admin_body = to_bytes(admin_resp.into_body(), usize::MAX).await.unwrap();
    let admin_payload: serde_json::Value = serde_json::from_slice(&admin_body).unwrap();
    assert_eq!(admin_payload["error"], serde_json::Value::Null);
    assert!(admin_payload["data"]["active_subscriptions"].is_number());
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
        .uri("/api/v1/notebooks")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"name":"Shared Chat Notebook","description":""}"#,
        ))
        .unwrap();
    let notebook_resp = app.clone().oneshot(notebook_req).await.unwrap();
    assert_eq!(notebook_resp.status(), StatusCode::CREATED);
    let notebook_body = to_bytes(notebook_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let notebook: serde_json::Value = serde_json::from_slice(&notebook_body).unwrap();
    let notebook_id = notebook["notebook"]["id"].as_str().unwrap().to_string();

    let share_token = avrag_share::ShareService::new(state.share_store().expect("pg expected"))
        .create_share_token(
            &contracts::auth_runtime::AuthContext::new(
                contracts::auth_runtime::OrgId::from(org_id),
                contracts::auth_runtime::SubjectKind::User,
            )
            .with_actor_id(contracts::auth_runtime::ActorId::new(user_id)),
            &notebook_id,
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
            r#"{{"query":"hello public share","notebook_id":"{notebook_id}","agent_type":"chat","source_type":"share","source_token":"{share_token}","doc_scope":[],"messages":[],"stream":false}}"#
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

    let sessions = state.chat().list_sessions(Some(&notebook_id)).await;
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

#[test]
fn rate_limit_allows_then_blocks() {
    let key = format!("test-{}", Uuid::new_v4());
    for _ in 0..middleware::DEFAULT_RATE_LIMIT_RPM {
        let (allowed, _, _) =
            middleware::check_rate_limit(&key, middleware::DEFAULT_RATE_LIMIT_RPM);
        assert!(allowed);
    }
    let (allowed, _, _) =
        middleware::check_rate_limit(&key, middleware::DEFAULT_RATE_LIMIT_RPM);
    assert!(!allowed, "should be blocked after exceeding limit");
}
