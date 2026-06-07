use sqlx::PgPool;

#[sqlx::test]
async fn migration_0037_sets_pricing_revamp_quotas(pool: PgPool) {
    // Apply all migrations up to 0037 (path relative to crate root: crates/billing)
    sqlx::migrate!("../../migrations").run(&pool).await.unwrap();

    // 1) quota_limits: Free tier llm_input_tokens
    let row: (Option<i64>, Option<i64>) = sqlx::query_as(
        "SELECT soft_limit, hard_limit FROM quota_limits \
         WHERE plan_id = 'free' AND metric_type = 'llm_input_tokens'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, (Some(50000), Some(100000)));

    // 2) usage_limit_plan_policies: Plus tier 5h/7d
    let row: (i64, i64) = sqlx::query_as(
        "SELECT rolling_5h_limit_units, rolling_7d_limit_units \
         FROM usage_limit_plan_policies WHERE plan_id = 'plus'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, (600000, 4000000));
}
