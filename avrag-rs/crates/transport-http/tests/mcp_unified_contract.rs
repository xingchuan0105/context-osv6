use app_bootstrap::AppState;
use app_core::AppConfig;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use common::{CreateApiKeyRequest, CreateDocumentRequest, CreateNotebookRequest};
use contracts::agent_permissions::PERM_ADMIN;
use contracts::documents::DocumentStatus;
use tower::ServiceExt;
use transport_http::build_router;
use uuid::Uuid;

fn test_app_state() -> AppState {
    AppState::new(AppConfig::default())
}

fn admin_app_state() -> AppState {
    let state = test_app_state();
    state.with_auth(state.auth().clone().grant(PERM_ADMIN))
}

fn mcp_app_error_code(payload: &serde_json::Value) -> Option<&str> {
    payload
        .pointer("/error/data/error")
        .and_then(|value| value.as_str())
        .or_else(|| payload.get("error").and_then(|value| value.as_str()))
}

fn mcp_guide_mode(payload: &serde_json::Value) -> Option<&str> {
    payload
        .pointer("/error/data/agent_operation_guide/mode")
        .and_then(|value| value.as_str())
        .or_else(|| {
            payload
                .pointer("/agent_operation_guide/mode")
                .and_then(|value| value.as_str())
        })
}

async fn create_workspace_with_key(
    permissions: Vec<String>,
) -> (AppState, String, String, axum::Router) {
    let state = test_app_state();
    let notebook = state
        .create_notebook(CreateNotebookRequest {
            name: "unified-contract".to_string(),
            description: String::new(),
        })
        .await
        .expect("notebook should create");
    let key = state
        .create_api_key(
            &notebook.id,
            CreateApiKeyRequest {
                name: "agent".to_string(),
                permissions,
                rate_limit_rpm: Some(60),
                expires_at: None,
            },
        )
        .await
        .expect("api key should create");
    let app = build_router(state.clone());
    (state, notebook.id, key.plaintext_key, app)
}

