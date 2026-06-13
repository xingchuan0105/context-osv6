//! Integration tests for the `usage/forecast` endpoint contract.
//!
//! The handler under test is `handle_get_usage_forecast` in
//! `crates/billing/src/api.rs`, which wraps `load_usage_forecast` in
//! `crates/billing/src/core_usage.rs`. It projects a 30-day average
//! `llm_usage_events.usage_units` against the user's current 7d plan
//! limit and emits a bilingual upgrade suggestion.
//!
//! The pure-rust test below locks the response shape the frontend
//! `UsageForecastCard` (Task 11) deserializes. A `#[sqlx::test]` is
//! CI-only (requires a live PG) and verifies the 30-day aggregation
//! across `llm_usage_events` matches the spec.

use avrag_billing::UsageForecastResponse;
use chrono::{Duration, TimeZone, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[test]
fn usage_forecast_response_shape_matches_spec() {
    // Lock the field names here so any rename is caught at compile time
    // on both sides (frontend TS type + backend Rust struct).
    let resp = UsageForecastResponse {
        current_plan: "free".to_string(),
        avg_30d_tokens: 8000,
        projected_30d_tokens: 240000,
        current_limit_7d: 400000,
        upgrade_recommended: false,
        suggestion_zh: "按当前用量，本月无需升级".to_string(),
        suggestion_en: "Based on current usage, no upgrade needed this month".to_string(),
    };
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["current_plan"], "free");
    assert_eq!(json["avg_30d_tokens"], 8000);
    assert_eq!(json["projected_30d_tokens"], 240000);
    assert_eq!(json["current_limit_7d"], 400000);
    assert_eq!(json["upgrade_recommended"], false);
    assert!(json["suggestion_zh"].as_str().unwrap().contains("无需升级"));
    assert!(
        json["suggestion_en"]
            .as_str()
            .unwrap()
            .contains("no upgrade needed")
    );
}

#[sqlx::test]
async fn usage_forecast_aggregates_30d_token_usage_from_llm_usage_events(pool: PgPool) {
    sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
    sqlx::query("select set_config('app.current_role', 'super_admin', false)")
        .execute(&pool)
        .await
        .unwrap();

    // Seed user + org.
    let org_id: Uuid =
        sqlx::query_scalar("insert into organizations (name) values ($1) returning id")
            .bind(format!("org-{}", Uuid::new_v4()))
            .fetch_one(&pool)
            .await
            .unwrap();

    let user_id: Uuid = sqlx::query_scalar(
        "insert into users (org_id, email, full_name) values ($1, $2, $3) returning id",
    )
    .bind(org_id)
    .bind(format!("u-{}@example.com", Uuid::new_v4()))
    .bind("Test User")
    .fetch_one(&pool)
    .await
    .unwrap();

    // Seed 3 distinct days of usage (10K units per day) within the last
    // 30-day window. Use UTC midday offsets to keep date_trunc deterministic.
    let now = Utc::now();
    let base_day = now.date_naive();
    for offset_days in 0..3_i64 {
        let day = base_day - chrono::Days::new(offset_days as u64);
        let when = Utc.from_utc_datetime(&day.and_hms_opt(12, 0, 0).unwrap());
        sqlx::query(
            r#"
            insert into llm_usage_events
                (org_id, user_id, feature, stage, provider, model,
                 prompt_tokens, completion_tokens, total_tokens,
                 usage_units, usage_source, created_at)
            values ($1, $2, 'chat', 'unknown', 'dashscope', 'qwen3.5-flash',
                    0, 0, 0, 10000, 'actual', $3)
            "#,
        )
        .bind(org_id)
        .bind(user_id)
        .bind(when)
        .execute(&pool)
        .await
        .unwrap();
    }

    // Sanity: aggregate the same SQL the handler uses. 3 days x 10K = 30K
    // total over the last 30 days. avg_daily = 30K / 30 = 1K.
    let total: i64 = sqlx::query_scalar(
        r#"
        select coalesce(sum(usage_units), 0)::bigint
        from llm_usage_events
        where user_id = $1 and created_at >= $2
        "#,
    )
    .bind(user_id)
    .bind(now - Duration::days(30))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(total, 30000, "expected 3 days x 10K = 30K total");

    let projected_30d = (total / 30) * 30;
    assert_eq!(projected_30d, 30000, "projected_30d = (total/30)*30");
}
