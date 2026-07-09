//! Exit-metering: successful chat writes `llm_usage_events` with `usage_kind='chat'`.
//!
//! Real PG + mock LLM (same profile as `quota_boundary`). Asserts the
//! `UsageObserver` path on `LlmClient` records a chat row after a general chat.

use uuid::Uuid;

use crate::product_e2e::{ChatResponse, TestContext, assertions::*};

#[tokio::test]
async fn chat_records_llm_usage_event_with_usage_kind_chat() {
    super::require_integration_suite();
    let ctx = TestContext::new_smoke().await;

    let email = format!("usage-exit-meter-{}@example.test", Uuid::new_v4());
    let token = ctx
        .register_user_token(&email, "Usage Exit Meter")
        .await
        .expect("register user");

    let pool = sqlx::PgPool::connect(&ctx.pg_url)
        .await
        .expect("connect test pg");
    let (user_id, org_id): (Uuid, Uuid) =
        sqlx::query_as("SELECT id, org_id FROM users WHERE email = $1")
            .bind(&email)
            .fetch_one(&pool)
            .await
            .expect("fetch registered user");

    // Baseline: no chat usage for this user yet.
    let before: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM llm_usage_events
        WHERE user_id = $1 AND org_id = $2 AND usage_kind = 'chat'
        "#,
    )
    .bind(user_id)
    .bind(org_id)
    .fetch_one(&pool)
    .await
    .expect("count usage before chat");

    let notebook = ctx
        .create_workspace_with_token(&token, "usage-exit-meter")
        .await
        .expect("create notebook");

    let http_resp = ctx
        .chat_with_bearer_token(&token, "Hello, who are you?", &notebook.id)
        .await
        .expect("chat response");

    assert_http_ok(&http_resp);
    assert!(http_resp.status < 500, "chat must not 5xx");
    let resp: ChatResponse = http_resp.into_business().unwrap();
    assert_eq!(resp.agent_type, "chat");
    assert_answer_substantive(&resp, 10);

    // Poll briefly — observer is async fail-open on the LLM success path.
    let mut after_count = before.0;
    for _ in 0..20 {
        let after: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM llm_usage_events
            WHERE user_id = $1 AND org_id = $2 AND usage_kind = 'chat'
            "#,
        )
        .bind(user_id)
        .bind(org_id)
        .fetch_one(&pool)
        .await
        .expect("count usage after chat");
        after_count = after.0;
        if after_count > before.0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    assert!(
        after_count > before.0,
        "expected at least one llm_usage_events row with usage_kind='chat' after chat \
         (before={before_count}, after={after_count})",
        before_count = before.0,
        after_count = after_count,
    );

    let row: (String, String, i64) = sqlx::query_as(
        r#"
        SELECT usage_kind, feature, total_tokens
        FROM llm_usage_events
        WHERE user_id = $1 AND org_id = $2 AND usage_kind = 'chat'
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(org_id)
    .fetch_one(&pool)
    .await
    .expect("fetch latest chat usage row");

    assert_eq!(row.0, "chat");
    assert!(
        !row.1.is_empty(),
        "feature label must be non-empty, got {:?}",
        row.1
    );
    assert!(
        row.2 > 0,
        "total_tokens should be positive from mock LLM usage, got {}",
        row.2
    );
}
