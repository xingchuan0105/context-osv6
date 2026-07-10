//! Guardrails HTTP blackbox — input injection is rejected at chat preflight.
//!
//! Product behavior is **fail-closed**: `AppError::validation("input_guard_blocked")`
//! → HTTP 400 (not 500). Matches crate unit cases in `avrag-guardrails`
//! (`'; DROP TABLE users; --`).

use crate::product_e2e::{TestContext, assertions::*};

/// Same injection sample as `guardrails` crate unit tests (`test_sql_injection_blocked`).
const SQL_INJECTION_QUERY: &str = "'; DROP TABLE users; --";

#[tokio::test]
async fn chat_prompt_injection_returns_input_guard_blocked() {
    super::require_smoke_suite();
    let ctx = TestContext::new_smoke().await;
    let notebook = ctx.create_workspace("guardrails-smoke").await.unwrap();

    let http_resp = ctx
        .chat_general(SQL_INJECTION_QUERY, &notebook.id)
        .await
        .unwrap();

    assert!(
        http_resp.status < 500,
        "injection query must not 5xx; got HTTP {}, body={}",
        http_resp.status,
        http_resp.body_json
    );
    assert_http_status(&http_resp, 400);

    let error = http_resp
        .body_json
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert_eq!(
        error, "input_guard_blocked",
        "expected input_guard_blocked error code, body={}",
        http_resp.body_json
    );

    let message = http_resp
        .body_json
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        message.to_ascii_lowercase().contains("blocked by guard")
            || message.to_ascii_lowercase().contains("prompt_injection")
            || message.to_ascii_lowercase().contains("sql_injection"),
        "expected guard reject semantics in message, got: {message}"
    );
}
