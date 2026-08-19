use super::support::*;
use avrag_retrieval_data_plane::{EXPORT_SCHEMA_VERSION, PublishFingerprint};

fn default_fingerprint() -> PublishFingerprint {
    PublishFingerprint::new("text-embedding-v4", 1024)
}

fn session_body(fingerprint: PublishFingerprint, local_workspace_id: Uuid) -> String {
    serde_json::json!({
        "local_workspace_id": local_workspace_id,
        "title": "Desktop lib",
        "fingerprint": fingerprint,
        "document_ids": [Uuid::from_u128(9)],
    })
    .to_string()
}

#[tokio::test]
async fn publish_session_rejects_api_key() {
    let state = test_app_state();
    let notebook = state
        .workspace()
        .create_workspace(CreateWorkspaceRequest {
            name: "Publish Key".to_string(),
            description: String::new(),
        })
        .await
        .expect("workspace");
    let key = state
        .admin_api()
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
        .expect("api key");
    let app = build_router(state);
    let req = Request::builder()
        .uri("/api/v1/workspaces/publish/sessions")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", key.plaintext_key))
        .body(Body::from(session_body(
            default_fingerprint(),
            Uuid::from_u128(11),
        )))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn publish_session_rejects_fingerprint_mismatch() {
    let state = test_app_state();
    let app = build_router(state);
    let mut fingerprint = default_fingerprint();
    fingerprint.vector_dim = 8;
    fingerprint.schema_version = EXPORT_SCHEMA_VERSION.to_string();
    let req = Request::builder()
        .uri("/api/v1/workspaces/publish/sessions")
        .method("POST")
        .header("Content-Type", "application/json")
        .header(middleware::HEADER_OWNER_USER_ID, "00000000-0000-0000-0000-000000000001")
        .header(middleware::HEADER_USER_ID, "00000000-0000-0000-0000-000000000002")
        .body(Body::from(session_body(fingerprint, Uuid::from_u128(12))))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["error"], "publish_fingerprint_mismatch");
}

#[tokio::test]
async fn publish_session_accepts_matching_fingerprint() {
    let state = test_app_state();
    let app = build_router(state);
    let req = Request::builder()
        .uri("/api/v1/workspaces/publish/sessions")
        .method("POST")
        .header("Content-Type", "application/json")
        .header(middleware::HEADER_OWNER_USER_ID, "00000000-0000-0000-0000-000000000001")
        .header(middleware::HEADER_USER_ID, "00000000-0000-0000-0000-000000000002")
        .body(Body::from(session_body(
            default_fingerprint(),
            Uuid::from_u128(13),
        )))
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(payload["upload_id"].as_str().is_some());
    assert!(payload["cloud_workspace_id"].as_str().is_some());
}

#[tokio::test]
async fn publish_commit_without_data_plane_is_unavailable() {
    let state = test_app_state();
    let app = build_router(state.clone());
    let create = Request::builder()
        .uri("/api/v1/workspaces/publish/sessions")
        .method("POST")
        .header("Content-Type", "application/json")
        .header(middleware::HEADER_OWNER_USER_ID, "00000000-0000-0000-0000-000000000001")
        .header(middleware::HEADER_USER_ID, "00000000-0000-0000-0000-000000000002")
        .body(Body::from(session_body(
            default_fingerprint(),
            Uuid::from_u128(14),
        )))
        .unwrap();
    let create_resp = app.clone().oneshot(create).await.unwrap();
    assert_eq!(create_resp.status(), StatusCode::OK);
    let create_body = to_bytes(create_resp.into_body(), usize::MAX).await.unwrap();
    let created: serde_json::Value = serde_json::from_slice(&create_body).unwrap();
    let upload_id = created["upload_id"].as_str().unwrap();

    let commit = Request::builder()
        .uri(format!(
            "/api/v1/workspaces/publish/sessions/{upload_id}/commit"
        ))
        .method("POST")
        .header(middleware::HEADER_OWNER_USER_ID, "00000000-0000-0000-0000-000000000001")
        .header(middleware::HEADER_USER_ID, "00000000-0000-0000-0000-000000000002")
        .body(Body::empty())
        .unwrap();
    let commit_resp = app.clone().oneshot(commit).await.unwrap();
    assert_eq!(commit_resp.status(), StatusCode::SERVICE_UNAVAILABLE);

    let status = Request::builder()
        .uri(format!(
            "/api/v1/workspaces/publish/status?local_workspace_id={}",
            Uuid::from_u128(14)
        ))
        .method("GET")
        .header(middleware::HEADER_OWNER_USER_ID, "00000000-0000-0000-0000-000000000001")
        .header(middleware::HEADER_USER_ID, "00000000-0000-0000-0000-000000000002")
        .body(Body::empty())
        .unwrap();
    let status_resp = app.oneshot(status).await.unwrap();
    assert_eq!(status_resp.status(), StatusCode::OK);
    let status_body = to_bytes(status_resp.into_body(), usize::MAX).await.unwrap();
    let status_payload: serde_json::Value = serde_json::from_slice(&status_body).unwrap();
    assert_eq!(status_payload["status"], "failed");
}

