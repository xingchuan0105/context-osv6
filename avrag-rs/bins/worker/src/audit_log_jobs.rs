use anyhow::Result;
use sqlx::PgPool;
use std::time::{Duration, Instant};
use tracing::info;

const AUDIT_LOG_RETENTION_DAYS: i32 = 3;
const DEFAULT_INTERVAL_SECS: u64 = 3600;

pub struct AuditLogJobRunner {
    pool: PgPool,
    interval: Duration,
    last_run_at: Option<Instant>,
}

impl AuditLogJobRunner {
    pub fn from_env(pool: PgPool) -> Option<Self> {
        if !env_bool("AUDIT_LOG_PRUNE_ENABLED", true) {
            return None;
        }

        let interval_secs = std::env::var("AUDIT_LOG_PRUNE_INTERVAL_SECS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_INTERVAL_SECS);

        Some(Self {
            pool,
            interval: Duration::from_secs(interval_secs),
            last_run_at: None,
        })
    }

    pub async fn maybe_run(&mut self) -> Result<()> {
        let now = Instant::now();
        if let Some(last_run_at) = self.last_run_at
            && now.duration_since(last_run_at) < self.interval
        {
            return Ok(());
        }
        self.last_run_at = Some(now);

        let result = sqlx::query(
            r#"
            delete from audit_log
            where created_at < now() - ($1::int * interval '1 day')
            "#,
        )
        .bind(AUDIT_LOG_RETENTION_DAYS)
        .execute(&self.pool)
        .await?;

        let deleted = result.rows_affected();
        if deleted > 0 {
            info!(deleted, retention_days = AUDIT_LOG_RETENTION_DAYS, "audit_log pruned");
        }
        Ok(())
    }
}

fn env_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(default)
}
