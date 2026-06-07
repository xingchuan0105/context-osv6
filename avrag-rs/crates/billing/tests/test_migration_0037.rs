use sqlx::PgPool;

#[sqlx::test]
async fn migration_0037_sets_pricing_revamp_quotas(pool: PgPool) {
    // Apply all migrations up to 0037 (path relative to crate root: crates/billing)
    sqlx::migrate!("../../migrations").run(&pool).await.unwrap();

    // 1) quota_limits: capacity values refreshed, spec §1.1
    //    Free llm_input_tokens: 50K soft / 100K hard
    let row: (Option<i64>, Option<i64>) = sqlx::query_as(
        "SELECT soft_limit, hard_limit FROM quota_limits \
         WHERE plan_id = 'free' AND metric_type = 'llm_input_tokens'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, (Some(50000), Some(100000)));

    //    Plus llm_output_tokens: 250K soft / 500K hard
    let row: (Option<i64>, Option<i64>) = sqlx::query_as(
        "SELECT soft_limit, hard_limit FROM quota_limits \
         WHERE plan_id = 'plus' AND metric_type = 'llm_output_tokens'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, (Some(250000), Some(500000)));

    // 2) usage_limit_plan_policies: 5h/7d rolling limits per spec §2.1
    //    Free: 5h 100K, 7d 400K
    let row: (i64, i64) = sqlx::query_as(
        "SELECT rolling_5h_limit_units, rolling_7d_limit_units \
         FROM usage_limit_plan_policies WHERE plan_id = 'free'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, (100000, 400000));

    //    Plus: 5h 600K, 7d 4M
    let row: (i64, i64) = sqlx::query_as(
        "SELECT rolling_5h_limit_units, rolling_7d_limit_units \
         FROM usage_limit_plan_policies WHERE plan_id = 'plus'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, (600000, 4000000));

    //    Pro: 5h 2.5M, 7d 15M
    let row: (i64, i64) = sqlx::query_as(
        "SELECT rolling_5h_limit_units, rolling_7d_limit_units \
         FROM usage_limit_plan_policies WHERE plan_id = 'pro'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, (2500000, 15000000));
}

#[sqlx::test]
async fn migration_0037_preserves_enterprise_unlimited_policy(pool: PgPool) {
    // The revamp targets free / plus / pro only; the legacy enterprise row
    // (left at 0/0 = unlimited by 0018) must remain untouched.
    sqlx::migrate!("../../migrations").run(&pool).await.unwrap();

    let row: (i64, i64) = sqlx::query_as(
        "SELECT rolling_5h_limit_units, rolling_7d_limit_units \
         FROM usage_limit_plan_policies WHERE plan_id = 'enterprise'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, (0, 0), "enterprise tier should remain unlimited (0/0)");
}
