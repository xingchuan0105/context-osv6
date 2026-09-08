use app_bootstrap::AppState;
use app_core::AppConfig;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use transport_http::{build_router, issue_jwt};
use uuid::Uuid;

fn upload(path: &str, token: Option<&str>, bytes: Vec<u8>) -> Request<Body> {
    let mut request = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/octet-stream");
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    request.body(Body::from(bytes)).unwrap()
}

#[tokio::test]
async fn anonymous_attachment_upload_is_rejected() {
    let response = build_router(AppState::new(AppConfig::default()))
        .oneshot(upload(
            "/api/v1/chat/attachments/parse?filename=note.txt",
            None,
            b"notes".to_vec(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn authenticated_attachment_upload_validates_filename_before_parsing() {
    let user = Uuid::new_v4();
    let token = issue_jwt(&user, &user);
    let response = build_router(AppState::new(AppConfig::default()))
        .oneshot(upload(
            "/api/v1/chat/attachments/parse?filename=..%2Fnotes.txt",
            Some(&token),
            b"notes".to_vec(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("attachment_filename_invalid"));
}

#[tokio::test]
async fn oversized_attachment_body_is_rejected() {
    let user = Uuid::new_v4();
    let token = issue_jwt(&user, &user);
    let response = build_router(AppState::new(AppConfig::default()))
        .oneshot(upload(
            "/api/v1/chat/attachments/parse?filename=note.txt",
            Some(&token),
            vec![b'x'; contracts::chat::MAX_TURN_ATTACHMENT_BYTES + 1],
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
#[ignore = "requires the configured anydoc executable"]
async fn authenticated_excel_upload_returns_direct_context() {
    let user = Uuid::new_v4();
    let token = issue_jwt(&user, &user);
    let response = build_router(AppState::new(AppConfig::default()))
        .oneshot(upload(
            "/api/v1/chat/attachments/parse?filename=smoke.xlsx",
            Some(&token),
            include_bytes!("../../ingestion/tests/fixtures/smoke.xlsx").to_vec(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(
        response.into_body(),
        contracts::chat::MAX_TURN_CONTEXT_BYTES,
    )
    .await
    .unwrap();
    let attachment: contracts::chat::TurnAttachment = serde_json::from_slice(&body).unwrap();
    assert_eq!(attachment.filename, "smoke.xlsx");
    assert!(attachment.text.contains("10.5"));
}
