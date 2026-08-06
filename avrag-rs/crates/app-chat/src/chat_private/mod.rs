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

    /// ADR-0010 W4: notify the **billable owner** (auth.user_id) when platform funds are empty.
    /// Frequency control: chat_persistence may create duplicates; UI treats unread stack as ok.
    pub(crate) async fn emit_funds_required_notification(&self) -> Result<(), AppError> {
        let Some(pg) = self.storage.chat_persistence() else {
            return Ok(());
        };
        let owner = self.auth.user_id().into_uuid();
        if owner.is_nil() {
            return Ok(());
        }
        pg.create_notification(
            &self.auth,
            NotificationCreateParams {
                user_id: owner,
                event_type: "billing.funds_required".to_string(),
                title: "Balance needed".to_string(),
                body: "Your balance is empty and no custom provider is configured. Top up or add a provider to continue.".to_string(),
                data: serde_json::json!({ "code": "payer_funds_required" }),
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

