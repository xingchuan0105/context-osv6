use app::{AppConfig, AppState};
use axum::{
    body::{Body, to_bytes},
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

#[tokio::test]
async fn admin_routes_reject_org_only_proxy_auth_before_repo_access() {
    let app = transport_http::build_router(AppState::new(AppConfig::default()));
    let paths = ["/api/v1/admin/billing", "/api/v1/admin/feature-flags"];

    for path in paths {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(path)
                    .header("x-org-id", "11111111-1111-1111-1111-111111111111")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            payload["error"]["code"].as_str(),
            Some("authenticated_user_required"),
            "{path}"
        );
    }
}
