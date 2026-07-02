use std::collections::HashMap;
use std::sync::Arc;

use app_core::{
    BillingConfig, BillingProvider, BillingStorePort, PLAN_FREE, PLAN_PRO, Subscription,
    SubscriptionStatus, UsageForecastResponse, UsageHistoryResponse, UsageWindowResponse,
    WebhookClaim,
};
use async_trait::async_trait;
use avrag_billing::handle_get_subscription;
use common::{AppError, UserId};
use tokio::sync::RwLock;
use uuid::Uuid;

#[test]
fn billing_modules_do_not_call_storage_pg_escape_hatch() {
    let forbidden = concat!("storage.", "pg(");
    let sources = [
        include_str!("../src/api.rs"),
        include_str!("../src/core.rs"),
        include_str!("../src/quota_service.rs"),
    ];
    for source in sources {
        assert!(
            !source.contains(forbidden),
            "avrag-billing must use BillingStorePort, not the pg escape hatch"
        );
    }
}

fn free_subscription(user_id: UserId) -> Subscription {
    Subscription {
        id: String::new(),
        user_id: user_id.to_string(),
        stripe_subscription_id: None,
        stripe_price_id: None,
        billing_provider: BillingProvider::Stripe,
        provider_subscription_id: None,
        provider_price_id: None,
        plan_id: PLAN_FREE.to_string(),
        status: SubscriptionStatus::Active,
        current_period_start: None,
        current_period_end: None,
        cancel_at_period_end: false,
        created_at: None,
        updated_at: None,
    }
}

#[derive(Clone, Default)]
struct MemoryBillingStore {
    subscriptions: Arc<RwLock<HashMap<UserId, Subscription>>>,
}

impl MemoryBillingStore {
    fn new() -> Self {
        Self::default()
    }

    async fn seed_subscription(&self, user_id: UserId, subscription: Subscription) {
        self.subscriptions
            .write()
            .await
            .insert(user_id, subscription);
    }
}

#[async_trait]
impl BillingStorePort for MemoryBillingStore {
    async fn get_current_subscription(&self, user_id: UserId) -> Result<Subscription, AppError> {
        if let Some(subscription) = self.subscriptions.read().await.get(&user_id) {
            return Ok(subscription.clone());
        }
        Ok(free_subscription(user_id))
    }

    async fn load_plan_quotas(&self) -> Result<HashMap<String, Vec<serde_json::Value>>, AppError> {
        Ok(HashMap::new())
    }

    async fn load_usage(&self, _user_id: UserId) -> Result<HashMap<String, i64>, AppError> {
        Ok(HashMap::new())
    }

    async fn current_metric_usage(
        &self,
        _user_id: UserId,
        _metric_type: &str,
    ) -> Result<i64, AppError> {
        Ok(0)
    }

    async fn load_quota_limit(
        &self,
        _plan_id: &str,
        _metric_type: &str,
    ) -> Result<Option<(Option<i64>, Option<i64>)>, AppError> {
        Ok(None)
    }

    async fn load_customer_id(&self, _user_id: UserId) -> Result<Option<String>, AppError> {
        Ok(None)
    }

    async fn load_user_contact(&self, _user_id: UserId) -> Result<(String, String), AppError> {
        Ok((String::new(), String::new()))
    }

    async fn save_stripe_customer_id(
        &self,
        _user_id: UserId,
        _customer_id: &str,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn load_usage_window(&self, _user_id: UserId) -> Result<UsageWindowResponse, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn load_usage_history(
        &self,
        _user_id: UserId,
        _days: i32,
    ) -> Result<UsageHistoryResponse, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn load_usage_forecast(
        &self,
        _user_id: UserId,
    ) -> Result<UsageForecastResponse, AppError> {
        Err(AppError::internal("not implemented"))
    }

    async fn insert_pending_alipay_order(
        &self,
        _user_id: UserId,
        _out_trade_no: &str,
        _plan_id: &str,
        _amount_cents: i32,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn claim_webhook_with_lease(
        &self,
        _provider: BillingProvider,
        event_id: &str,
    ) -> Result<WebhookClaim, AppError> {
        Ok(WebhookClaim {
            event_id: event_id.to_string(),
            duplicate_processed: false,
        })
    }

    async fn update_webhook_lease_status(
        &self,
        _provider: BillingProvider,
        _event_id: &str,
        _status: &str,
        _error: Option<String>,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn process_webhook_event(
        &self,
        _provider: BillingProvider,
        _payload: &serde_json::Value,
        _config: &BillingConfig,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn expire_subscriptions(&self) -> Result<(), AppError> {
        Ok(())
    }

    async fn process_outbox(&self) -> Result<(), AppError> {
        Ok(())
    }
}

#[tokio::test]
async fn get_current_subscription_defaults_to_free_plan_for_unknown_user() {
    let store = Arc::new(MemoryBillingStore::new());
    let user_id = UserId::new(Uuid::new_v4());

    let response = handle_get_subscription(store, user_id).await;

    assert!(response.ok);
    let subscription = response.data.expect("subscription payload").subscription;
    assert_eq!(subscription.plan_id, PLAN_FREE);
    assert_eq!(subscription.status, SubscriptionStatus::Active);
    assert_eq!(subscription.user_id, user_id.to_string());
}

#[tokio::test]
async fn get_current_subscription_returns_stored_subscription() {
    let store = Arc::new(MemoryBillingStore::new());
    let user_id = UserId::new(Uuid::new_v4());
    store
        .seed_subscription(
            user_id,
            Subscription {
                id: "sub-contract-1".to_string(),
                user_id: user_id.to_string(),
                stripe_subscription_id: Some("sub_stripe_1".to_string()),
                stripe_price_id: Some("price_pro".to_string()),
                billing_provider: BillingProvider::Stripe,
                provider_subscription_id: Some("sub_stripe_1".to_string()),
                provider_price_id: Some("price_pro".to_string()),
                plan_id: PLAN_PRO.to_string(),
                status: SubscriptionStatus::Active,
                current_period_start: None,
                current_period_end: None,
                cancel_at_period_end: false,
                created_at: None,
                updated_at: None,
            },
        )
        .await;

    let response = handle_get_subscription(store, user_id).await;

    assert!(response.ok);
    let subscription = response.data.expect("subscription payload").subscription;
    assert_eq!(subscription.plan_id, PLAN_PRO);
    assert_eq!(subscription.id, "sub-contract-1");
}
