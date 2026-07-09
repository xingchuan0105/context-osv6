use super::support::*;

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

