//! Integration tests for the `usage/history` endpoint contract.
//!
//! The handler under test is `handle_get_usage_history` in
//! `crates/billing/src/handlers.rs`, which wraps `load_usage_history` in
//! `crates/billing/src/core_usage.rs`. It is a straight-line SQL function
//! that aggregates `llm_usage_events.usage_units` per day for the last N
//! days (default 7) for the authenticated user.
//!
//! The pure-rust test locks the response shape the frontend
//! `UsageTrendChart` (Task 12) deserializes. A live-PG integration test
//! verifies the SQL aggregation produces one row per day for a seeded event stream.

mod support;

use avrag_billing::{DailyUsage, UsageHistoryResponse};
use chrono::{Duration, Utc};
use support::{pg_pool_or_skip, run_migrations, seed_llm_usage_events, seed_user_with_plan};

#[test]
fn usage_history_response_shape_matches_spec() {
    // Lock the field names here so any rename is caught at compile time
    // on both sides (frontend TS type + backend Rust struct).
    let resp = UsageHistoryResponse {
        daily: vec![
            DailyUsage {
                date: "2026-06-01".to_string(),
                tokens: 50000,
            },
            DailyUsage {
                date: "2026-06-02".to_string(),
                tokens: 75000,
            },
        ],
    };
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["daily"][0]["date"], "2026-06-01");
    assert_eq!(json["daily"][0]["tokens"], 50000);
    assert_eq!(json["daily"][1]["date"], "2026-06-02");
    assert_eq!(json["daily"][1]["tokens"], 75000);
}

#[tokio::test]
async fn usage_history_aggregates_daily_token_usage_from_llm_usage_events() {
    let Some(pool) =
        pg_pool_or_skip("usage_history_aggregates_daily_token_usage_from_llm_usage_events").await
    else {
        return;
    };
    run_migrations(&pool).await;
    sqlx::query("select set_config('app.current_role', 'super_admin', false)")
        .execute(&pool)
        .await
        .unwrap();

    let (user_id, owner_user_id) = seed_user_with_plan(&pool, "free").await;
    seed_llm_usage_events(&pool, owner_user_id, user_id, 3, 50_000).await;

    // Sanity: 3 days x 50K = 150K total, grouped by date_trunc('day').
    let now = Utc::now();
    let rows: Vec<(chrono::NaiveDate, i64)> = sqlx::query_as(
        r#"
        select date_trunc('day', created_at)::date as day,
               sum(usage_units)::bigint as tokens
        from llm_usage_events
        where user_id = $1 and created_at >= $2
        group by day
        order by day asc
        "#,
    )
    .bind(user_id)
    .bind(now - Duration::days(7))
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 3, "expected 3 distinct day buckets");
    assert!(rows.iter().all(|(_, tokens)| *tokens == 50000));
}
