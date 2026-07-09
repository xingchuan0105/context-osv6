use anyhow::Result;
use app_core::{UsageLimitStorePort, UsageLimitUsageRecord};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use uuid::Uuid;

use crate::usage_limit::types::{
    MeteringContext, QuotaCheckResult, UsageLimitPolicy, UsageLimitResponse, UsageScope,
    UsageSource, UsageWindow, UsageWindows,
};

pub struct UsageRecord<'a> {
    pub provider: &'a str,
    pub model: &'a str,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub usage_source: UsageSource,
}

pub struct UsageLimitService {
    store: std::sync::Arc<dyn UsageLimitStorePort>,
}

impl UsageLimitService {
    pub fn new(store: std::sync::Arc<dyn UsageLimitStorePort>) -> Self {
        Self { store }
    }

    pub async fn record_usage(
        &self,
        ctx: &MeteringContext,
        record: UsageRecord<'_>,
    ) -> Result<i64> {
        self.store
            .insert_llm_usage_event(
                ctx,
                UsageLimitUsageRecord {
                    provider: record.provider,
                    model: record.model,
                    prompt_tokens: record.prompt_tokens,
                    completion_tokens: record.completion_tokens,
                    total_tokens: record.total_tokens,
                    usage_source: record.usage_source,
                    usage_kind: "chat",
                    billable: true,
                },
            )
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))
    }

    pub async fn get_user_usage(&self, org_id: Uuid, user_id: Uuid) -> Result<UsageLimitResponse> {
        let _ = org_id;
        let policy = self.load_effective_policy(user_id).await?;
        let windows = self.compute_windows(user_id, &policy).await?;
        let breakdown = self.load_breakdown(user_id).await?;
        let scope = self.determine_scope(user_id).await?;
        let has_estimated = self
            .store
            .has_estimated_usage(user_id)
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;

        Ok(UsageLimitResponse {
            policy,
            windows,
            breakdown,
            scope,
            has_estimated_usage: has_estimated,
        })
    }

    /// Hard-cap multiplier for abuse protection (ADR 0006). Soft limit = plan limit;
    /// hard block when used ≥ limit × multiplier. Env: `USAGE_HARD_CAP_MULTIPLIER` (default 3.0).
    pub fn hard_cap_multiplier() -> f64 {
        std::env::var("USAGE_HARD_CAP_MULTIPLIER")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| *v >= 1.0)
            .unwrap_or(3.0)
    }

    pub async fn check_quota(&self, org_id: Uuid, user_id: Uuid) -> Result<QuotaCheckResult> {
        let _ = org_id;
        let policy = self.load_effective_policy(user_id).await?;
        let windows = self.compute_windows(user_id, &policy).await?;
        let mult = Self::hard_cap_multiplier();

        let hard_cap_5h = if policy.rolling_5h_limit_units > 0 {
            ((policy.rolling_5h_limit_units as f64) * mult).ceil() as i64
        } else {
            0
        };
        let hard_cap_7d = if policy.rolling_7d_limit_units > 0 {
            ((policy.rolling_7d_limit_units as f64) * mult).ceil() as i64
        } else {
            0
        };

        let soft_5h = windows.rolling_5h.blocked;
        let soft_7d = windows.rolling_7d.blocked;
        // Hard block only past abuse cap (or soft limit when mult == 1.0).
        let hard_5h = policy.enabled
            && hard_cap_5h > 0
            && windows.rolling_5h.used_units >= hard_cap_5h;
        let hard_7d = policy.enabled
            && hard_cap_7d > 0
            && windows.rolling_7d.used_units >= hard_cap_7d;

        Ok(QuotaCheckResult {
            soft_exceeded_5h: soft_5h,
            soft_exceeded_7d: soft_7d,
            blocked_5h: hard_5h,
            blocked_7d: hard_7d,
            used_5h: windows.rolling_5h.used_units,
            limit_5h: windows.rolling_5h.limit_units,
            used_7d: windows.rolling_7d.used_units,
            limit_7d: windows.rolling_7d.limit_units,
            hard_cap_5h,
            hard_cap_7d,
            blocked_until_5h: if hard_5h {
                windows.rolling_5h.blocked_until.and_then(|s| {
                    chrono::DateTime::parse_from_rfc3339(&s)
                        .ok()
                        .map(|dt| dt.with_timezone(&Utc))
                })
            } else {
                None
            },
            blocked_until_7d: if hard_7d {
                windows.rolling_7d.blocked_until.and_then(|s| {
                    chrono::DateTime::parse_from_rfc3339(&s)
                        .ok()
                        .map(|dt| dt.with_timezone(&Utc))
                })
            } else {
                None
            },
        })
    }

    async fn load_effective_policy(&self, user_id: Uuid) -> Result<UsageLimitPolicy> {
        let override_row = self.store.load_user_override(user_id).await?;

        if let Some(row) = &override_row {
            if !row.enabled {
                return Ok(UsageLimitPolicy {
                    enabled: false,
                    rolling_5h_limit_units: 0,
                    rolling_7d_limit_units: 0,
                });
            }
            if let (Some(h5), Some(d7)) = (row.rolling_5h_limit_units, row.rolling_7d_limit_units) {
                return Ok(UsageLimitPolicy {
                    enabled: true,
                    rolling_5h_limit_units: h5,
                    rolling_7d_limit_units: d7,
                });
            }
        }

        let plan_id = self.store.get_user_plan(user_id).await?;
        let plan_row = self.store.load_plan_policy(&plan_id).await?;
        let (default_5h, default_7d, plan_enabled) = plan_row
            .map(|row| {
                (
                    row.rolling_5h_limit_units,
                    row.rolling_7d_limit_units,
                    row.enabled,
                )
            })
            .unwrap_or((100, 1000, true));

        if let Some(row) = override_row {
            return Ok(UsageLimitPolicy {
                enabled: row.enabled,
                rolling_5h_limit_units: row.rolling_5h_limit_units.unwrap_or(default_5h),
                rolling_7d_limit_units: row.rolling_7d_limit_units.unwrap_or(default_7d),
            });
        }

        Ok(UsageLimitPolicy {
            enabled: plan_enabled,
            rolling_5h_limit_units: default_5h,
            rolling_7d_limit_units: default_7d,
        })
    }

    async fn compute_windows(
        &self,
        user_id: Uuid,
        policy: &UsageLimitPolicy,
    ) -> Result<UsageWindows> {
        let now = Utc::now();
        let h5_cutoff = now - Duration::hours(5);
        let d7_cutoff = now - Duration::days(7);

        let h5_used = self.store.sum_usage_units_since(user_id, h5_cutoff).await?;
        let d7_used = self.store.sum_usage_units_since(user_id, d7_cutoff).await?;

        let h5_limit = policy.rolling_5h_limit_units;
        let d7_limit = policy.rolling_7d_limit_units;

        let h5_blocked = h5_limit > 0 && h5_used >= h5_limit && policy.enabled;
        let d7_blocked = d7_limit > 0 && d7_used >= d7_limit && policy.enabled;

        let h5_next_relief = if h5_limit > 0 {
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

    async fn estimate_next_relief(
        &self,
        user_id: Uuid,
        window_start: DateTime<Utc>,
    ) -> Result<String> {
        match self
            .store
            .oldest_usage_event_since(user_id, window_start)
            .await?
        {
            Some(created) => {
                let window_duration = Utc::now() - window_start;
                Ok((created + window_duration).to_rfc3339())
            }
            None => Ok(Utc::now().to_rfc3339()),
        }
    }

    async fn load_breakdown(&self, user_id: Uuid) -> Result<HashMap<String, i64>> {
        let since = Utc::now() - Duration::days(7);
        self.store
            .load_usage_breakdown(user_id, since)
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))
    }

    async fn determine_scope(&self, user_id: Uuid) -> Result<UsageScope> {
        if self.store.has_user_override(user_id).await? {
            Ok(UsageScope::UserOverride)
        } else {
            Ok(UsageScope::PlanDefault {
                plan_id: "free".to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::UsageLimitService;

    #[test]
    fn hard_cap_multiplier_defaults_to_three() {
        // SAFETY: test-only env mutation in single-threaded unit test.
        unsafe {
            std::env::remove_var("USAGE_HARD_CAP_MULTIPLIER");
        }
        assert!((UsageLimitService::hard_cap_multiplier() - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn hard_cap_multiplier_reads_env_and_rejects_below_one() {
        unsafe {
            std::env::set_var("USAGE_HARD_CAP_MULTIPLIER", "2.5");
        }
        assert!((UsageLimitService::hard_cap_multiplier() - 2.5).abs() < f64::EPSILON);
        unsafe {
            std::env::set_var("USAGE_HARD_CAP_MULTIPLIER", "0.5");
        }
        assert!((UsageLimitService::hard_cap_multiplier() - 3.0).abs() < f64::EPSILON);
        unsafe {
            std::env::remove_var("USAGE_HARD_CAP_MULTIPLIER");
        }
    }

    #[test]
    fn hard_cap_from_limit_uses_ceil() {
        let mult = 3.0_f64;
        let limit = 100_i64;
        let hard = ((limit as f64) * mult).ceil() as i64;
        assert_eq!(hard, 300);
        let limit_odd = 101_i64;
        let hard_odd = ((limit_odd as f64) * 2.5).ceil() as i64;
        assert_eq!(hard_odd, 253);
    }

    #[test]
    fn soft_vs_hard_semantics() {
        // Soft: used >= plan limit; Hard: used >= hard_cap. Soft alone must not hard-block.
        let limit = 100_i64;
        let hard_cap = 300_i64;
        let used_soft_only = 150_i64;
        let used_hard = 300_i64;
        let soft = used_soft_only >= limit;
        let hard_soft_only = used_soft_only >= hard_cap;
        let hard = used_hard >= hard_cap;
        assert!(soft && !hard_soft_only);
        assert!(hard);
    }
}
