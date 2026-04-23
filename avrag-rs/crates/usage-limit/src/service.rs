use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

use crate::usage_units::compute_usage_units_with_rates;
use crate::{
    MeteringContext, QuotaCheckResult, UsageLimitPolicy, UsageLimitResponse, UsageScope,
    UsageSource, UsageWindow, UsageWindows,
};

pub struct UsageLimitService {
    pool: PgPool,
}

impl UsageLimitService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Record actual LLM usage for a user. Returns the computed usage_units.
    pub async fn record_usage(
        &self,
        ctx: &MeteringContext,
        provider: &str,
        model: &str,
        prompt_tokens: u32,
        completion_tokens: u32,
        total_tokens: u32,
        usage_source: UsageSource,
    ) -> Result<i64> {
        let (input_rate, output_rate) = self.load_model_rates(provider, model).await?;
        let usage_units = compute_usage_units_with_rates(
            prompt_tokens,
            completion_tokens,
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
        .bind(provider)
        .bind(model)
        .bind(prompt_tokens as i64)
        .bind(completion_tokens as i64)
        .bind(total_tokens as i64)
        .bind(usage_units)
        .bind(usage_source.as_str())
        .bind(ctx.session_id)
        .bind(ctx.document_id)
        .bind(&ctx.request_id)
        .bind(&ctx.trace_id)
        .execute(&self.pool)
        .await?;

        Ok(usage_units)
    }

    /// Get current usage for a user.
    pub async fn get_user_usage(&self, org_id: Uuid, user_id: Uuid) -> Result<UsageLimitResponse> {
        let policy = self.load_effective_policy(org_id, user_id).await?;
        let windows = self.compute_windows(user_id, &policy).await?;
        let breakdown = self.load_breakdown(user_id).await?;
        let scope = self.determine_scope(user_id).await?;
        let has_estimated = self.has_estimated_usage(user_id).await?;

        Ok(UsageLimitResponse {
            policy,
            windows,
            breakdown,
            scope,
            has_estimated_usage: has_estimated,
        })
    }

    /// Check if a user is blocked by quota.
    pub async fn check_quota(&self, org_id: Uuid, user_id: Uuid) -> Result<QuotaCheckResult> {
        let policy = self.load_effective_policy(org_id, user_id).await?;
        let windows = self.compute_windows(user_id, &policy).await?;

        Ok(QuotaCheckResult {
            blocked_5h: windows.rolling_5h.blocked,
            blocked_7d: windows.rolling_7d.blocked,
            used_5h: windows.rolling_5h.used_units,
            limit_5h: windows.rolling_5h.limit_units,
            used_7d: windows.rolling_7d.used_units,
            limit_7d: windows.rolling_7d.limit_units,
            blocked_until_5h: windows.rolling_5h.blocked_until.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            }),
            blocked_until_7d: windows.rolling_7d.blocked_until.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            }),
        })
    }

    async fn load_effective_policy(&self, org_id: Uuid, user_id: Uuid) -> Result<UsageLimitPolicy> {
        // 1. Check user override
        let override_row = sqlx::query(
            r#"
            SELECT rolling_5h_limit_units, rolling_7d_limit_units, enabled
            FROM usage_limit_user_overrides
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = &override_row {
            let enabled: bool = row.try_get("enabled")?;
            if !enabled {
                return Ok(UsageLimitPolicy {
                    enabled: false,
                    rolling_5h_limit_units: 0,
                    rolling_7d_limit_units: 0,
                });
            }
            let h5: Option<i64> = row.try_get("rolling_5h_limit_units")?;
            let d7: Option<i64> = row.try_get("rolling_7d_limit_units")?;
            if h5.is_some() && d7.is_some() {
                return Ok(UsageLimitPolicy {
                    enabled: true,
                    rolling_5h_limit_units: h5.unwrap(),
                    rolling_7d_limit_units: d7.unwrap(),
                });
            }
            // Partial override — fall through to plan for the NULL fields
        }

        // 2. Determine plan from subscriptions table
        let plan_id = self.get_user_plan(org_id).await?;

        // 3. Load plan defaults
        let plan_row = sqlx::query(
            r#"
            SELECT rolling_5h_limit_units, rolling_7d_limit_units, enabled
            FROM usage_limit_plan_policies
            WHERE plan_id = $1
            "#,
        )
        .bind(&plan_id)
        .fetch_optional(&self.pool)
        .await?;

        let (default_5h, default_7d, plan_enabled) = plan_row
            .map(|row| {
                let e: bool = row.try_get("enabled").unwrap_or(true);
                let h: i64 = row.try_get("rolling_5h_limit_units").unwrap_or(100);
                let d: i64 = row.try_get("rolling_7d_limit_units").unwrap_or(1000);
                (h, d, e)
            })
            .unwrap_or((100, 1000, true));

        if let Some(row) = override_row {
            let h5_override: Option<i64> = row.try_get("rolling_5h_limit_units")?;
            let d7_override: Option<i64> = row.try_get("rolling_7d_limit_units")?;
            let enabled: bool = row.try_get("enabled")?;
            return Ok(UsageLimitPolicy {
                enabled,
                rolling_5h_limit_units: h5_override.unwrap_or(default_5h),
                rolling_7d_limit_units: d7_override.unwrap_or(default_7d),
            });
        }

        Ok(UsageLimitPolicy {
            enabled: plan_enabled,
            rolling_5h_limit_units: default_5h,
            rolling_7d_limit_units: default_7d,
        })
    }

    async fn get_user_plan(&self, org_id: Uuid) -> Result<String> {
        let row = sqlx::query(
            r#"
            SELECT plan_id FROM subscriptions
            WHERE org_id = $1 AND status = 'active'
            ORDER BY updated_at DESC LIMIT 1
            "#,
        )
        .bind(org_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row
            .and_then(|r| r.try_get::<String, _>("plan_id").ok())
            .unwrap_or_else(|| "free".to_string()))
    }

    async fn compute_windows(
        &self,
        user_id: Uuid,
        policy: &UsageLimitPolicy,
    ) -> Result<UsageWindows> {
        let now = Utc::now();
        let h5_cutoff = now - Duration::hours(5);
        let d7_cutoff = now - Duration::days(7);

        let h5_used = self.sum_usage_units_since(user_id, h5_cutoff).await?;
        let d7_used = self.sum_usage_units_since(user_id, d7_cutoff).await?;

        let h5_limit = policy.rolling_5h_limit_units;
        let d7_limit = policy.rolling_7d_limit_units;

        // 0 means unlimited
        let h5_blocked = h5_limit > 0 && h5_used >= h5_limit && policy.enabled;
        let d7_blocked = d7_limit > 0 && d7_used >= d7_limit && policy.enabled;

        let h5_next_relief = if h5_limit > 0 {
            // Find earliest event within the 5h window; relief when it falls out
            Some(self.estimate_next_relief(user_id, h5_cutoff).await?)
        } else {
            None
        };

        let d7_next_relief = if d7_limit > 0 {
            Some(self.estimate_next_relief(user_id, d7_cutoff).await?)
        } else {
            None
        };

        Ok(UsageWindows {
            rolling_5h: UsageWindow {
                used_units: h5_used,
                limit_units: h5_limit,
                remaining_units: if h5_limit > 0 {
                    (h5_limit - h5_used).max(0)
                } else {
                    i64::MAX
                },
                percent_used: if h5_limit > 0 {
                    (h5_used as f64 / h5_limit as f64) * 100.0
                } else {
                    0.0
                },
                blocked: h5_blocked,
                blocked_until: if h5_blocked {
                    h5_next_relief.clone()
                } else {
                    None
                },
                next_relief_at: h5_next_relief,
            },
            rolling_7d: UsageWindow {
                used_units: d7_used,
                limit_units: d7_limit,
                remaining_units: if d7_limit > 0 {
                    (d7_limit - d7_used).max(0)
                } else {
                    i64::MAX
                },
                percent_used: if d7_limit > 0 {
                    (d7_used as f64 / d7_limit as f64) * 100.0
                } else {
                    0.0
                },
                blocked: d7_blocked,
                blocked_until: if d7_blocked {
                    d7_next_relief.clone()
                } else {
                    None
                },
                next_relief_at: d7_next_relief,
            },
        })
    }

    async fn sum_usage_units_since(&self, user_id: Uuid, since: DateTime<Utc>) -> Result<i64> {
        let row = sqlx::query(
            r#"
            SELECT COALESCE(SUM(usage_units), 0)::bigint AS total
            FROM llm_usage_events
            WHERE user_id = $1 AND created_at >= $2
            "#,
        )
        .bind(user_id)
        .bind(since)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.try_get::<i64, _>("total")?)
    }

    async fn estimate_next_relief(
        &self,
        user_id: Uuid,
        window_start: DateTime<Utc>,
    ) -> Result<String> {
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
        .bind(window_start)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let created: DateTime<Utc> = row.try_get("created_at")?;
                // The event will fall out of the window at created_at + window_duration
                let window_duration = Utc::now() - window_start;
                let relief = created + window_duration;
                Ok(relief.to_rfc3339())
            }
            None => Ok(Utc::now().to_rfc3339()),
        }
    }

    async fn load_breakdown(&self, user_id: Uuid) -> Result<HashMap<String, i64>> {
        let since = Utc::now() - Duration::days(7);
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
        .fetch_all(&self.pool)
        .await?;

        let mut breakdown = HashMap::new();
        for row in rows {
            let feature: String = row.try_get("feature")?;
            let total: i64 = row.try_get("total")?;
            breakdown.insert(feature, total);
        }
        Ok(breakdown)
    }

    async fn load_model_rates(&self, provider: &str, model: &str) -> Result<(f64, f64)> {
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
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let input_rate = row.try_get::<f64, _>("input_unit_rate").unwrap_or(1.0);
            let output_rate = row.try_get::<f64, _>("output_unit_rate").unwrap_or(2.0);
            Ok((input_rate, output_rate))
        } else {
            Ok((1.0, 2.0))
        }
    }

    async fn determine_scope(&self, user_id: Uuid) -> Result<UsageScope> {
        let row = sqlx::query("SELECT user_id FROM usage_limit_user_overrides WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;

        if row.is_some() {
            Ok(UsageScope::UserOverride)
        } else {
            Ok(UsageScope::PlanDefault {
                plan_id: "free".to_string(),
            })
        }
    }

    async fn has_estimated_usage(&self, user_id: Uuid) -> Result<bool> {
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
        .fetch_one(&self.pool)
        .await?;

        Ok(row.try_get::<bool, _>("has_estimated")?)
    }
}
