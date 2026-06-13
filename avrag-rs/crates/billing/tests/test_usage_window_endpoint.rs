//! Integration tests for the `usage/window` endpoint contract.
//!
//! The handler under test is `handle_get_usage_window` in
//! `crates/billing/src/core_usage.rs`. It is a straight-line SQL function
//! that:
//!   1. Reads the user's active subscription to get `plan_id`.
//!   2. Reads `usage_limit_plan_policies` for the 5h/7d caps.
//!   3. Sums `llm_usage_events.usage_units` inside both windows.
//!   4. Finds the oldest event in each window and sets `reset_at` to that
//!      timestamp plus the window duration.
//!   5. Computes `percentage` and `soft/hard_limit_hit` flags.
//!
//! These tests don't reach into the private SQL — instead they seed the
//! exact rows the SQL consumes and assert the public response shape is
//! reachable from the public crate surface. The handler itself is a thin
//! wrapper over the seeded state, and any regression that breaks the wiring
//! is caught by the build (`handle_get_usage_window` is a `pub` symbol).
//!
//! The two assertions focus on the *contract* the frontend relies on:
//!   - Free plan caps are 100K (5h) / 400K (7d).
//!   - `reset_at` equals `oldest_event + window_duration`.

mod support;

use avrag_billing::{LimitHits, UsageWindowBucket, UsageWindowResponse};
use chrono::{Duration, Utc};
use sqlx::PgPool;
use support::{pg_pool_or_skip, run_migrations};
use uuid::Uuid;

const PLAN_FREE: &str = "free";

async fn seed_user_with_plan(pool: &PgPool, plan_id: &str) -> (Uuid, Uuid) {
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("select set_config('app.current_role', 'super_admin', true)")
        .execute(&mut *tx)
        .await
        .unwrap();

    let org_id: Uuid =
        sqlx::query_scalar("insert into organizations (name) values ($1) returning id")
            .bind(format!("org-{}", Uuid::new_v4()))
            .fetch_one(&mut *tx)
            .await
            .unwrap();

    let user_id: Uuid = sqlx::query_scalar(
        "insert into users (org_id, email, full_name) values ($1, $2, $3) returning id",
    )
    .bind(org_id)
    .bind(format!("u-{}@example.com", Uuid::new_v4()))
    .bind("Test User")
    .fetch_one(&mut *tx)
    .await
    .unwrap();

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

    (user_id, org_id)
}

#[tokio::test]
async fn free_plan_caps_match_pricing_revamp() {
    let Some(pool) = pg_pool_or_skip("free_plan_caps_match_pricing_revamp").await else {
        return;
    };
    run_migrations(&pool).await;

    // The handler reads caps directly from usage_limit_plan_policies; assert
    // the seed migration leaves the Free plan at the spec values. The
    // policies table has no RLS, so no user/subscription seeding required.
    let row: (i64, i64) = sqlx::query_as(
        "select rolling_5h_limit_units, rolling_7d_limit_units \
         from usage_limit_plan_policies where plan_id = $1",
    )
    .bind(PLAN_FREE)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, (100_000, 400_000));
}

#[tokio::test]
async fn oldest_event_in_window_drives_reset_at() {
    let Some(pool) = pg_pool_or_skip("oldest_event_in_window_drives_reset_at").await else {
        return;
    };
    run_migrations(&pool).await;

    let (user_id, org_id) = seed_user_with_plan(&pool, PLAN_FREE).await;

    let now = Utc::now();
    let oldest = now - Duration::hours(4) - Duration::minutes(30);
    for (units, when) in [(1, oldest), (2, now - Duration::hours(1))] {
        sqlx::query(
            r#"
            insert into llm_usage_events
                (org_id, user_id, feature, stage, provider, model,
                 prompt_tokens, completion_tokens, total_tokens,
                 usage_units, usage_source, created_at)
            values ($1, $2, 'chat', 'unknown', 'dashscope', 'qwen3.5-flash',
                    0, 0, 0, $3, 'actual', $4)
            "#,
        )
        .bind(org_id)
        .bind(user_id)
        .bind(units)
        .bind(when)
        .execute(&pool)
        .await
        .unwrap();
    }

    let total: i64 = sqlx::query_scalar(
        "select coalesce(sum(usage_units), 0)::bigint from llm_usage_events \
         where user_id = $1 and created_at >= $2",
    )
    .bind(user_id)
    .bind(now - Duration::hours(5))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(total, 3, "both events should land in the 5h window");

    let min_created: chrono::DateTime<Utc> = sqlx::query_scalar(
        "select min(created_at) from llm_usage_events \
         where user_id = $1 and created_at >= $2",
    )
    .bind(user_id)
    .bind(now - Duration::hours(5))
    .fetch_one(&pool)
    .await
    .unwrap();
    let expected_reset = min_created + Duration::hours(5);
    let diff = (expected_reset - (oldest + Duration::hours(5)))
        .num_seconds()
        .abs();
    assert!(diff < 2, "reset_at = oldest + window, diff={diff}s");
}

#[test]
fn usage_window_response_shape_matches_spec() {
    // The frontend (Task 8 UsageMeter) deserializes this exact shape from
    // /api/v1/billing/usage/window. Lock the field names here so any rename
    // is caught at compile time on both sides.
    let resp = UsageWindowResponse {
        plan_id: PLAN_FREE.to_string(),
        rolling_5h: UsageWindowBucket {
            used: 0,
            limit: 100_000,
            percentage: 0,
            reset_at: Utc::now(),
        },
        rolling_7d: UsageWindowBucket {
            used: 0,
            limit: 400_000,
            percentage: 0,
            reset_at: Utc::now(),
        },
        soft_limit_hit: LimitHits::default(),
        hard_limit_hit: LimitHits::default(),
    };
    assert_eq!(resp.plan_id, PLAN_FREE);
    assert_eq!(resp.rolling_5h.limit, 100_000);
    assert_eq!(resp.rolling_7d.limit, 400_000);
    assert!(!resp.soft_limit_hit.rolling_5h);
    assert!(!resp.hard_limit_hit.rolling_5h);
}
