use std::sync::Arc;

use app_core::{
    MeteringContext, UsageExportJobRow, UsageLimitOverrideRow, UsageLimitPlanPolicyRow,
    UsageLimitStorePort, UsageLimitUsageRecord,
};
use async_trait::async_trait;
use avrag_storage_pg::PgAppRepository;
use chrono::{DateTime, Utc};
use common::AppError;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::adapters::pg_session::set_current_user;

pub struct PgUsageLimitStoreAdapter {
    repo: Arc<PgAppRepository>,
}

impl PgUsageLimitStoreAdapter {
    pub fn new(repo: Arc<PgAppRepository>) -> Self {
        Self { repo }
    }

    fn pool(&self) -> &PgPool {
        self.repo.raw()
    }

    /// `llm_usage_events` has FORCE RLS on `owner_user_id = app.current_user`.
    /// All product reads/writes must open a txn and set that GUC first.
    async fn begin_as_owner(
        &self,
        owner_user_id: Uuid,
    ) -> Result<Transaction<'_, Postgres>, AppError> {
        let mut tx = self
            .pool()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_user(tx.as_mut(), &owner_user_id.to_string()).await?;
        Ok(tx)
    }
}

#[async_trait]
impl UsageLimitStorePort for PgUsageLimitStoreAdapter {
    async fn insert_llm_usage_event(
        &self,
        ctx: &MeteringContext,
        record: UsageLimitUsageRecord<'_>,
    ) -> Result<i64, AppError> {
        let (rate_miss, rate_cache, rate_out) =
            self.load_model_rates(record.provider, record.model).await?;
        // Billable rows use the user's plan margin M; internal rows still unitize with M=1.0
        // so analytics stay comparable without inflating non-quota ledgers.
        let margin_multiplier = if record.billable {
            let plan_id = self.get_user_plan(ctx.user_id).await.unwrap_or_else(|_| "free".into());
            self.load_plan_policy(&plan_id)
                .await
                .ok()
                .flatten()
                .map(|p| p.margin_multiplier)
                .filter(|m| m.is_finite() && *m > 0.0)
                .unwrap_or(2.0)
        } else {
            1.0
        };
        let usage_units = app_core::compute_usage_units_three_bucket(
            record.prompt_tokens,
            record.completion_tokens,
            record.cached_tokens,
            rate_miss,
            rate_cache,
            rate_out,
            margin_multiplier,
        );
        let mut tx = self.begin_as_owner(ctx.owner_user_id).await?;
        sqlx::query(
            r#"
            INSERT INTO llm_usage_events (
                owner_user_id, user_id, feature, stage, provider, model,
                prompt_tokens, completion_tokens, total_tokens, cached_tokens,
                usage_units, usage_source, usage_kind, billable,
                session_id, document_id, request_id, trace_id
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
            "#,
        )
        .bind(ctx.owner_user_id)
        .bind(ctx.user_id)
        .bind(ctx.feature.as_str())
        .bind(&ctx.stage)
        .bind(record.provider)
        .bind(record.model)
        .bind(record.prompt_tokens as i64)
        .bind(record.completion_tokens as i64)
        .bind(record.total_tokens as i64)
        .bind(record.cached_tokens as i64)
        .bind(usage_units)
        .bind(record.usage_source.as_str())
        .bind(record.usage_kind)
        .bind(record.billable)
        .bind(ctx.session_id)
        .bind(ctx.document_id)
        .bind(&ctx.request_id)
        .bind(&ctx.trace_id)
        .execute(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(usage_units)
    }

    async fn load_user_override(
        &self,
        user_id: Uuid,
    ) -> Result<Option<UsageLimitOverrideRow>, AppError> {
        let row = sqlx::query(
            r#"
            SELECT rolling_5h_limit_units, rolling_7d_limit_units, enabled
            FROM usage_limit_user_overrides
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(self.pool())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(row.map(|row| UsageLimitOverrideRow {
            rolling_5h_limit_units: row.try_get("rolling_5h_limit_units").ok().flatten(),
            rolling_7d_limit_units: row.try_get("rolling_7d_limit_units").ok().flatten(),
            enabled: row.try_get("enabled").unwrap_or(true),
        }))
    }

    async fn get_user_plan(&self, user_id: Uuid) -> Result<String, AppError> {
        let row = sqlx::query(
            r#"
            SELECT plan_id FROM subscriptions
            WHERE user_id = $1 AND status = 'active'
            ORDER BY updated_at DESC LIMIT 1
            "#,
        )
        .bind(user_id)
        .fetch_optional(self.pool())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(row
            .and_then(|r| r.try_get::<String, _>("plan_id").ok())
            .unwrap_or_else(|| "free".to_string()))
    }

    async fn load_plan_policy(
        &self,
        plan_id: &str,
    ) -> Result<Option<UsageLimitPlanPolicyRow>, AppError> {
        let row = sqlx::query(
            r#"
            SELECT rolling_5h_limit_units, rolling_7d_limit_units, enabled,
                   COALESCE(margin_multiplier, 2.0) AS margin_multiplier
            FROM usage_limit_plan_policies
            WHERE plan_id = $1
            "#,
        )
        .bind(plan_id)
        .fetch_optional(self.pool())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(row.map(|row| UsageLimitPlanPolicyRow {
            enabled: row.try_get("enabled").unwrap_or(true),
            rolling_5h_limit_units: row.try_get("rolling_5h_limit_units").unwrap_or(100),
            rolling_7d_limit_units: row.try_get("rolling_7d_limit_units").unwrap_or(1000),
            margin_multiplier: row.try_get("margin_multiplier").unwrap_or(2.0),
        }))
    }

    async fn sum_usage_units_since(
        &self,
        user_id: Uuid,
        since: DateTime<Utc>,
    ) -> Result<i64, AppError> {
        // ADR 0006 §7: only customer-billable rows count toward rolling quotas.
        // B2C: owner_user_id == user_id for RLS; set GUC to that principal.
        let mut tx = self.begin_as_owner(user_id).await?;
        let row = sqlx::query(
            r#"
            SELECT COALESCE(SUM(usage_units), 0)::bigint AS total
            FROM llm_usage_events
            WHERE user_id = $1
              AND created_at >= $2
              AND billable = true
            "#,
        )
        .bind(user_id)
        .bind(since)
        .fetch_one(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(row
            .try_get::<i64, _>("total")
            .map_err(|e| AppError::internal(e.to_string()))?)
    }

    async fn oldest_usage_event_since(
        &self,
        user_id: Uuid,
        since: DateTime<Utc>,
    ) -> Result<Option<DateTime<Utc>>, AppError> {
        let mut tx = self.begin_as_owner(user_id).await?;
        let row = sqlx::query(
            r#"
            SELECT created_at
            FROM llm_usage_events
            WHERE user_id = $1
              AND created_at >= $2
              AND billable = true
            ORDER BY created_at ASC
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .bind(since)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(row.and_then(|row| row.try_get::<DateTime<Utc>, _>("created_at").ok()))
    }

    async fn load_usage_breakdown(
        &self,
        user_id: Uuid,
        since: DateTime<Utc>,
    ) -> Result<std::collections::HashMap<String, i64>, AppError> {
        let mut tx = self.begin_as_owner(user_id).await?;
        let rows = sqlx::query(
            r#"
            SELECT feature, COALESCE(SUM(usage_units), 0)::bigint AS total
            FROM llm_usage_events
            WHERE user_id = $1
              AND created_at >= $2
              AND billable = true
            GROUP BY feature
            "#,
        )
        .bind(user_id)
        .bind(since)
        .fetch_all(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        let mut breakdown = std::collections::HashMap::new();
        for row in rows {
            breakdown.insert(
                row.try_get::<String, _>("feature")
                    .map_err(|e| AppError::internal(e.to_string()))?,
                row.try_get::<i64, _>("total")
                    .map_err(|e| AppError::internal(e.to_string()))?,
            );
        }
        Ok(breakdown)
    }

    async fn load_model_rates(
        &self,
        provider: &str,
        model: &str,
    ) -> Result<(f64, f64, f64), AppError> {
        let row = sqlx::query(
            r#"
            SELECT input_unit_rate,
                   COALESCE(cache_hit_unit_rate, 0.02) AS cache_hit_unit_rate,
                   output_unit_rate
            FROM llm_model_weights
            WHERE enabled = true AND provider = $1 AND model = $2
            ORDER BY effective_from DESC
            LIMIT 1
            "#,
        )
        .bind(provider)
        .bind(model)
        .fetch_optional(self.pool())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(if let Some(row) = row {
            (
                row.try_get::<f64, _>("input_unit_rate").unwrap_or(1.0),
                row.try_get::<f64, _>("cache_hit_unit_rate").unwrap_or(0.02),
                row.try_get::<f64, _>("output_unit_rate").unwrap_or(2.0),
            )
        } else {
            (1.0, 0.02, 2.0)
        })
    }

    async fn has_user_override(&self, user_id: Uuid) -> Result<bool, AppError> {
        let row = sqlx::query("SELECT user_id FROM usage_limit_user_overrides WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(self.pool())
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(row.is_some())
    }

    async fn has_estimated_usage(&self, user_id: Uuid) -> Result<bool, AppError> {
        let mut tx = self.begin_as_owner(user_id).await?;
        let row = sqlx::query(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM llm_usage_events
                WHERE user_id = $1 AND usage_source = 'estimated'
                LIMIT 1
            ) AS has_estimated
            "#,
        )
        .bind(user_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(row
            .try_get::<bool, _>("has_estimated")
            .map_err(|e| AppError::internal(e.to_string()))?)
    }

    async fn create_usage_export_job(
        &self,
        owner_user_id: Uuid,
        user_id: Uuid,
        range_from: DateTime<Utc>,
        range_to: DateTime<Utc>,
        format: &str,
    ) -> Result<Uuid, AppError> {
        let format = match format {
            "csv" | "jsonl" => format,
            _ => {
                return Err(AppError::validation(
                    "invalid_export_format",
                    "format must be csv or jsonl",
                ));
            }
        };
        if range_to <= range_from {
            return Err(AppError::validation(
                "invalid_export_range",
                "to must be after from",
            ));
        }
        // Sync-friendly window: ≤ 31 days still goes through the job table but is
        // processed immediately in the request path via process_export_job_by_id.
        let max_span = chrono::Duration::days(366);
        if range_to - range_from > max_span {
            return Err(AppError::validation(
                "export_range_too_large",
                "export window must be ≤ 366 days",
            ));
        }

        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO usage_export_jobs (
              id, owner_user_id, user_id, range_from, range_to, format, status
            ) VALUES ($1, $2, $3, $4, $5, $6, 'pending')
            "#,
        )
        .bind(id)
        .bind(owner_user_id)
        .bind(user_id)
        .bind(range_from)
        .bind(range_to)
        .bind(format)
        .execute(self.pool())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;

        // Eagerly process short windows so UI can download immediately.
        if range_to - range_from <= chrono::Duration::days(7) {
            let _ = self.process_export_job_by_id(id).await;
        }
        Ok(id)
    }

    async fn get_usage_export_job(
        &self,
        user_id: Uuid,
        export_id: Uuid,
    ) -> Result<Option<UsageExportJobRow>, AppError> {
        let row = sqlx::query(
            r#"
            SELECT id, status, format, range_from, range_to, row_count, result_text,
                   error_message, created_at, completed_at
            FROM usage_export_jobs
            WHERE id = $1 AND user_id = $2
            "#,
        )
        .bind(export_id)
        .bind(user_id)
        .fetch_optional(self.pool())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(row.map(|row| UsageExportJobRow {
            id: row.get("id"),
            status: row.get("status"),
            format: row.get("format"),
            range_from: row.get("range_from"),
            range_to: row.get("range_to"),
            row_count: row.get("row_count"),
            result_text: row.get("result_text"),
            error_message: row.get("error_message"),
            created_at: row.get("created_at"),
            completed_at: row.get("completed_at"),
        }))
    }

    async fn process_next_usage_export_job(&self) -> Result<bool, AppError> {
        let row = sqlx::query(
            r#"
            SELECT id FROM usage_export_jobs
            WHERE status = 'pending'
            ORDER BY created_at ASC
            LIMIT 1
            "#,
        )
        .fetch_optional(self.pool())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        let Some(row) = row else {
            return Ok(false);
        };
        let id: Uuid = row.get("id");
        self.process_export_job_by_id(id).await?;
        Ok(true)
    }

    async fn purge_llm_usage_older_than(
        &self,
        cutoff: DateTime<Utc>,
        limit: i64,
    ) -> Result<u64, AppError> {
        let result = sqlx::query(
            r#"
            DELETE FROM llm_usage_events
            WHERE ctid IN (
              SELECT ctid FROM llm_usage_events
              WHERE created_at < $1
              ORDER BY created_at ASC
              LIMIT $2
            )
            "#,
        )
        .bind(cutoff)
        .bind(limit)
        .execute(self.pool())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(result.rows_affected())
    }
}

impl PgUsageLimitStoreAdapter {
    async fn process_export_job_by_id(&self, job_id: Uuid) -> Result<(), AppError> {
        let job = sqlx::query(
            r#"
            SELECT user_id, range_from, range_to, format, status
            FROM usage_export_jobs WHERE id = $1
            "#,
        )
        .bind(job_id)
        .fetch_optional(self.pool())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        let Some(job) = job else {
            return Ok(());
        };
        let status: String = job.get("status");
        if status != "pending" {
            return Ok(());
        }
        let user_id: Uuid = job.get("user_id");
        let range_from: DateTime<Utc> = job.get("range_from");
        let range_to: DateTime<Utc> = job.get("range_to");
        let format: String = job.get("format");

        let rows = sqlx::query(
            r#"
            SELECT created_at, feature, stage, provider, model,
                   prompt_tokens, completion_tokens, total_tokens,
                   usage_units, usage_source, session_id, request_id
            FROM llm_usage_events
            WHERE user_id = $1
              AND billable = true
              AND created_at >= $2
              AND created_at < $3
            ORDER BY created_at ASC
            LIMIT 50000
            "#,
        )
        .bind(user_id)
        .bind(range_from)
        .bind(range_to)
        .fetch_all(self.pool())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;

        let row_count = rows.len() as i32;
        let result_text = if format == "jsonl" {
            let mut out = String::new();
            for row in &rows {
                let line = serde_json::json!({
                    "created_at": row.get::<DateTime<Utc>, _>("created_at").to_rfc3339(),
                    "feature": row.get::<String, _>("feature"),
                    "stage": row.get::<String, _>("stage"),
                    "provider": row.get::<String, _>("provider"),
                    "model": row.get::<String, _>("model"),
                    "prompt_tokens": row.get::<i64, _>("prompt_tokens"),
                    "completion_tokens": row.get::<i64, _>("completion_tokens"),
                    "total_tokens": row.get::<i64, _>("total_tokens"),
                    "usage_units": row.get::<i64, _>("usage_units"),
                    "usage_source": row.get::<String, _>("usage_source"),
                    "session_id": row.try_get::<Uuid, _>("session_id").ok().map(|u| u.to_string()),
                    "request_id": row.try_get::<String, _>("request_id").ok(),
                });
                out.push_str(&line.to_string());
                out.push('\n');
            }
            out
        } else {
            let mut out = String::from(
                "created_at,feature,stage,provider,model,prompt_tokens,completion_tokens,total_tokens,usage_units,usage_source,session_id,request_id\n",
            );
            for row in &rows {
                let session = row
                    .try_get::<Uuid, _>("session_id")
                    .ok()
                    .map(|u| u.to_string())
                    .unwrap_or_default();
                let request_id = row
                    .try_get::<String, _>("request_id")
                    .unwrap_or_default();
                out.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{},{},{},{}\n",
                    row.get::<DateTime<Utc>, _>("created_at").to_rfc3339(),
                    csv_escape(&row.get::<String, _>("feature")),
                    csv_escape(&row.get::<String, _>("stage")),
                    csv_escape(&row.get::<String, _>("provider")),
                    csv_escape(&row.get::<String, _>("model")),
                    row.get::<i64, _>("prompt_tokens"),
                    row.get::<i64, _>("completion_tokens"),
                    row.get::<i64, _>("total_tokens"),
                    row.get::<i64, _>("usage_units"),
                    csv_escape(&row.get::<String, _>("usage_source")),
                    csv_escape(&session),
                    csv_escape(&request_id),
                ));
            }
            out
        };

        sqlx::query(
            r#"
            UPDATE usage_export_jobs
            SET status = 'completed',
                row_count = $2,
                result_text = $3,
                completed_at = now(),
                error_message = NULL
            WHERE id = $1
            "#,
        )
        .bind(job_id)
        .bind(row_count)
        .bind(result_text)
        .execute(self.pool())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(())
    }
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}
