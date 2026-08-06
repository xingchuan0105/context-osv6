mod memory;
mod quota;
mod visibility;

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use app_core::NotificationCreateParams;
use common::AppError;
use uuid::Uuid;

use crate::context::ChatContext;

/// Process-local throttle for balance-empty notices (6 hours per owner).
fn funds_notify_throttled(owner: Uuid) -> bool {
    static LAST: OnceLock<Mutex<HashMap<Uuid, Instant>>> = OnceLock::new();
    let map = LAST.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut guard) = map.lock() else {
        return false;
    };
    const COOLDOWN: Duration = Duration::from_secs(6 * 60 * 60);
    if let Some(prev) = guard.get(&owner) {
        if prev.elapsed() < COOLDOWN {
            return true;
        }
    }
    guard.insert(owner, Instant::now());
    false
}

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
    /// Throttled: process-local 6h cooldown per owner (avoids one spam row per blocked turn).
    pub(crate) async fn emit_funds_required_notification(&self) -> Result<(), AppError> {
        let Some(pg) = self.storage.chat_persistence() else {
            return Ok(());
        };
        let owner = self.auth.user_id().into_uuid();
        if owner.is_nil() {
            return Ok(());
        }
        if funds_notify_throttled(owner) {
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

