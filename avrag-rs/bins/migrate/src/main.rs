//! Dedicated migration entry point — the only product path that runs schema
//! migrations (plus the jieba re-segmentation backfill).
//!
//! Reads `MIGRATION_DATABASE_URL` (falling back to `DATABASE_URL` for local
//! dev), uses a single connection, and exits when done. Never linked into the
//! API/worker binaries: their pool connections cannot reach `_sqlx_migrations`
//! or run DDL at all (grants exclude both for `avrag_runtime`).

use anyhow::{Context, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    telemetry::init("avrag-migrate")?;

    let database_url = match std::env::var("MIGRATION_DATABASE_URL") {
        Ok(url) if !url.trim().is_empty() => url,
        _ => std::env::var("DATABASE_URL")
            .context("MIGRATION_DATABASE_URL (or DATABASE_URL for local dev) must be set")?,
    };

    let bootstrap = avrag_storage_pg::BootstrapRepository::connect(&database_url)
        .await
        .context("connect migration database")?;

    tracing::info!(target: "avrag_migrate", "running migrations");
    bootstrap.migrate().await.context("run migrations")?;

    tracing::info!(target: "avrag_migrate", "migrations complete");
    Ok(())
}