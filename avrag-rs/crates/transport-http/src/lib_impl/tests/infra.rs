use super::support::*;

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
        .uri("/api/v1/workspaces")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
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
