mod memory;
mod quota;
mod visibility;

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use app_core::NotificationCreateParams;
use avrag_rag_core_ports::CachePort;
use common::AppError;
use uuid::Uuid;

use crate::context::ChatContext;

/// Shared 6h throttle for funds-required notices (wired once at bootstrap).
static SHARED_THROTTLE: std::sync::OnceLock<Option<std::sync::Arc<dyn CachePort>>> =
    std::sync::OnceLock::new();

pub fn init_funds_notify_cache(cache: Option<std::sync::Arc<dyn CachePort>>) {
    let _ = SHARED_THROTTLE.set(cache);
}

/// Process-local throttle for balance-empty notices (6 hours per owner).
/// With Redis wired: cross-replica SET NX EX cooldown; memory is the fallback.
async fn funds_notify_throttled(owner: Uuid) -> bool {
    const COOLDOWN_SECS: u64 = 6 * 60 * 60;
    if let Some(Some(cache)) = SHARED_THROTTLE.get() {
        let key = format!("funds-notify:{owner}");
        match cache.get(&key).await {
            Some(_) => return true,
            None => {
                let _ = cache.set(&key, "1", COOLDOWN_SECS).await;
                return false;
            }
        }
    }
    static LAST: OnceLock<Mutex<HashMap<Uuid, Instant>>> = OnceLock::new();
    let map = LAST.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut guard) = map.lock() else {
        return false;
    };
    const COOLDOWN: Duration = Duration::from_secs(COOLDOWN_SECS);
    if let Some(prev) = guard.get(&owner) {
        if prev.elapsed() < COOLDOWN {
            return true;
        }
    }
    guard.insert(owner, Instant::now());
    false
}

impl ChatContext {
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
        if funds_notify_throttled(owner).await {
            return Ok(());
        }
        let copy = common::notification_copy::render(
            common::notification_copy::NotifyKind::FundsRequired,
            common::notification_copy::NotifyLocale::product_default(),
        );
        pg.create_notification(
            &self.auth,
            NotificationCreateParams {
                user_id: owner,
                event_type: "billing.funds_required".to_string(),
                title: copy.title,
                body: copy.body,
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

