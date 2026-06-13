//! Shared helpers for billing integration tests that need a live Postgres.

use sqlx::PgPool;

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
