//! ADR 0006: `/api/v1/rag/execute-plan` is removed from the router (404).

use app_bootstrap::AppState;
use app_core::AppConfig;
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use tower::ServiceExt;
use transport_http::build_router;
use uuid::Uuid;

#[tokio::test]
async fn post_rag_execute_plan_is_not_routed() {
    let app = build_router(AppState::new(AppConfig::default()));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/rag/execute-plan")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-owner-user-id", Uuid::new_v4().to_string())
                .body(Body::from(
                    serde_json::json!({
                        "plan_version": "rag-execute-v1",
                        "doc_scope": ["00000000-0000-0000-0000-000000000001"],
                        "items": [{ "priority": 1.0, "query": "atlas" }]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "execute-plan route must be physically absent"
    );
}
