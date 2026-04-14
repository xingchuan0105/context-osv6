use app::{AppConfig, AppState};
use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use tower::ServiceExt;

#[tokio::test]
async fn router_exposes_only_post_chat_contract() {
    let app = transport_http::build_router(AppState::new(AppConfig::default()));

    let post_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/v1/chat")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"query":"ping","agent_type":"general","stream":false}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(post_response.status(), StatusCode::METHOD_NOT_ALLOWED);

    let get_response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v1/chat")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::METHOD_NOT_ALLOWED);
}
