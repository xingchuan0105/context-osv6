//! Bound face — billing.

use app_core::{BillingStorePort, StorageContext};
use avrag_storage_pg::PgAppRepository;
use common::{ApiResponse, UserId};
use contracts::auth_runtime::AuthContext;
use std::sync::Arc;
use uuid::Uuid;


pub struct BoundBilling<'a> {
    pub(crate) auth: &'a AuthContext,
    pub(crate) storage: &'a StorageContext,
    pub(crate) postgres: Option<Arc<PgAppRepository>>,
}

impl<'a> BoundBilling<'a> {
    fn billing_store(&self) -> Option<Arc<dyn BillingStorePort>> {
        self.storage.billing_store()
    }

    fn postgres_not_configured<T>() -> ApiResponse<T> {
        ApiResponse::err(
            "postgres_not_configured",
            "postgres backend is not configured",
        )
    }

    fn auth_required<T>() -> ApiResponse<T> {
        ApiResponse::err("authenticated_user_required", "authenticated user required")
    }

    pub async fn get_plans(&self) -> ApiResponse<serde_json::Value> {
        let Some(store) = self.billing_store() else {
            return Self::postgres_not_configured();
        };
        let Some(actor_id) = self.auth.actor_id() else {
            return Self::auth_required();
        };
        avrag_billing::handle_get_plans(store, UserId::from(actor_id.into_uuid())).await
    }

    pub async fn get_subscription(&self) -> ApiResponse<avrag_billing::SubscriptionResponse> {
        let Some(store) = self.billing_store() else {
            return Self::postgres_not_configured();
        };
        let Some(actor_id) = self.auth.actor_id() else {
            return Self::auth_required();
        };
        avrag_billing::handle_get_subscription(store, UserId::from(actor_id.into_uuid())).await
    }

    pub async fn get_usage(&self) -> ApiResponse<avrag_billing::UsageResponse> {
        let Some(store) = self.billing_store() else {
            return Self::postgres_not_configured();
        };
        let Some(actor_id) = self.auth.actor_id() else {
            return Self::auth_required();
        };
        avrag_billing::handle_get_usage(store, UserId::from(actor_id.into_uuid())).await
    }

    pub async fn get_usage_window(&self) -> ApiResponse<avrag_billing::UsageWindowResponse> {
        let Some(store) = self.billing_store() else {
            return Self::postgres_not_configured();
        };
        let Some(actor_id) = self.auth.actor_id() else {
            return Self::auth_required();
        };
        avrag_billing::handle_get_usage_window(store, UserId::from(actor_id.into_uuid())).await
    }

    pub async fn get_usage_history(
        &self,
        days: i32,
    ) -> ApiResponse<avrag_billing::UsageHistoryResponse> {
        let Some(store) = self.billing_store() else {
            return Self::postgres_not_configured();
        };
        let Some(actor_id) = self.auth.actor_id() else {
            return Self::auth_required();
        };
        avrag_billing::handle_get_usage_history(store, UserId::from(actor_id.into_uuid()), days)
            .await
    }

    pub async fn get_usage_forecast(&self) -> ApiResponse<avrag_billing::UsageForecastResponse> {
        let Some(store) = self.billing_store() else {
            return Self::postgres_not_configured();
        };
        let Some(actor_id) = self.auth.actor_id() else {
            return Self::auth_required();
        };
        avrag_billing::handle_get_usage_forecast(store, UserId::from(actor_id.into_uuid())).await
    }

    pub async fn create_usage_export(
        &self,
        body: avrag_billing::CreateUsageExportRequest,
    ) -> ApiResponse<avrag_billing::UsageExportAccepted> {
        let Some(repo) = self.postgres.clone() else {
            return Self::postgres_not_configured();
        };
        let Some(actor_id) = self.auth.actor_id() else {
            return Self::auth_required();
        };
        let store: Arc<dyn app_core::UsageLimitStorePort> =
            Arc::new(crate::adapters::PgUsageLimitStoreAdapter::new(repo));
        let org_id = self.auth.org_id().into_uuid();
        let user_id = actor_id.into_uuid();
        let response =
            avrag_billing::handle_create_usage_export(store, org_id, user_id, body).await;
        if response.ok {
            if let Some(data) = response.data.as_ref() {
                tracing::info!(
                    target: "usage_export",
                    export_id = %data.export_id,
                    status = %data.status,
                    user_id = %user_id,
                    org_id = %org_id,
                    "usage export job created"
                );
            }
        }
        response
    }

    pub async fn get_usage_export(
        &self,
        export_id: Uuid,
    ) -> ApiResponse<avrag_billing::UsageExportStatusResponse> {
        let Some(repo) = self.postgres.clone() else {
            return Self::postgres_not_configured();
        };
        let Some(actor_id) = self.auth.actor_id() else {
            return Self::auth_required();
        };
        let store: Arc<dyn app_core::UsageLimitStorePort> =
            Arc::new(crate::adapters::PgUsageLimitStoreAdapter::new(repo));
        avrag_billing::handle_get_usage_export(store, actor_id.into_uuid(), export_id).await
    }

    pub async fn create_checkout(
        &self,
        body: avrag_billing::CreateCheckoutRequest,
    ) -> ApiResponse<avrag_billing::CheckoutResponse> {
        let Some(store) = self.billing_store() else {
            return Self::postgres_not_configured();
        };
        let Some(actor_id) = self.auth.actor_id() else {
            return ApiResponse::err(
                "authenticated_user_required",
                "billing checkout requires an authenticated user",
            );
        };
        let user_id = UserId::from(actor_id.into_uuid());
        if let Some(auth_store) = self.storage.auth_store() {
            match auth_store
                .has_payment_legal_acceptance(user_id.into_uuid())
                .await
            {
                Ok(true) => {}
                Ok(false) => {
                    return ApiResponse::err(
                        "consent_required",
                        "payment legal acceptance is required before checkout",
                    );
                }
                Err(error) => {
                    return ApiResponse::err(
                        "internal_error",
                        &format!("failed to verify payment legal acceptance: {error}"),
                    );
                }
            }
        }
        avrag_billing::handle_create_checkout(store, user_id, body).await
    }

    pub async fn create_portal(&self) -> ApiResponse<avrag_billing::PortalResponse> {
        let Some(store) = self.billing_store() else {
            return Self::postgres_not_configured();
        };
        let Some(actor_id) = self.auth.actor_id() else {
            return Self::auth_required();
        };
        avrag_billing::handle_create_portal(store, UserId::from(actor_id.into_uuid())).await
    }

    pub async fn handle_webhook(
        &self,
        provider: avrag_billing::BillingProvider,
        signature: Option<&str>,
        body: &[u8],
    ) -> ApiResponse<serde_json::Value> {
        let Some(store) = self.billing_store() else {
            return ApiResponse::err("billing_unavailable", "billing repository unavailable");
        };
        avrag_billing::handle_webhook(store, provider, signature, body).await
    }
}

