//! Rolling usage hard-cap 429 boundary at the integration layer (ADR-0006).
//!
//! Product semantics: soft limit (plan) allows traffic; hard-cap
//! (`limit × multiplier`, default 3×) returns HTTP 429 with
//! `usage_limit_exceeded` only when `USAGE_LIMIT_ENFORCEMENT_PHASE` is
//! `5h_enforcement` or `7d_enforcement`. Distinct from per-key
//! `rate_limit_exceeded` (`rate_limit_boundary`).
//!
//! Real PG + mock LLM. Quota check runs before the agent (`new_smoke`).

use uuid::Uuid;

use crate::product_e2e::TestContext;

#[tokio::test]
async fn exhausted_quota_blocks_chat_with_quota_exceeded() {
    super::require_integration_suite();

    // Enforcement is off by default in many envs (shadow). Force 5h hard block.
    let prev_phase = std::env::var("USAGE_LIMIT_ENFORCEMENT_PHASE").ok();
    // SAFETY: product_e2e runs with --test-threads=1 for this suite path.
    unsafe {
        std::env::set_var("USAGE_LIMIT_ENFORCEMENT_PHASE", "5h_enforcement");
    }

    let ctx = TestContext::new_smoke().await;

    let email = format!("quota-boundary-{}@example.test", Uuid::new_v4());
    let token = ctx
        .register_user_token(&email, "Quota Boundary")
        .await
        .expect("register user");

    let pool = sqlx::PgPool::connect(&ctx.pg_url)
        .await
        .expect("connect test pg");
    // B2C personal: users has no owner_user_id; account root is users.id.
    let user_id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = $1")
        .bind(&email)
        .fetch_one(&pool)
        .await
        .expect("fetch registered user");
    let owner_user_id = user_id;

    // Far past free plan × hard_cap_multiplier so blocked_5h is true.
    sqlx::query(
        r#"
        INSERT INTO llm_usage_events (
            owner_user_id, user_id, feature, stage, provider, model,
            prompt_tokens, completion_tokens, total_tokens,
            usage_units, usage_source
        ) VALUES ($1, $2, 'chat', 'test', 'test', 'scripted', 1000, 1000, 2000, 1000000, 'actual')
        "#,
    )
    .bind(owner_user_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .expect("seed exhausted usage");

    let notebook = ctx
        .create_workspace_with_token(&token, "quota-test")
        .await
        .expect("create notebook");

    let resp = ctx
        .chat_with_bearer_token(&token, "hi", &notebook.id)
        .await
        .expect("chat response");

    match prev_phase {
        Some(v) => unsafe { std::env::set_var("USAGE_LIMIT_ENFORCEMENT_PHASE", v) },
        None => unsafe { std::env::remove_var("USAGE_LIMIT_ENFORCEMENT_PHASE") },
    }

    assert_eq!(
        resp.status, 429,
        "hard-cap must block chat with 429, body={}",
        resp.body_json,
    );
    let error = resp
        .body_json
        .get("error")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    assert_eq!(
        error, "usage_limit_exceeded",
        "expected usage_limit_exceeded (ADR-0006 hard cap), not rate_limit_exceeded; body={}",
        resp.body_json,
    );
}
