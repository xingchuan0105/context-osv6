//! PR-3 (plan §6.2): per-key rate limit 429 boundary at the integration layer.
//!
//! Creates a workspace API key with `rate_limit_rpm: 2`, then fires 3 chat
//! completions against the OpenAI route. The 3rd must come back as HTTP 429
//! with `rate_limit_exceeded`, a `Retry-After` header, and the per-key
//! `x-ratelimit-limit: 2` marker (which distinguishes the per-key 429 from the
//! edge 429).
//!
//! Two harness facts make this non-trivial:
//! * The `request_context_middleware` overrides the per-key limit to 1000 RPM
//!   when `E2E_ENABLED=true`. The product_e2e bootstrap sets that env var, so
//!   the 2 RPM limit would be masked. This test flips `E2E_ENABLED` off for the
//!   3 calls (save/restore) so the key's 2 RPM limit is enforced.
//! * With `E2E_ENABLED` off the edge limit drops to 120 RPM and the edge
//!   counter is shared by client IP. Each run uses a unique `x-forwarded-for`
//!   so the edge counter starts fresh and only the per-key limit can trip.
//!
//! The env toggle is process-global, so this file MUST run under
//! `--test-threads=1` (the canonical integration invocation, plan
//! `G-serial-integration`) — the `RATE_LIMIT_GUARD` mutex only keeps this
//! file's own cases from overlapping the toggle.

use std::sync::Mutex;
use std::time::Duration;

use common::CreateApiKeyRequest;
use serde_json::json;

use crate::product_e2e::TestContext;

static RATE_LIMIT_GUARD: Mutex<()> = Mutex::new(());

/// RAII guard that restores `E2E_ENABLED` on drop (even on panic), so a failed
/// assertion cannot leak the toggle to later tests in the same process.
struct E2eEnabledGuard {
    previous: Option<String>,
}

impl E2eEnabledGuard {
    fn set(value: &str) -> Self {
        let previous = std::env::var("E2E_ENABLED").ok();
        // SAFETY: this file runs under `--test-threads=1` (plan G-serial-integration),
        // so no other product_e2e test reads E2E_ENABLED while the guard is held.
        unsafe {
            std::env::set_var("E2E_ENABLED", value);
        }
        Self { previous }
    }
}

impl Drop for E2eEnabledGuard {
    fn drop(&mut self) {
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var("E2E_ENABLED", value),
                None => std::env::remove_var("E2E_ENABLED"),
            }
        }
    }
}

async fn openai_chat_completion(
    ctx: &TestContext,
    bearer: &str,
    notebook_id: &str,
    edge_ip: &str,
) -> reqwest::Response {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("rate limit reqwest client");
    client
        .post(format!(
            "{}/v1/notebooks/{}/chat/completions",
            ctx.base_url, notebook_id
        ))
        .header("Authorization", format!("Bearer {bearer}"))
        .header("x-forwarded-for", edge_ip)
        .header("Content-Type", "application/json")
        .body(json!({ "query": "hi", "stream": false }).to_string())
        .send()
        .await
        .expect("chat completion send")
}

#[tokio::test]
async fn workspace_key_rate_limit_rpm_2_blocks_third_request_with_429() {
    super::require_integration_suite();
    let _guard = RATE_LIMIT_GUARD.lock().expect("rate limit guard");

    let ctx = TestContext::new_smoke().await;
    // Setup runs while E2E_ENABLED is still "true" (set by the bootstrap), so
    // the proxy-header auth used by `create_notebook` works.
    let notebook = ctx
        .create_notebook("rate-limit")
        .await
        .expect("create notebook");

    let state = ctx
        .app_state
        .as_ref()
        .expect("app_state present in integration profile")
        .clone();
    let key = state
        .create_api_key(
            &notebook.id,
            CreateApiKeyRequest {
                name: "rate-limited".to_string(),
                permissions: vec!["query".to_string()],
                rate_limit_rpm: Some(2),
                expires_at: None,
            },
        )
        .await
        .expect("create rate-limited api key");
    let bearer = key.plaintext_key;

    // Unique edge IP isolates the edge counter so only the per-key 2 RPM limit
    // can produce the 429 (not the shared edge limit).
    let edge_ip = format!("rate-limit-{}", uuid::Uuid::new_v4());

    // Flip E2E_ENABLED off so the per-key 2 RPM limit is enforced (the
    // middleware otherwise overrides it to 1000 RPM). Restored on drop, even
    // if an assertion below panics.
    let _e2e_guard = E2eEnabledGuard::set("false");
    let r1 = openai_chat_completion(&ctx, &bearer, &notebook.id, &edge_ip).await;
    let r2 = openai_chat_completion(&ctx, &bearer, &notebook.id, &edge_ip).await;
    let r3 = openai_chat_completion(&ctx, &bearer, &notebook.id, &edge_ip).await;

    let s1 = r1.status().as_u16();
    let s2 = r2.status().as_u16();
    let s3 = r3.status().as_u16();
    assert_ne!(s1, 429, "first request must not be rate-limited (got {s1})");
    assert_ne!(
        s2, 429,
        "second request must not be rate-limited (got {s2})"
    );
    assert_eq!(s3, 429, "third request must be rate-limited");

    let retry_after = r3
        .headers()
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    assert!(
        retry_after.is_some(),
        "429 must carry a Retry-After header, headers={:?}",
        r3.headers(),
    );

    let limit = r3
        .headers()
        .get("x-ratelimit-limit")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    assert_eq!(
        limit.as_deref(),
        Some("2"),
        "429 must be the per-key 2 RPM limit (not the edge limit), headers={:?}",
        r3.headers(),
    );

    let body: serde_json::Value = r3.json().await.unwrap_or(serde_json::Value::Null);
    assert_eq!(
        body.get("error").and_then(|value| value.as_str()),
        Some("rate_limit_exceeded"),
        "429 body must carry rate_limit_exceeded, body={body}",
    );
}
