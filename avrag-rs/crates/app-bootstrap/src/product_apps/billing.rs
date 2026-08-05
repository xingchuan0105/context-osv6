//! Product App — Billing (ADR-0007 / ADR-0010 wallet + BYOK secrets).

use app_core::{
    BillingStorePort, ByokMasterKey, ProviderSecretStorePort, ReferralStorePort, StorageContext,
    WalletStorePort,
};
use avrag_storage_pg::PgAppRepository;
use common::{ApiResponse, UserId};
use contracts::auth_runtime::AuthContext;
use std::sync::Arc;
use uuid::Uuid;

pub struct BillingApp<'a> {
    pub(crate) auth: &'a AuthContext,
    pub(crate) storage: &'a StorageContext,
    pub(crate) postgres: Option<Arc<PgAppRepository>>,
}

impl<'a> BillingApp<'a> {
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
        let owner_user_id = self.auth.user_id().into_uuid();
        let user_id = actor_id.into_uuid();
        let response =
            avrag_billing::handle_create_usage_export(store, owner_user_id, user_id, body).await;
        if response.ok {
            if let Some(data) = response.data.as_ref() {
                tracing::info!(
                    target: "usage_export",
                    export_id = %data.export_id,
                    status = %data.status,
                    user_id = %user_id,
                    owner_user_id = %owner_user_id,
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

    pub async fn get_order_status(
        &self,
        order_id: &str,
    ) -> ApiResponse<avrag_billing::OrderStatusResponse> {
        let Some(store) = self.billing_store() else {
            return Self::postgres_not_configured();
        };
        let Some(actor_id) = self.auth.actor_id() else {
            return Self::auth_required();
        };
        avrag_billing::handle_get_order_status(store, UserId::from(actor_id.into_uuid()), order_id)
            .await
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

    /// Wallet balance for the **account owner** (fen / 分). ADR-0010.
    pub async fn get_wallet(&self) -> ApiResponse<avrag_billing::WalletBalanceResponse> {
        let Some(repo) = self.postgres.clone() else {
            return Self::postgres_not_configured();
        };
        // Prefer account owner (`user_id`); fall back to actor for legacy callers.
        let owner = self.auth.user_id().into_uuid();
        if owner.is_nil() {
            return Self::auth_required();
        }
        let store: Arc<dyn WalletStorePort> =
            Arc::new(crate::adapters::PgWalletStoreAdapter::new(repo));
        avrag_billing::handle_get_wallet(store, owner).await
    }

    /// Fixed wallet top-up packs (ADR-0010 PR5).
    pub async fn list_topup_packs(&self) -> ApiResponse<Vec<avrag_billing::TopupPackResponse>> {
        let Some(actor_id) = self.auth.actor_id() else {
            return Self::auth_required();
        };
        let _ = actor_id;
        avrag_billing::handle_list_topup_packs()
    }

    /// My referral code + quota stats (ADR-0010 PR4).
    pub async fn get_referral(&self) -> ApiResponse<avrag_billing::ReferralStatsResponse> {
        let Some(repo) = self.postgres.clone() else {
            return Self::postgres_not_configured();
        };
        let Some(actor_id) = self.auth.actor_id() else {
            return Self::auth_required();
        };
        let wallet: Arc<dyn WalletStorePort> =
            Arc::new(crate::adapters::PgWalletStoreAdapter::new(repo.clone()));
        let referral: Arc<dyn ReferralStorePort> =
            Arc::new(crate::adapters::PgReferralStoreAdapter::new(repo));
        avrag_billing::handle_get_referral(wallet, referral, actor_id.into_uuid()).await
    }

    fn provider_secret_store(&self) -> Result<Arc<dyn ProviderSecretStorePort>, common::AppError> {
        let repo = self.postgres.clone().ok_or_else(|| {
            common::AppError::validation(
                "postgres_not_configured",
                "postgres backend is not configured",
            )
        })?;
        let master = ByokMasterKey::from_env()?;
        Ok(Arc::new(crate::adapters::PgProviderSecretStoreAdapter::new(
            repo, master,
        )) as Arc<dyn ProviderSecretStorePort>)
    }

    /// Upsert encrypted cloud BYOK secret (ADR-0010 PR7). Response is fingerprint-only.
    pub async fn upsert_provider_secret(
        &self,
        body: avrag_billing::UpsertProviderSecretRequest,
    ) -> ApiResponse<avrag_billing::ProviderSecretResponse> {
        let Some(actor_id) = self.auth.actor_id() else {
            return Self::auth_required();
        };
        let store = match self.provider_secret_store() {
            Ok(s) => s,
            Err(e) => return ApiResponse::err(e.code(), e.message()),
        };
        avrag_billing::handle_upsert_provider_secret(store, actor_id.into_uuid(), body).await
    }

    /// List cloud BYOK secrets (fingerprints only).
    pub async fn list_provider_secrets(
        &self,
        include_revoked: bool,
    ) -> ApiResponse<avrag_billing::ProviderSecretListResponse> {
        let Some(actor_id) = self.auth.actor_id() else {
            return Self::auth_required();
        };
        let store = match self.provider_secret_store() {
            Ok(s) => s,
            Err(e) => return ApiResponse::err(e.code(), e.message()),
        };
        avrag_billing::handle_list_provider_secrets(store, actor_id.into_uuid(), include_revoked)
            .await
    }

    /// Revoke a cloud BYOK secret (soft). Resolve will stop returning it.
    pub async fn revoke_provider_secret(
        &self,
        id: Uuid,
    ) -> ApiResponse<avrag_billing::ProviderSecretResponse> {
        let Some(actor_id) = self.auth.actor_id() else {
            return Self::auth_required();
        };
        let store = match self.provider_secret_store() {
            Ok(s) => s,
            Err(e) => return ApiResponse::err(e.code(), e.message()),
        };
        avrag_billing::handle_revoke_provider_secret(store, actor_id.into_uuid(), id).await
    }
}
