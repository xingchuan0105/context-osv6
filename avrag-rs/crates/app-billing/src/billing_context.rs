use std::sync::Arc;

use app_core::{
    AnalyticsContext, CostEventRecord as AnalyticsCostRecord, util::non_empty_or_unknown,
};
use contracts::auth_runtime::AuthContext;
use avrag_billing::usage_limit::BillableFeature;
use avrag_llm::{LlmUsage, UsageObserver};
use common::AppError;
use uuid::Uuid;

#[derive(Clone)]
pub struct BillingContext {
    quota_manager: Option<Arc<avrag_billing::QuotaManager>>,
    usage_limit_phase: String,
    /// Exit-metering observer for LLM clients outside UnifiedAgent (e.g. write path).
    usage_observer: Option<Arc<dyn UsageObserver>>,
}

impl BillingContext {
    pub fn new(
        quota_manager: Option<Arc<avrag_billing::QuotaManager>>,
        usage_limit_phase: String,
    ) -> Self {
        Self {
            quota_manager,
            usage_limit_phase,
            usage_observer: None,
        }
    }

    pub fn with_usage_observer(mut self, observer: Arc<dyn UsageObserver>) -> Self {
        self.usage_observer = Some(observer);
        self
    }

    pub fn is_available(&self) -> bool {
        self.quota_manager.is_some()
    }

    pub fn usage_limit_phase(&self) -> &str {
        &self.usage_limit_phase
    }

    pub fn quota_manager(&self) -> Option<&Arc<avrag_billing::QuotaManager>> {
        self.quota_manager.as_ref()
    }

    pub fn usage_observer(&self) -> Option<&Arc<dyn UsageObserver>> {
        self.usage_observer.as_ref()
    }

    pub async fn get_user_usage_limit(
        &self,
        auth: &AuthContext,
    ) -> Result<avrag_billing::usage_limit::UsageLimitResponse, AppError> {
        let Some(ref qm) = self.quota_manager else {
            return Err(AppError::internal("quota service not configured"));
        };
        let user_id = auth
            .actor_id()
            .map(|a| a.into_uuid())
            .ok_or_else(|| AppError::internal("no authenticated user"))?;
        let org_id = auth.org_id().into_uuid();
        qm.rolling_service()
            .get_user_usage(org_id, user_id)
            .await
            .map_err(|e| AppError::internal(format!("failed to get usage limit: {}", e)))
    }

    pub async fn check_user_quota(
        &self,
        auth: &AuthContext,
    ) -> Result<avrag_billing::usage_limit::QuotaCheckResult, AppError> {
        let Some(ref qm) = self.quota_manager else {
            return Err(AppError::internal("quota service not configured"));
        };
        let user_id = auth
            .actor_id()
            .map(|a| a.into_uuid())
            .unwrap_or_else(Uuid::nil);
        let org_id = auth.org_id().into_uuid();
        qm.rolling_service()
            .check_quota(org_id, user_id)
            .await
            .map_err(|e| AppError::internal(format!("usage limit check failed: {}", e)))
    }

    pub async fn ensure_metric_quota(
        &self,
        auth: &AuthContext,
        metric_type: &str,
        requested: i64,
    ) -> Result<(), AppError> {
        if requested <= 0 {
            return Ok(());
        }
        let Some(ref qm) = self.quota_manager else {
            return Ok(());
        };
        let user_uuid = auth
            .actor_id()
            .map(|v| v.into_uuid())
            .unwrap_or_else(Uuid::nil);
        let decision = qm
            .check_quota(auth.org_id().into_uuid(), user_uuid, metric_type, requested)
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;

        if decision.allowed {
            return Ok(());
        }

        let error_message = decision
            .reason
            .as_ref()
            .map(|reason| reason.to_string())
            .unwrap_or_else(|| format!("quota exceeded for {}", metric_type));

        Err(AppError::rate_limited(
            "quota_exceeded",
            error_message,
            decision.retry_after_secs,
        ))
    }

    pub async fn record_llm_usage(
        &self,
        auth: &AuthContext,
        analytics: &AnalyticsContext,
        feature: BillableFeature,
        stage: &str,
        usage: &LlmUsage,
        source: &str,
    ) {
        if let Some(ref qm) = self.quota_manager {
            let user_id = auth
                .actor_id()
                .map(|a| a.into_uuid())
                .unwrap_or_else(Uuid::nil);
            let org_id = auth.org_id().into_uuid();
            let ctx = avrag_billing::usage_limit::MeteringContext {
                user_id,
                org_id,
                feature,
                stage: stage.to_string(),
                session_id: None,
                document_id: None,
                request_id: auth.request_id().map(|s| s.to_string()),
                trace_id: None,
            };
            let _ = qm
                .rolling_service()
                .record_usage(
                    &ctx,
                    avrag_billing::usage_limit::UsageRecord {
                        provider: &non_empty_or_unknown(&usage.provider),
                        model: &non_empty_or_unknown(&usage.model),
                        prompt_tokens: usage.prompt_tokens,
                        completion_tokens: usage.completion_tokens,
                        total_tokens: usage.total_tokens,
                        usage_source: avrag_billing::usage_limit::UsageSource::Actual,
                    },
                )
                .await;
        }
        analytics
            .record_cost_event(AnalyticsCostRecord {
                event_name: analytics::CostEventName::LlmUsageMetered,
                feature: feature.as_str(),
                session_id: None,
                notebook_id: None,
                usage,
                source,
                metadata: serde_json::json!({
                    "stage": stage,
                    "feature": feature.as_str(),
                }),
            })
            .await;
    }
}
