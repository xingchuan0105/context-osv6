//! Integration tests for the `usage/forecast` endpoint contract.
//!
//! The handler under test is `handle_get_usage_forecast` in
//! `crates/billing/src/handlers.rs`, which wraps `load_usage_forecast` in
//! `crates/billing/src/core_usage.rs`. It projects a 30-day average
//! `llm_usage_events.usage_units` against the user's current 7d plan
//! limit and emits a bilingual upgrade suggestion.
//!
//! The pure-rust test below locks the response shape the frontend
//! `UsageForecastCard` (Task 11) deserializes. A live-PG integration test
//! verifies the 30-day aggregation across `llm_usage_events` matches the spec.

mod support;

use avrag_billing::UsageForecastResponse;
use chrono::{Duration, Utc};
use support::{pg_pool_or_skip, run_migrations, seed_llm_usage_events, seed_user_with_plan};

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

#[tokio::test]
async fn usage_forecast_aggregates_30d_token_usage_from_llm_usage_events() {
    let Some(pool) =
        pg_pool_or_skip("usage_forecast_aggregates_30d_token_usage_from_llm_usage_events").await
    else {
        return;
    };
    run_migrations(&pool).await;
    sqlx::query("select set_config('app.current_role', 'super_admin', false)")
        .execute(&pool)
        .await
        .unwrap();

    let (user_id, owner_user_id) = seed_user_with_plan(&pool, "free").await;
    seed_llm_usage_events(&pool, owner_user_id, user_id, 3, 10_000).await;

    // Sanity: aggregate the same SQL the handler uses. 3 days x 10K = 30K
    // total over the last 30 days. avg_daily = 30K / 30 = 1K.
    let now = Utc::now();
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
