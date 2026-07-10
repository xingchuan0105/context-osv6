//! Shared helpers for billing integration tests that need a live Postgres.

use chrono::{TimeZone, Utc};
use sqlx::PgPool;
use uuid::Uuid;

const SKIP_REASON: &str =
    "DATABASE_URL not set; live Postgres required for billing sqlx integration tests";

/// Connect to Postgres when `DATABASE_URL` is configured; otherwise skip the test.
pub async fn pg_pool_or_skip(test_name: &str) -> Option<PgPool> {
    let Some(database_url) = std::env::var("DATABASE_URL")
        .ok()
        .filter(|url| !url.trim().is_empty())
    else {
        eprintln!("skip: {test_name} — {SKIP_REASON}");
        return None;
    };

    match PgPool::connect(&database_url).await {
        Ok(pool) => Some(pool),
        Err(error) => panic!(
            "{test_name}: failed to connect using DATABASE_URL ({error}); \
             ensure Postgres is reachable for migration tests"
        ),
    }
}

/// Apply workspace migrations for billing PG tests.
pub async fn run_migrations(pool: &PgPool) {
    sqlx::migrate!("../../migrations")
        .run(pool)
        .await
        .expect("billing migration tests should apply workspace migrations");
}

/// Seed org + user + active subscription for usage endpoint tests.
pub async fn seed_user_with_plan(pool: &PgPool, plan_id: &str) -> (Uuid, Uuid) {
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("select set_config('app.current_role', 'super_admin', true)")
        .execute(&mut *tx)
        .await
        .unwrap();

    // Personal B2C: account owner == user id (no organizations table).
    let user_id: Uuid = sqlx::query_scalar(
        "insert into users (email, full_name, role) values ($1, $2, $3) returning id",
    )
    .bind(format!("u-{}@example.com", Uuid::new_v4()))
    .bind("Test User")
    .bind("user")
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    let owner_user_id = user_id;

    sqlx::query(
        r#"
        insert into subscriptions
            (user_id, plan_id, status, billing_provider, cancel_at_period_end)
        values ($1, $2, 'active', 'stripe', false)
        "#,
    )
    .bind(user_id)
    .bind(plan_id)
    .execute(&mut *tx)
    .await
    .unwrap();

    tx.commit().await.unwrap();

    (user_id, owner_user_id)
}

/// Seed `days` distinct daily usage rows with `usage_units` per day.
pub async fn seed_llm_usage_events(
    pool: &PgPool,
    owner_user_id: Uuid,
    user_id: Uuid,
    days: i64,
    usage_units: i64,
) {
    let now = Utc::now();
    let base_day = now.date_naive();
    for offset_days in 0..days {
        let day = base_day - chrono::Days::new(offset_days as u64);
        let when = Utc.from_utc_datetime(&day.and_hms_opt(12, 0, 0).unwrap());
        sqlx::query(
            r#"
            insert into llm_usage_events
                (owner_user_id, user_id, feature, stage, provider, model,
                 prompt_tokens, completion_tokens, total_tokens,
                 usage_units, usage_source, created_at)
            values ($1, $2, 'chat', 'unknown', 'dashscope', 'qwen3.5-flash',
                    0, 0, 0, $3, 'actual', $4)
            "#,
        )
        .bind(owner_user_id)
        .bind(user_id)
        .bind(usage_units)
        .bind(when)
        .execute(pool)
        .await
        .unwrap();
    }
}