fn sample_part_json(fingerprint: PublishFingerprint, document_id: Uuid) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "document_id": document_id,
        "filename": "a.md",
        "mime_type": "text/markdown",
        "status": "completed",
        "summary": null,
        "chunk_count": 0,
        "export": {
            "manifest": {
                "fingerprint": fingerprint,
                "owner_user_id": "00000000-0000-0000-0000-000000000002",
                "workspace_id": null,
                "document_id": document_id,
                "parse_run_id": Uuid::nil(),
                "doc_version": 1,
                "text_chunk_count": 0,
                "multimodal_chunk_count": 0,
                "entity_count": 0,
                "relation_count": 0,
                "graph_passage_count": 0
            },
            "batch": {
                "owner_user_id": "00000000-0000-0000-0000-000000000002",
                "workspace_id": null,
                "document_id": document_id,
                "parse_run_id": Uuid::nil(),
                "doc_version": 1,
                "text_chunks": [],
                "multimodal_chunks": [],
                "entities": [],
                "relations": [],
                "graph_passages": []
            }
        }
    }))
    .expect("part json")
}

#[tokio::test]
async fn publish_part_accepts_zstd_content_encoding() {
    let state = test_app_state();
    let app = build_router(state);
    let local = Uuid::from_u128(15);
    let create = Request::builder()
        .uri("/api/v1/workspaces/publish/sessions")
        .method("POST")
        .header("Content-Type", "application/json")
        .header(middleware::HEADER_OWNER_USER_ID, "00000000-0000-0000-0000-000000000001")
        .header(middleware::HEADER_USER_ID, "00000000-0000-0000-0000-000000000002")
        .body(Body::from(session_body(default_fingerprint(), local)))
        .unwrap();
    let create_resp = app.clone().oneshot(create).await.unwrap();
    assert_eq!(create_resp.status(), StatusCode::OK);
    let create_body = to_bytes(create_resp.into_body(), usize::MAX).await.unwrap();
    let created: serde_json::Value = serde_json::from_slice(&create_body).unwrap();
    let upload_id = created["upload_id"].as_str().unwrap();

    let json = sample_part_json(default_fingerprint(), Uuid::from_u128(9));
    let compressed = zstd::encode_all(&json[..], 3).expect("zstd");
    let put = Request::builder()
        .uri(format!(
            "/api/v1/workspaces/publish/sessions/{upload_id}/parts/0"
        ))
        .method("PUT")
        .header("Content-Type", "application/json")
        .header("Content-Encoding", "zstd")
        .header(middleware::HEADER_OWNER_USER_ID, "00000000-0000-0000-0000-000000000001")
        .header(middleware::HEADER_USER_ID, "00000000-0000-0000-0000-000000000002")
        .body(Body::from(compressed))
        .unwrap();
    let put_resp = app.oneshot(put).await.unwrap();
    assert_eq!(put_resp.status(), StatusCode::NO_CONTENT);
}
