use std::sync::Arc;

use crate::adapters::pg_session::set_current_role;
use app_core::BillingStorePort;
use app_core::{
    BillingConfig, BillingProvider, Subscription, UsageForecastResponse, UsageHistoryResponse,
    UsageWindowResponse, WebhookClaim,
};
use async_trait::async_trait;
use avrag_storage_pg::PgAppRepository;
use common::{AppError, UserId};
use sqlx::Row;
use std::collections::HashMap;

mod billing_sql {
    use std::collections::HashMap;
    use std::sync::Arc;

    use anyhow::{Result, anyhow, bail};
    use app_core::{
        ADMIN_ROLE_SUPER, BillingConfig, BillingProvider, DailyUsage, ExistingSubscriptionFields,
        LimitHits, PLAN_FREE, PLAN_PLUS, PLAN_PRO, STATUS_ACTIVE, STATUS_CANCELED, STATUS_PAST_DUE,
        STATUS_UNPAID, StripeSubscriptionSnapshot, Subscription, SubscriptionStatus,
        UsageForecastResponse, UsageHistoryResponse, UsageWindowBucket, UsageWindowResponse,
        WebhookClaim,
    };
    use avrag_storage_pg::PgAppRepository;
    use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};
    use common::UserId;
    use sqlx::Row;
    use uuid::Uuid;

    use crate::adapters::pg_session::{set_current_role_sqlx, set_current_user_sqlx};

    pub(super) async fn set_current_user(
        conn: &mut sqlx::PgConnection,
        user_id: &str,
    ) -> Result<()> {
        set_current_user_sqlx(conn, user_id).await?;
        Ok(())
    }

    pub(super) async fn set_current_role(conn: &mut sqlx::PgConnection, role: &str) -> Result<()> {
        set_current_role_sqlx(conn, role).await?;
        Ok(())
    }

    include!("billing_sql/core_support.rs");
    include!("billing_sql/core_usage.rs");
    include!("billing_sql/core_webhooks.rs");
}

pub struct PgBillingStoreAdapter {
    repo: Arc<PgAppRepository>,
}

impl PgBillingStoreAdapter {
    pub fn new(repo: Arc<PgAppRepository>) -> Self {
        Self { repo }
    }
}

fn map_err(error: anyhow::Error) -> AppError {
    AppError::internal(error.to_string())
}

#[async_trait]
impl BillingStorePort for PgBillingStoreAdapter {
    async fn get_current_subscription(&self, user_id: UserId) -> Result<Subscription, AppError> {
        billing_sql::get_current_subscription(self.repo.clone(), user_id)
            .await
            .map_err(map_err)
    }

    async fn load_plan_quotas(&self) -> Result<HashMap<String, Vec<serde_json::Value>>, AppError> {
        billing_sql::load_plan_quotas(self.repo.clone())
            .await
            .map_err(map_err)
    }

    async fn load_usage(&self, user_id: UserId) -> Result<HashMap<String, i64>, AppError> {
        billing_sql::load_usage(self.repo.clone(), user_id)
            .await
            .map_err(map_err)
    }

    async fn current_metric_usage(
        &self,
        user_id: UserId,
        metric_type: &str,
    ) -> Result<i64, AppError> {
        billing_sql::current_metric_usage(self.repo.clone(), user_id, metric_type)
            .await
            .map_err(map_err)
    }

    async fn load_quota_limit(
        &self,
        plan_id: &str,
        metric_type: &str,
    ) -> Result<Option<(Option<i64>, Option<i64>)>, AppError> {
        billing_sql::load_quota_limit(self.repo.clone(), plan_id, metric_type)
            .await
            .map_err(map_err)
    }

    async fn load_customer_id(&self, user_id: UserId) -> Result<Option<String>, AppError> {
        billing_sql::load_customer_id(self.repo.clone(), user_id)
            .await
            .map_err(map_err)
    }