async fn mcp_tools_call(
    app: &axum::Router,
    bearer: &str,
    tool_name: &str,
    arguments: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/mcp")
                .header(header::CONTENT_TYPE, "application/json")
                .header("Authorization", format!("Bearer {bearer}"))
                .body(Body::from(
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": "1",
                        "method": "tools/call",
                        "params": {
                            "name": tool_name,
                            "arguments": arguments
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload =
        serde_json::from_slice::<serde_json::Value>(&body).unwrap_or(serde_json::Value::Null);
    (status, payload)
}

#[tokio::test]
async fn workspace_api_key_defaults_include_index_and_query() {
    let state = test_app_state();
    let notebook = state
        .create_notebook(CreateNotebookRequest {
            name: "defaults".to_string(),
            description: String::new(),
        })
        .await
        .unwrap();
    let key = state
        .create_api_key(
            &notebook.id,
            CreateApiKeyRequest {
                name: "default-perms".to_string(),
                permissions: vec![],
                rate_limit_rpm: Some(60),
                expires_at: None,
            },
        )
        .await
        .unwrap();

    assert!(key.api_key.permissions.iter().any(|p| p == "index"));
    assert!(key.api_key.permissions.iter().any(|p| p == "query"));
}

#[tokio::test]
async fn org_api_key_can_create_workspace_via_mcp() {
    let state = admin_app_state();
    let org_key = state
        .create_org_api_key(CreateApiKeyRequest {
            name: "org-agent".to_string(),
            permissions: vec![],
            rate_limit_rpm: Some(60),
            expires_at: None,
        })
        .await
        .expect("org key should create");
    let app = build_router(state);

    let (status, payload) = mcp_tools_call(
        &app,
        &org_key.plaintext_key,
        "org.create_workspace",
        serde_json::json!({
            "name": "Org Bearer Workspace",
            "description": "via org key"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        payload
            .pointer("/result/structuredContent/data/notebook/id")
            .and_then(|value| value.as_str())
            .is_some()
    );
}

#[tokio::test]
async fn workspace_key_cannot_call_org_mcp_tool() {
    let (_state, _notebook_id, bearer, app) =
        create_workspace_with_key(vec!["index".to_string(), "query".to_string()]).await;

    let (status, payload) = mcp_tools_call(
        &app,
        &bearer,
        "org.create_workspace",
        serde_json::json!({
            "name": "blocked",
            "description": ""
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        mcp_app_error_code(&payload),
        Some("workspace_key_cannot_call_org_tools")
    );
    assert_eq!(mcp_guide_mode(&payload), Some("workspace.create"));
}

#[tokio::test]
async fn workspace_scope_mismatch_mcp_rag_query() {
    let (_state, _notebook_id, bearer, app) =
        create_workspace_with_key(vec!["query".to_string()]).await;
    let other_notebook = Uuid::new_v4().to_string();

    let (status, payload) = mcp_tools_call(
        &app,
        &bearer,
        "workspace.rag_query",
        serde_json::json!({
            "notebook_id": other_notebook,
            "query": "hello"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        mcp_app_error_code(&payload),
        Some("notebook_scope_mismatch")
    );
    assert_eq!(mcp_guide_mode(&payload), Some("rag"));
}

#[tokio::test]
async fn workspace_query_only_key_cannot_mcp_create_upload() {
    let (_state, notebook_id, bearer, app) =
        create_workspace_with_key(vec!["query".to_string()]).await;

    let (status, payload) = mcp_tools_call(
        &app,
        &bearer,
        "workspace.create_upload",
        serde_json::json!({
            "notebook_id": notebook_id,
            "filename": "notes.txt",
            "mime_type": "text/plain",
            "file_size": 12
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(mcp_app_error_code(&payload), Some("missing_permission"));
    assert_eq!(mcp_guide_mode(&payload), Some("index"));
}

#[tokio::test]
async fn mcp_complete_upload_rejects_document_from_other_workspace() {
    let state = test_app_state();
    let notebook_a = state
        .create_notebook(CreateNotebookRequest {
            name: "workspace-a".to_string(),
            description: String::new(),
        })
        .await
        .unwrap();
    let notebook_b = state
        .create_notebook(CreateNotebookRequest {
            name: "workspace-b".to_string(),
            description: String::new(),
        })
        .await
        .unwrap();
    let key = state
        .create_api_key(
            &notebook_a.id,
            CreateApiKeyRequest {
                name: "scoped-a".to_string(),
                permissions: vec!["index".to_string(), "query".to_string()],
                rate_limit_rpm: Some(60),
                expires_at: None,
            },
        )
        .await
        .unwrap();
    let upload_b = state
        .create_document_upload(
            &notebook_b.id,
            CreateDocumentRequest {
                filename: "other.txt".to_string(),
                mime_type: "text/plain".to_string(),
                file_size: 12,
            },
        )
        .await
        .unwrap();
    let app = build_router(state);

    let (status, payload) = mcp_tools_call(
        &app,
        &key.plaintext_key,
        "workspace.complete_upload",
        serde_json::json!({
            "notebook_id": notebook_a.id,
            "document_id": upload_b.document_id
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        mcp_app_error_code(&payload),
        Some("document_notebook_mismatch")
    );
}

#[tokio::test]
async fn workspace_key_cannot_rag_other_workspace_doc_scope() {
    let state = test_app_state();
    let notebook_a = state
        .create_notebook(CreateNotebookRequest {
            name: "scope-a".to_string(),
            description: String::new(),
        })
        .await
        .unwrap();
    let notebook_b = state
        .create_notebook(CreateNotebookRequest {
            name: "scope-b".to_string(),
            description: String::new(),
        })
        .await
        .unwrap();
    let upload_b = state
        .create_document_upload(
            &notebook_b.id,
            CreateDocumentRequest {
                filename: "secret.txt".to_string(),
                mime_type: "text/plain".to_string(),
                file_size: 12,
            },
        )
        .await
        .unwrap();
    state
        .put_uploaded_document(&upload_b.document_id, b"secret content".to_vec())
        .await
        .unwrap();
    state
        .transition_document_status(&upload_b.document_id, DocumentStatus::Completed)
        .await
        .unwrap();
    let key = state
        .create_api_key(
            &notebook_a.id,
            CreateApiKeyRequest {
                name: "scope-a-key".to_string(),
                permissions: vec!["query".to_string()],
                rate_limit_rpm: Some(60),
                expires_at: None,
            },
        )
        .await
        .unwrap();
    let app = build_router(state);

    let (status, payload) = mcp_tools_call(
        &app,
        &key.plaintext_key,
        "workspace.rag_query",
        serde_json::json!({
            "notebook_id": notebook_a.id,
            "query": "secret",
            "doc_scope": [upload_b.document_id]
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(mcp_app_error_code(&payload), Some("invalid_document_scope"));
}

#[tokio::test]
async fn rest_create_notebook_forbidden_for_workspace_key() {
    let (_state, _notebook_id, bearer, app) =
        create_workspace_with_key(vec!["index".to_string(), "query".to_string()]).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notebooks")
                .header(header::CONTENT_TYPE, "application/json")
                .header("Authorization", format!("Bearer {bearer}"))
                .body(Body::from(
                    serde_json::json!({
                        "name": "blocked",
                        "description": ""
                    })
                    .to_string(),
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
        Some("workspace_key_cannot_call_org_tools")
    );
}

#[tokio::test]
async fn rest_upload_scope_mismatch_returns_forbidden() {
    let (_state, _notebook_id, bearer, app) =
        create_workspace_with_key(vec!["index".to_string(), "query".to_string()]).await;
    let other_notebook = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/notebooks/{other_notebook}/documents"))
                .header(header::CONTENT_TYPE, "application/json")
                .header("Authorization", format!("Bearer {bearer}"))
                .body(Body::from(
                    serde_json::json!({
                        "filename": "notes.txt",
                        "mime_type": "text/plain",
                        "file_size": 12
                    })
                    .to_string(),
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
        Some("notebook_scope_mismatch")
    );
}

#[tokio::test]
async fn mcp_ingestion_flow_create_upload_complete_status() {
    let (state, notebook_id, bearer, app) =
        create_workspace_with_key(vec!["index".to_string(), "query".to_string()]).await;

    let (status, payload) = mcp_tools_call(
        &app,
        &bearer,
        "workspace.create_upload",
        serde_json::json!({
            "notebook_id": notebook_id,
            "filename": "flow.txt",
            "mime_type": "text/plain",
            "file_size": 24
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let document_id = payload
        .pointer("/result/structuredContent/data/document_id")
        .and_then(|value| value.as_str())
        .expect("document_id should be returned")
        .to_string();
    assert_eq!(
        payload
            .pointer("/result/structuredContent/agent_operation_guide/mode")
            .and_then(|value| value.as_str()),
        Some("index")
    );

    state
        .put_uploaded_document(&document_id, b"ingestion flow contract body".to_vec())
        .await
        .expect("upload bytes");

    let (complete_status, complete_payload) = mcp_tools_call(
        &app,
        &bearer,
        "workspace.complete_upload",
        serde_json::json!({
            "notebook_id": notebook_id,
            "document_id": document_id
        }),
    )
    .await;
    assert_eq!(complete_status, StatusCode::OK);
    assert_eq!(
        complete_payload
            .pointer("/result/structuredContent/ok")
            .and_then(|value| value.as_bool()),
        Some(true)
    );

    state
        .transition_document_status(&document_id, DocumentStatus::Completed)
        .await
        .expect("mark completed");

    let (status_status, status_payload) = mcp_tools_call(
        &app,
        &bearer,
        "workspace.document_status",
        serde_json::json!({
            "notebook_id": notebook_id,
            "document_id": document_id
        }),
    )
    .await;
    assert_eq!(status_status, StatusCode::OK);
    assert_eq!(
        status_payload
            .pointer("/result/structuredContent/data/status")
            .and_then(|value| value.as_str()),
        Some("completed")
    );
}
