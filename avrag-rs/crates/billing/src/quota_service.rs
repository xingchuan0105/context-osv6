use anyhow::Result;
use std::sync::Arc;
use uuid::Uuid;
use crate::usage_limit::{UsageLimitService, QuotaCheckResult as RollingQuotaResult};
use crate::api::{check_quota as check_monthly_quota, QuotaDecision as MonthlyQuotaDecision};
use avrag_storage_pg::PgAppRepository;

#[derive(Debug, Clone)]
pub struct UnifiedQuotaDecision {
    pub allowed: bool,
    pub reason: Option<String>,
    pub retry_after_secs: u64,
    pub rolling_result: Option<RollingQuotaResult>,
    pub monthly_decision: Option<MonthlyQuotaDecision>,
}

pub struct QuotaManager {
    rolling_svc: UsageLimitService,
    repo: Arc<PgAppRepository>,
}

impl QuotaManager {
    pub fn new(repo: Arc<PgAppRepository>) -> Self {
        Self {
            rolling_svc: UsageLimitService::new(repo.raw().clone()),
            repo,
        }
    }

    pub async fn check_quota(
        &self,
        org_id: Uuid,
        user_id: Uuid,
        metric_type: &str,
        requested: i64,
    ) -> Result<UnifiedQuotaDecision> {
        // 1. Check rolling window (LLM units)
        // Only if it's an LLM related metric, but for now we always check if we have the user context
        let rolling = self.rolling_svc.check_quota(org_id, user_id).await?;
        if rolling.blocked_5h || rolling.blocked_7d {
            let (period, until) = if rolling.blocked_5h {
                ("5h", rolling.blocked_until_5h)
            } else {
                ("7d", rolling.blocked_until_7d)
            };
            let retry_after = until
                .map(|dt| (dt - chrono::Utc::now()).num_seconds().max(1) as u64)
                .unwrap_or(60);

            return Ok(UnifiedQuotaDecision {
                allowed: false,
                reason: Some(format!("Rolling {} window limit exceeded", period)),
                retry_after_secs: retry_after,
                rolling_result: Some(rolling),
                monthly_decision: None,
            });
        }

        // 2. Check monthly limit
        let monthly = check_monthly_quota(self.repo.clone(), common::OrgId::new(org_id), metric_type, requested).await?;
        if !monthly.allowed {
            return Ok(UnifiedQuotaDecision {
                allowed: false,
                reason: Some(format!("Monthly limit exceeded for {}", metric_type)),
                retry_after_secs: monthly.retry_after_secs,
                rolling_result: Some(rolling),
                monthly_decision: Some(monthly),
            });
        }

        Ok(UnifiedQuotaDecision {
            allowed: true,
            reason: None,
            retry_after_secs: 0,
            rolling_result: Some(rolling),
            monthly_decision: Some(monthly),
        })
    }

    pub fn rolling_service(&self) -> &UsageLimitService {
        &self.rolling_svc
    }
}
