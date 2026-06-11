use crate::api::{QuotaDecision as MonthlyQuotaDecision, check_quota as check_monthly_quota};
use crate::usage_limit::{QuotaCheckResult as RollingQuotaResult, UsageLimitService};
use anyhow::Result;
use avrag_storage_pg::PgAppRepository;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuotaDenyReason {
    RollingWindow5h,
    RollingWindow7d,
    MonthlyLimit { metric_type: String },
}

impl fmt::Display for QuotaDenyReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RollingWindow5h => write!(f, "Rolling 5h window limit exceeded"),
            Self::RollingWindow7d => write!(f, "Rolling 7d window limit exceeded"),
            Self::MonthlyLimit { metric_type } => {
                write!(f, "Monthly limit exceeded for {}", metric_type)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct UnifiedQuotaDecision {
    pub allowed: bool,
    pub reason: Option<QuotaDenyReason>,
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
        let rolling = self.rolling_svc.check_quota(org_id, user_id).await?;
        if rolling.blocked_5h || rolling.blocked_7d {
            let (reason, until) = if rolling.blocked_5h {
                (QuotaDenyReason::RollingWindow5h, rolling.blocked_until_5h)
            } else {
                (QuotaDenyReason::RollingWindow7d, rolling.blocked_until_7d)
            };
            let retry_after = until
                .map(|dt| (dt - chrono::Utc::now()).num_seconds().max(1) as u64)
                .unwrap_or(60);

            return Ok(UnifiedQuotaDecision {
                allowed: false,
                reason: Some(reason),
                retry_after_secs: retry_after,
                rolling_result: Some(rolling),
                monthly_decision: None,
            });
        }

        // 2. Check monthly limit
        let monthly = check_monthly_quota(
            self.repo.clone(),
            common::UserId::from(user_id),
            metric_type,
            requested,
        )
        .await?;
        if !monthly.allowed {
            return Ok(UnifiedQuotaDecision {
                allowed: false,
                reason: Some(QuotaDenyReason::MonthlyLimit {
                    metric_type: metric_type.to_string(),
                }),
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
