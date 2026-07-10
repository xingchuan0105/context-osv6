use super::support::*;

#[tokio::test]
async fn workspace_api_key_can_access_its_workspace_sources() {
    let state = test_app_state();
    let notebook = state.workspace()
        .create_workspace(CreateWorkspaceRequest {
            name: "API Key Workspace".to_string(),
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
        .uri(format!("/api/v1/sources?workspace_id={}", notebook.id))
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
        .uri("/api/v1/workspaces")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"name":"Routes Workspace","description":""}"#))
        .unwrap();
    let notebook_resp = app.clone().oneshot(notebook_req).await.unwrap();
    assert_eq!(notebook_resp.status(), StatusCode::CREATED);
    let notebook_body = to_bytes(notebook_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let notebook_payload: serde_json::Value = serde_json::from_slice(&notebook_body).unwrap();
    let workspace_id = notebook_payload["workspace"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let create_doc_req = Request::builder()
        .uri(format!("/api/v1/workspaces/{workspace_id}/documents"))
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
        .uri(format!("/api/v1/workspaces/{workspace_id}/share"))
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
        .uri(format!("/api/v1/workspaces/{workspace_id}/share/settings"))
        .method("GET")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let settings_resp = app.clone().oneshot(settings_req).await.unwrap();
    assert_eq!(settings_resp.status(), StatusCode::OK);

    let analytics_req = Request::builder()
        .uri(format!("/api/v1/workspaces/{workspace_id}/share/analytics"))
        .method("GET")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let analytics_resp = app.clone().oneshot(analytics_req).await.unwrap();
    assert_eq!(analytics_resp.status(), StatusCode::OK);

    let logs_req = Request::builder()
        .uri(format!("/api/v1/workspaces/{workspace_id}/share/access-logs"))
        .method("GET")
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let logs_resp = app.oneshot(logs_req).await.unwrap();
    assert_eq!(logs_resp.status(), StatusCode::OK);
}


#[tokio::test]
async fn signed_upload_handler_accepts_valid_signed_url() {
    let state = test_app_state();
    let notebook = state.workspace()
        .create_workspace(CreateWorkspaceRequest {
            name: "Upload Test".to_string(),
            description: String::new(),
        })
        .await
        .expect("notebook should create");
    let created = state.workspace()
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
    let notebook = state.workspace()
        .create_workspace(CreateWorkspaceRequest {
            name: "Unsupported Upload Test".to_string(),
            description: String::new(),
        })
        .await
        .expect("notebook should create");
    let org_id = "00000000-0000-0000-0000-000000000001";
    let user_id = "00000000-0000-0000-0000-000000000002";

    let app = build_router(state);
    let req = Request::builder()
        .uri(format!("/api/v1/workspaces/{}/documents", notebook.id))
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
    let notebook = state.workspace()
        .create_workspace(CreateWorkspaceRequest {
            name: "Dev Upload Test".to_string(),
            description: String::new(),
        })
        .await
        .expect("notebook should create");
    let created = state.workspace()
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
async fn shared_workspace_handler_is_not_implemented_no_longer_returns_501() {
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

