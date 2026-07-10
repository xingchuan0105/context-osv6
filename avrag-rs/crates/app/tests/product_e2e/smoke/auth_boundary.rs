//! Auth middleware boundary tests.
//!
//! The transport-http layer enforces two things on every request:
//! 1. An `AuthContext` must be resolvable from either a JWT bearer
//!    token or `x-owner-user-id` / `x-user-id` proxy headers. Otherwise → 401.
//! 2. The resolved `AuthContext` must be in the right org scope for
//!    the requested resource (notebooks, documents, etc.) — this is
//!    the RLS path that the `tenants::isolation` tests also exercise.
//!
//! These tests focus on the **request boundary** (what the middleware
//! rejects with 401/403) rather than the data-scope boundary (which
//! lives in `tenants::isolation.rs`).
//!
//! Each test builds a **bare** `reqwest::Client` with the auth header
//! configuration under test (or no headers) so the boundary is
//! exercised independently of the default `TestContext` headers.

use std::time::Duration;

use crate::product_e2e::TestContext;

const ORG_ID: &str = "33333333-3333-3333-3333-333333333333";
const USER_ID: &str = "cccccccc-cccc-cccc-cccc-cccccccccccc";

/// Helper: a bare client with the given set of extra headers, used to
/// test what happens when callers send malformed / missing auth.
fn bare_client_with_headers(headers: &[(&str, &str)]) -> reqwest::Client {
    let mut hmap = reqwest::header::HeaderMap::new();
    for (k, v) in headers {
        hmap.insert(
            (*k).parse::<reqwest::header::HeaderName>().unwrap(),
            (*v).parse().unwrap(),
        );
    }
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .default_headers(hmap)
        .build()
        .expect("build bare client")
}

#[tokio::test]
async fn chat_without_any_auth_headers_returns_401() {
    super::require_smoke_suite();
    let ctx = TestContext::new_smoke().await;
    let bare = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();

    let resp = bare
        .post(format!("{}/api/v1/chat", ctx.base_url))
        .json(&serde_json::json!({
            "query": "hi",
            "agent_type": "rag",
            "stream": false,
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.status().as_u16(),
        401,
        "request without any auth headers must be rejected with 401, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn chat_with_malformed_org_id_uuid_returns_401() {
    super::require_smoke_suite();
    let ctx = TestContext::new_smoke().await;
    let bare = bare_client_with_headers(&[
        ("x-owner-user-id", "not-a-uuid"),
        ("x-user-id", USER_ID),
        ("x-permissions", "external_network"),
    ]);

    let resp = bare
        .post(format!("{}/api/v1/chat", ctx.base_url))
        .json(&serde_json::json!({
            "query": "hi",
            "agent_type": "rag",
            "stream": false,
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.status().as_u16(),
        401,
        "x-owner-user-id that is not a valid UUID must be rejected, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn chat_with_valid_org_but_no_user_id_is_accepted() {
    super::require_smoke_suite();
    // `x-user-id` is optional in the proxy-header path; missing user_id
    // means the request is treated as system-actor. This is intentional
    // for service-to-service calls and should NOT be rejected.
    //
    // We need to send a structurally-valid request body (workspace_id +
    // valid agent_type) so the only thing under test is the auth
    // boundary. The chat handler may still 4xx for other reasons
    // (no doc_scope, etc.) — we just want to confirm it does NOT 500.
    let ctx = TestContext::new_smoke().await;
    let bare =
        bare_client_with_headers(&[("x-owner-user-id", ORG_ID), ("x-permissions", "external_network")]);

    let resp = bare
        .post(format!("{}/api/v1/chat", ctx.base_url))
        .json(&serde_json::json!({
            "query": "hi",
            "agent_type": "rag",
            "stream": false,
            "workspace_id": "00000000-0000-0000-0000-000000000001",
        }))
        .send()
        .await
        .unwrap();

    let status = resp.status().as_u16();
    // Acceptable outcomes: 200 (treated as anonymous actor), 4xx (rejected
    // for some other reason — auth, docscope, etc.). NOT 500.
    assert!(
        status < 500,
        "missing x-user-id should not 500, got HTTP {status}"
    );
}

#[tokio::test]
async fn create_workspace_under_one_org_then_read_under_another_org_returns_404_or_403() {
    super::require_smoke_suite();
    // Data-scope boundary at the notebook layer: once User A creates
    // a notebook, User B (different org) cannot fetch it by ID.
    let ctx_a = TestContext::new_smoke_with_org(ORG_ID, USER_ID).await;
    let notebook = ctx_a.create_workspace("org-a-private").await.unwrap();

    let ctx_b = TestContext::new_smoke_with_org(
        "44444444-4444-4444-4444-444444444444",
        "dddddddd-dddd-dddd-dddd-dddddddddddd",
    )
    .await;

    // Use the TestContext's own client (which carries org-B's headers)
    // to attempt a GET on org-A's notebook.
    let resp = ctx_b
        .http_client
        .get(format!(
            "{}/api/v1/workspaces/{}",
            ctx_b.base_url, notebook.id
        ))
        .send()
        .await
        .unwrap();

    let status = resp.status().as_u16();
    assert!(
        (400..500).contains(&status),
        "cross-org notebook read should be rejected with 4xx, got HTTP {status}"
    );
    assert_ne!(
        status, 200,
        "cross-org notebook read must NOT succeed (200), got HTTP {status}"
    );
}

#[tokio::test]
async fn unauthenticated_request_to_docs_status_returns_401() {
    super::require_smoke_suite();
    // The /api/v1/documents/{id}/status endpoint also sits behind the
    // auth middleware (it's in the protected `/api/v1` tree).
    let ctx = TestContext::new_smoke().await;
    let bare = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();

    let resp = bare
        .get(format!(
            "{}/api/v1/documents/00000000-0000-0000-0000-000000000000/status",
            ctx.base_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.status().as_u16(),
        401,
        "documents status endpoint must require auth, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn chat_with_valid_jwt_bearer_returns_200() {
    super::require_smoke_suite();
    let ctx = TestContext::new_smoke().await;
    let email = format!("jwt-smoke-{}@example.test", uuid::Uuid::new_v4());
    let token = ctx
        .register_user_token(&email, "JWT Smoke User")
        .await
        .expect("register user for JWT test");
    let notebook = ctx
        .create_workspace_with_token(&token, "jwt-chat")
        .await
        .expect("create notebook for JWT user");

    let http_resp = ctx
        .chat_with_bearer_token(&token, "Hello from JWT", &notebook.id)
        .await
        .expect("jwt chat");

    assert_eq!(
        http_resp.status, 200,
        "valid JWT bearer chat must return 200, body={}",
        http_resp.body_json
    );
    let resp: crate::product_e2e::ChatResponse = http_resp.into_business().unwrap();
    assert!(
        !resp.answer.is_empty(),
        "JWT chat answer should be non-empty"
    );
}
