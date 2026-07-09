use common::AppError;

use crate::context::ChatContext;

impl ChatContext {
    /// Get usage limit response for the current user.
    pub async fn get_user_usage_limit(
        &self,
    ) -> Result<avrag_billing::usage_limit::UsageLimitResponse, AppError> {
        self.billing.get_user_usage_limit(&self.auth).await
    }

    /// Check if the current user has quota remaining.
    pub async fn check_user_quota(
        &self,
    ) -> Result<avrag_billing::usage_limit::QuotaCheckResult, AppError> {
        self.billing.check_user_quota(&self.auth).await
    }

    pub(crate) async fn ensure_metric_quota(
        &self,
        metric_type: &str,
        requested: i64,
    ) -> Result<(), AppError> {
        self.billing
            .ensure_metric_quota(&self.auth, metric_type, requested)
            .await
    }

    pub(crate) async fn record_usage(
        &self,
        metric_type: &str,
        quantity: i64,
        source: &str,
    ) -> Result<(), AppError> {
        if quantity <= 0 {
            return Ok(());
        }
        let Some(pg) = self.storage.chat_persistence() else {
            return Ok(());
        };
        pg.record_usage_event(&self.auth, metric_type, quantity, source)
            .await?;
        Ok(())
    }
}