    async fn load_user_contact(&self, user_id: UserId) -> Result<(String, String), AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        billing_sql::set_current_user(tx.as_mut(), &user_id.to_string())
            .await
            .map_err(map_err)?;
        let row = sqlx::query(
            r#"
            select name, email
            from users
            where id = $1
            "#,
        )
        .bind(user_id.into_uuid())
        .fetch_one(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok((
            row.try_get::<String, _>("name")
                .map_err(|error| AppError::internal(error.to_string()))?,
            row.try_get::<Option<String>, _>("email")
                .ok()
                .flatten()
                .unwrap_or_else(|| "billing@context.local".to_string()),
        ))
    }

    async fn save_stripe_customer_id(
        &self,
        user_id: UserId,
        customer_id: &str,
    ) -> Result<(), AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        billing_sql::set_current_user(tx.as_mut(), &user_id.to_string())
            .await
            .map_err(map_err)?;
        sqlx::query("update users set stripe_customer_id = $2 where id = $1")
            .bind(user_id.into_uuid())
            .bind(customer_id)
            .execute(tx.as_mut())
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(())
    }

    async fn load_usage_window(&self, user_id: UserId) -> Result<UsageWindowResponse, AppError> {
        billing_sql::load_usage_window(self.repo.clone(), user_id)
            .await
            .map_err(map_err)
    }

    async fn load_usage_history(
        &self,
        user_id: UserId,
        days: i32,
    ) -> Result<UsageHistoryResponse, AppError> {
        billing_sql::load_usage_history(self.repo.clone(), user_id, days)
            .await
            .map_err(map_err)
    }

    async fn load_usage_forecast(
        &self,
        user_id: UserId,
    ) -> Result<UsageForecastResponse, AppError> {
        billing_sql::load_usage_forecast(self.repo.clone(), user_id)
            .await
            .map_err(map_err)
    }

    async fn insert_pending_alipay_order(
        &self,
        user_id: UserId,
        out_trade_no: &str,
        plan_id: &str,
        amount_cents: i32,
    ) -> Result<(), AppError> {
        let mut tx = self
            .repo
            .raw()
            .begin()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        set_current_role(tx.as_mut(), "super_admin").await?;
        sqlx::query(
            r#"
            insert into billing_orders (user_id, provider, provider_order_id, plan_id, status, amount_cents, currency)
            values ($1, 'alipay', $2, $3, 'pending', $4, 'CNY')
            "#,
        )
        .bind(user_id.into_uuid())
        .bind(out_trade_no)
        .bind(plan_id)
        .bind(amount_cents)
        .execute(tx.as_mut())
        .await
        .map_err(|error| AppError::internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
        Ok(())
    }

    async fn claim_webhook_with_lease(
        &self,
        provider: BillingProvider,
        event_id: &str,
    ) -> Result<WebhookClaim, AppError> {
        billing_sql::claim_webhook_with_lease(self.repo.clone(), provider, event_id)
            .await
            .map_err(map_err)
    }

    async fn update_webhook_lease_status(
        &self,
        provider: BillingProvider,
        event_id: &str,
        status: &str,
        error: Option<String>,
    ) -> Result<(), AppError> {
        billing_sql::update_webhook_lease_status(
            self.repo.clone(),
            provider,
            event_id,
            status,
            error,
        )
        .await
        .map_err(map_err)
    }

    async fn process_webhook_event(
        &self,
        provider: BillingProvider,
        payload: &serde_json::Value,
        config: &BillingConfig,
    ) -> Result<(), AppError> {
        billing_sql::process_webhook_event(self.repo.clone(), provider, payload, config)
            .await
            .map_err(map_err)
    }

    async fn expire_subscriptions(&self) -> Result<(), AppError> {
        billing_sql::expire_subscriptions(self.repo.clone())
            .await
            .map_err(map_err)
    }

    async fn process_outbox(&self) -> Result<(), AppError> {
        billing_sql::process_outbox(self.repo.clone())
            .await
            .map_err(map_err)
    }
}
