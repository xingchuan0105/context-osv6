use app::{AppState};
use app_core::AppConfig;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use tower::ServiceExt;
use transport_http::build_router;
use uuid::Uuid;

#[tokio::test]
async fn post_runtime_execute_requires_auth_context() {
    let app = build_router(AppState::new(AppConfig::default()));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/runtime/execute")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "calls": [
                            { "tool": "dense_retrieval", "version": "1.0", "args": { "queries": ["hello"] } }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn post_runtime_execute_rejects_empty_calls() {
    let app = build_router(AppState::new(AppConfig::default()));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/runtime/execute")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-org-id", Uuid::new_v4().to_string())
                .body(Body::from(serde_json::json!({ "calls": [] }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    assert_eq!(
        payload.get("error").and_then(|value| value.as_str()),
        Some("invalid_calls")
    );
}

#[tokio::test]
async fn post_runtime_execute_fails_closed_without_runtime() {
    let app = build_router(AppState::new(AppConfig::default()));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/runtime/execute")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-org-id", Uuid::new_v4().to_string())
                .body(Body::from(
                    serde_json::json!({
                        "calls": [
                            { "tool": "dense_retrieval", "version": "1.0", "args": { "queries": ["hello"] } }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload = serde_json::from_slice::<serde_json::Value>(&body).unwrap();
    assert_eq!(
        payload.get("error").and_then(|value| value.as_str()),
        Some("rag_runtime_not_configured")
    );
}

#[tokio::test]
async fn get_runtime_execute_is_not_allowed() {
    let app = build_router(AppState::new(AppConfig::default()));
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/runtime/execute")
                .header("x-org-id", Uuid::new_v4().to_string())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}
