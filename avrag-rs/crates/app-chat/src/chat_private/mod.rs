mod memory;
mod quota;
mod visibility;

use app_core::NotificationCreateParams;
use common::AppError;

use crate::context::ChatContext;

impl ChatContext {
    pub(crate) async fn emit_notification(
        &self,
        event_type: &str,
        title: &str,
        body: &str,
        data: serde_json::Value,
    ) -> Result<(), AppError> {
        let Some(pg) = self.storage.chat_persistence() else {
            return Ok(());
        };
        let Some(user_id) = self.auth.actor_id().map(|value| value.into_uuid()) else {
            return Ok(());
        };
        pg.create_notification(
            &self.auth,
            NotificationCreateParams {
                user_id,
                event_type: event_type.to_string(),
                title: title.to_string(),
                body: body.to_string(),
                data,
                channels: vec!["in_app".to_string()],
            },
        )
        .await?;
        Ok(())
    }

    /// Record LLM token usage into the usage-limit metering service.
    /// Silently no-ops if the service is not configured.
    pub(crate) async fn record_llm_usage_if_available(
        &self,
        feature: avrag_billing::usage_limit::BillableFeature,
        stage: &str,
        usage: &avrag_llm::LlmUsage,
        source: &str,
    ) {
        let analytics_ctx = self.analytics_ctx();
        self.billing
            .record_llm_usage(&self.auth, &analytics_ctx, feature, stage, usage, source)
            .await;
    }
}

