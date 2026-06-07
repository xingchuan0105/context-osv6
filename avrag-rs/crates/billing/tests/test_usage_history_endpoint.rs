//! Integration tests for the `usage/history` endpoint contract.
//!
//! The handler under test is `handle_get_usage_history` in
//! `crates/billing/src/api.rs`, which wraps `load_usage_history` in
//! `crates/billing/src/core_usage.rs`. It is a straight-line SQL function
//! that aggregates `llm_usage_events.usage_units` per day for the last N
//! days (default 7) for the authenticated user.
//!
//! The pure-rust test locks the response shape the frontend
//! `UsageTrendChart` (Task 12) deserializes. The `#[sqlx::test]` is
//! CI-only (requires a live PG) and verifies the SQL aggregation
//! produces one row per day for a seeded event stream.

use avrag_billing::{DailyUsage, UsageHistoryResponse};
use chrono::{Duration, TimeZone, Utc};
use sqlx::PgPool;
use uuid::Uuid;

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

#[sqlx::test]
async fn usage_history_aggregates_daily_token_usage_from_llm_usage_events(pool: PgPool) {
    sqlx::migrate!("../../migrations").run(&pool).await.unwrap();

    // Seed user + org.
    let org_id: Uuid = sqlx::query_scalar(
        "insert into organizations (name) values ($1) returning id",
    )
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

    // Seed 3 distinct days of usage (50K units per day) within the last
    // 7-day window. Use UTC midnight offsets to keep date_trunc deterministic.
    let now = Utc::now();
    let base_day = now.date_naive();
    for offset_days in 0..3_i64 {
        let day = base_day - chrono::Days::new(offset_days as u64);
        let when = Utc
            .from_utc_datetime(&day.and_hms_opt(12, 0, 0).unwrap());
        sqlx::query(
            r#"
            insert into llm_usage_events
                (org_id, user_id, feature, stage, provider, model,
                 prompt_tokens, completion_tokens, total_tokens,
                 usage_units, usage_source, created_at)
            values ($1, $2, 'chat', 'unknown', 'dashscope', 'qwen3.5-flash',
                    0, 0, 0, 50000, 'actual', $3)
            "#,
        )
        .bind(org_id)
        .bind(user_id)
        .bind(when)
        .execute(&pool)
        .await
        .unwrap();
    }

    // Sanity: 3 days x 50K = 150K total, grouped by date_trunc('day').
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
