use std::sync::Arc;

use app_core::{
    AnalyticsContext, CostEventRecord as AnalyticsCostRecord, ProviderSecretPurpose,
    ProviderSecretStorePort, WalletStorePort, util::non_empty_or_unknown,
};
use avrag_billing::usage_limit::BillableFeature;
use avrag_llm::{LlmUsage, UsageObserver};
use common::AppError;
use contracts::auth_runtime::AuthContext;
use uuid::Uuid;

#[derive(Clone)]
pub struct BillingContext {
    quota_manager: Option<Arc<avrag_billing::QuotaManager>>,
    usage_limit_phase: String,
    /// Exit-metering observer for LLM clients outside UnifiedAgent (e.g. write path).
    usage_observer: Option<Arc<dyn UsageObserver>>,
    /// Optional wallet for ADR-0010 protective preflight (no free platform key ride).
    wallet: Option<Arc<dyn WalletStorePort>>,
    /// Optional BYOK secrets for protective preflight.
    provider_secrets: Option<Arc<dyn ProviderSecretStorePort>>,
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
            wallet: None,
            provider_secrets: None,
        }
    }

    pub fn with_usage_observer(mut self, observer: Arc<dyn UsageObserver>) -> Self {
        self.usage_observer = Some(observer);
        self
    }

    pub fn with_wallet(mut self, wallet: Arc<dyn WalletStorePort>) -> Self {
        self.wallet = Some(wallet);
        self
    }

    pub fn with_provider_secrets(mut self, secrets: Arc<dyn ProviderSecretStorePort>) -> Self {
        self.provider_secrets = Some(secrets);
        self
    }

    /// ADR-0010 §1.1: allow **chat/LLM** when wallet balance > 0 **or** active BYOK LLM secret.
    ///
    /// BYOK is only a valid pass because chat/write **resolve** the secret into the request
    /// (`UnifiedAgent` / writer) and set per-request skip for chat debits. Junk keys fail at
    /// the provider, not as free env-key rides.
    ///
    /// For **indexing/upload** (platform embedding + worker LLM), use
    /// [`Self::ensure_payer_has_wallet_balance`] — never trust `has_active` alone.
    ///
    /// When wallet port is absent (unit tests / memory bootstrap), allow.
    pub async fn ensure_payer_can_spend(&self, auth: &AuthContext) -> Result<(), AppError> {
        if self.wallet.is_none() && self.provider_secrets.is_none() {
            return Ok(());
        }
        let owner = auth.user_id().into_uuid();
        if owner.is_nil() {
            return Err(AppError::unauthorized("payer identity missing"));
        }
        if let Some(secrets) = &self.provider_secrets {
            if secrets
                .has_active(owner, ProviderSecretPurpose::Llm)
                .await
                .unwrap_or(false)
            {
                return Ok(());
            }
        }
        self.ensure_payer_has_wallet_balance(auth).await
    }

    /// Platform-key spend only: positive wallet balance (lazy signup grant). No BYOK shortcut.
    pub async fn ensure_payer_has_wallet_balance(
        &self,
        auth: &AuthContext,
    ) -> Result<(), AppError> {
        let Some(wallet) = &self.wallet else {
            return Ok(());
        };
        let owner = auth.user_id().into_uuid();
        if owner.is_nil() {
            return Err(AppError::unauthorized("payer identity missing"));
        }
        let _ = avrag_billing::grant_signup_bonus(wallet.clone(), owner).await;
        match wallet.ensure_wallet(owner).await {
            Ok(w) if w.balance_fen > 0 => {
                // Soft check: configured platform model should be on the price whitelist.
                Self::warn_if_platform_model_not_whitelisted();
                Ok(())
            }
            Ok(_) => Err(AppError::validation(
                "payer_funds_required",
                "Wallet balance is empty. Top up before using platform models for indexing or chat.",
            )),
            Err(e) => Err(AppError::internal(format!("wallet preflight failed: {e}"))),
        }
    }

    fn warn_if_platform_model_not_whitelisted() {
        let provider = std::env::var("AGENT_LLM_PROVIDER")
            .or_else(|_| std::env::var("LLM_PROVIDER"))
            .unwrap_or_default();
        let model = std::env::var("AGENT_LLM_MODEL")
            .or_else(|_| std::env::var("LLM_MODEL"))
            .unwrap_or_default();
        if provider.is_empty() || model.is_empty() {
            return;
        }
        if avrag_billing::official_rates_for(&provider, &model).is_none() {
            tracing::error!(
                provider = %provider,
                model = %model,
                error_code = "wallet_model_not_whitelisted",
                "platform LLM model is not on the wallet price whitelist; usage will fail-open unpaid (configure PLATFORM_OFFICIAL_RATES_JSON or change model)"
            );
        }
    }

    /// Resolve cloud BYOK LLM secret for owner (+ optional workspace scope).
    pub async fn resolve_llm_secret(
        &self,
        owner: uuid::Uuid,
        workspace_id: Option<uuid::Uuid>,
    ) -> Option<app_core::ResolvedProviderSecret> {
        let secrets = self.provider_secrets.as_ref()?;
        match secrets
            .resolve(owner, workspace_id, ProviderSecretPurpose::Llm)
            .await
        {
            Ok(Some(s)) => Some(s),
            Ok(None) => None,
            Err(e) => {
                tracing::warn!(error = %e, owner = %owner, "BYOK resolve failed");
                None
            }
        }
    }

    /// ADR-0010 §4/§9: Owner daily platform-spend fuse for share chat (fen).
    ///
    /// `SHARE_OWNER_DAILY_BUDGET_FEN` — default 5000 (¥50). Set `0` to disable.
    /// Sums wallet `usage_debit` fen in the last 24h (SQL aggregate; not ledger page sample).
    /// Note: includes private platform spend for the same owner (strict fuse).
    pub async fn ensure_share_owner_daily_budget(
        &self,
        auth: &AuthContext,
    ) -> Result<(), AppError> {
        let cap_fen: i64 = std::env::var("SHARE_OWNER_DAILY_BUDGET_FEN")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5000);
        if cap_fen <= 0 {
            return Ok(());
        }
        let Some(wallet) = &self.wallet else {
            return Ok(());
        };
        let owner = auth.user_id().into_uuid();
        if owner.is_nil() {
            return Ok(());
        }
        let cutoff = chrono::Utc::now() - chrono::Duration::hours(24);
        let spent_fen = wallet
            .sum_usage_debit_fen_since(owner, cutoff)
            .await
            .map_err(|e| {
                AppError::internal(format!("share daily budget ledger read failed: {e}"))
            })?;
        if spent_fen >= cap_fen {
            return Err(AppError::rate_limited(
                "share_owner_daily_budget_exceeded",
                format!(
                    "Share owner platform spend in the last 24h is {spent_fen} fen, at or above cap {cap_fen} fen. Raise SHARE_OWNER_DAILY_BUDGET_FEN, top up later, or use cloud BYOK."
                ),
                3600,
            ));
        }
        Ok(())
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
        let owner_user_id = auth.user_id().into_uuid();
        qm.rolling_service()
            .get_user_usage(owner_user_id, user_id)
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
        let owner_user_id = auth.user_id().into_uuid();
        qm.rolling_service()
            .check_quota(owner_user_id, user_id)
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
            .check_quota(
                auth.user_id().into_uuid(),
                user_uuid,
                metric_type,
                requested,
            )
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
            let owner_user_id = auth.user_id().into_uuid();
            let ctx = avrag_billing::usage_limit::MeteringContext {
                user_id,
                owner_user_id,
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
                workspace_id: None,
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
