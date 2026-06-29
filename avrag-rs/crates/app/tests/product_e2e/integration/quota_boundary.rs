//! PR-3 (plan §6.3): quota exhaustion 429 boundary at the integration layer.
//!
//! Registers a real user, seeds `llm_usage_events` with usage_units that exceed
//! the rolling 5h limit, then sends a chat. The chat flow's first gate is
//! `ensure_metric_quota` (`chat/service.rs`), which calls
//! `QuotaManager::check_quota`; the rolling 5h window is blocked, so the
//! request must come back as HTTP 429 with `quota_exceeded` — distinct from the
//! per-key rate-limit `rate_limit_exceeded` covered by `rate_limit_boundary`.
//!
//! Real PG (so the `QuotaManager` is wired) + mock LLM. The quota check runs
//! before the agent, so no Milvus/RAG is needed (`new_smoke`).

use uuid::Uuid;

use crate::product_e2e::TestContext;

#[tokio::test]
async fn exhausted_quota_blocks_chat_with_quota_exceeded() {
    super::require_integration_suite();
    let ctx = TestContext::new_smoke().await;

    let email = format!("quota-boundary-{}@example.test", Uuid::new_v4());
    let token = ctx
        .register_user_token(&email, "Quota Boundary")
        .await
        .expect("register user");

    // Resolve the new user's id + org_id so we can seed usage on their behalf.
    let pool = sqlx::PgPool::connect(&ctx.pg_url)
        .await
        .expect("connect test pg");
    let (user_id, org_id): (Uuid, Uuid) =
        sqlx::query_as("SELECT id, org_id FROM users WHERE email = $1")
            .bind(&email)
            .fetch_one(&pool)
            .await
            .expect("fetch registered user");

    // Seed enough usage_units to exhaust the rolling 5h window regardless of
    // the free plan's exact limit (compute_windows blocks when used >= limit).
    sqlx::query(
        r#"
        INSERT INTO llm_usage_events (
            org_id, user_id, feature, stage, provider, model,
            prompt_tokens, completion_tokens, total_tokens,
            usage_units, usage_source
        ) VALUES ($1, $2, 'chat', 'test', 'test', 'scripted', 1000, 1000, 2000, 1000000, 'actual')
        "#,
    )
    .bind(org_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .expect("seed exhausted usage");

    let notebook = ctx
        .create_notebook_with_token(&token, "quota-test")
        .await
        .expect("create notebook");

    let resp = ctx
        .chat_with_bearer_token(&token, "hi", &notebook.id)
        .await
        .expect("chat response");

    assert_eq!(
        resp.status, 429,
        "exhausted quota must block chat with 429, body={}",
        resp.body_json,
    );
    assert_eq!(
        resp.body_json.get("error").and_then(|value| value.as_str()),
        Some("quota_exceeded"),
        "body must carry quota_exceeded (not rate_limit_exceeded), body={}",
        resp.body_json,
    );
}
