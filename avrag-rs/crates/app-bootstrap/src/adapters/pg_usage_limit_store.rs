use std::sync::Arc;

use app_core::{
    MeteringContext, UsageLimitOverrideRow, UsageLimitPlanPolicyRow, UsageLimitStorePort,
    UsageLimitUsageRecord,
};
use async_trait::async_trait;
use avrag_storage_pg::PgAppRepository;
use chrono::{DateTime, Utc};
use common::AppError;
use sqlx::Row;
use uuid::Uuid;

pub struct PgUsageLimitStoreAdapter {
    repo: Arc<PgAppRepository>,
}

impl PgUsageLimitStoreAdapter {
    pub fn new(repo: Arc<PgAppRepository>) -> Self {
        Self { repo }
    }

    fn pool(&self) -> &sqlx::PgPool {
        self.repo.raw()
    }
}

#[async_trait]
impl UsageLimitStorePort for PgUsageLimitStoreAdapter {
    async fn insert_llm_usage_event(
        &self,
        ctx: &MeteringContext,
        record: UsageLimitUsageRecord<'_>,
    ) -> Result<i64, AppError> {
        let (input_rate, output_rate) =
            self.load_model_rates(record.provider, record.model).await?;
        let usage_units = app_core::compute_usage_units_with_rates(
            record.prompt_tokens,
            record.completion_tokens,
            input_rate,
            output_rate,
        );
        sqlx::query(
            r#"
            INSERT INTO llm_usage_events (
                org_id, user_id, feature, stage, provider, model,
                prompt_tokens, completion_tokens, total_tokens,
                usage_units, usage_source,
                session_id, document_id, request_id, trace_id
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            "#,
        )
        .bind(ctx.org_id)
        .bind(ctx.user_id)
        .bind(ctx.feature.as_str())
        .bind(&ctx.stage)
        .bind(record.provider)
        .bind(record.model)
        .bind(record.prompt_tokens as i64)
        .bind(record.completion_tokens as i64)
        .bind(record.total_tokens as i64)
        .bind(usage_units)
        .bind(record.usage_source.as_str())
        .bind(ctx.session_id)
        .bind(ctx.document_id)
        .bind(&ctx.request_id)
        .bind(&ctx.trace_id)
        .execute(self.pool())
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
            SELECT rolling_5h_limit_units, rolling_7d_limit_units, enabled
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
        }))
    }

    async fn sum_usage_units_since(
        &self,
        user_id: Uuid,
        since: DateTime<Utc>,
    ) -> Result<i64, AppError> {
        let row = sqlx::query(
            r#"
            SELECT COALESCE(SUM(usage_units), 0)::bigint AS total
            FROM llm_usage_events
            WHERE user_id = $1 AND created_at >= $2
            "#,
        )
        .bind(user_id)
        .bind(since)
        .fetch_one(self.pool())
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
        let row = sqlx::query(
            r#"
            SELECT created_at
            FROM llm_usage_events
            WHERE user_id = $1 AND created_at >= $2
            ORDER BY created_at ASC
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .bind(since)
        .fetch_optional(self.pool())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(row.and_then(|row| row.try_get::<DateTime<Utc>, _>("created_at").ok()))
    }

    async fn load_usage_breakdown(
        &self,
        user_id: Uuid,
        since: DateTime<Utc>,
    ) -> Result<std::collections::HashMap<String, i64>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT feature, COALESCE(SUM(usage_units), 0)::bigint AS total
            FROM llm_usage_events
            WHERE user_id = $1 AND created_at >= $2
            GROUP BY feature
            "#,
        )
        .bind(user_id)
        .bind(since)
        .fetch_all(self.pool())
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

    async fn load_model_rates(&self, provider: &str, model: &str) -> Result<(f64, f64), AppError> {
        let row = sqlx::query(
            r#"
            SELECT input_unit_rate, output_unit_rate
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
                row.try_get::<f64, _>("output_unit_rate").unwrap_or(2.0),
            )
        } else {
            (1.0, 2.0)
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
        .fetch_one(self.pool())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(row
            .try_get::<bool, _>("has_estimated")
            .map_err(|e| AppError::internal(e.to_string()))?)
    }
}
