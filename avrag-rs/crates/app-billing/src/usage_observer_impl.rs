//! Postgres-backed exit-metering observer for LLM / embedding calls.
//!
//! # Wallet debit (ADR-0010 PR6)
//!
//! When a [`WalletStorePort`] is attached and the row is billable, the observer
//! debits the payer wallet after a successful `llm_usage_events` insert:
//!
//! - `list_fen = ceil(official * 1.5)` via [`avrag_billing::list_price_fen`]
//! - ledger kind `usage_debit` (negative fen)
//! - idempotency key `usage_debit:{event_id}`
//!
//! ## Skip rules
//!
//! | Condition | Debit? |
//! |-----------|--------|
//! | no wallet store wired | no |
//! | `billable = false` (worker path) | no |
//! | `skip_wallet_debit = true` (BYOK / billing_mode) | no |
//! | list_fen == 0 | no |
//!
//! ## Insufficient balance
//!
//! Debit failures (including `wallet_insufficient_balance`) are logged at
//! **error** level and do **not** fail the LLM path (UsageObserver is
//! fail-open). Balance is never driven negative by the store. A pre-flight
//! hard-stop is out of scope for PR6; rolling token walls remain interim
//! protection.

use std::sync::Arc;

use app_core::{
    BillableFeature, MeteringContext, UsageLimitStorePort, UsageLimitUsageRecord, UsageSource,
    WalletStorePort,
};
use async_trait::async_trait;
use avrag_billing::{UsageDebitInput, debit_platform_usage};
use avrag_llm::{ChatUsageRecord, EmbeddingUsageRecord, TenantContext, UsageObserver};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Writes exit-metered usage into `llm_usage_events` via [`UsageLimitStorePort`],
/// and optionally debits the user wallet for platform-proxy spend.
#[derive(Clone)]
pub struct PgUsageObserver {
    store: Arc<dyn UsageLimitStorePort>,
    /// When false, rows do not count toward user rolling quotas (ADR 0006 §7 worker path).
    billable: bool,
    /// Optional wallet store for platform-proxy usage debits (PR6).
    wallet: Option<Arc<dyn WalletStorePort>>,
    /// When true, skip wallet debit even if billable (BYOK / billing_mode flag).
    /// Default `false` → debit platform path.
    skip_wallet_debit: bool,
}

impl std::fmt::Debug for PgUsageObserver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PgUsageObserver")
            .field("billable", &self.billable)
            .field("has_wallet", &self.wallet.is_some())
            .field("skip_wallet_debit", &self.skip_wallet_debit)
            .finish_non_exhaustive()
    }
}

impl PgUsageObserver {
    pub fn new(store: Arc<dyn UsageLimitStorePort>) -> Self {
        Self {
            store,
            billable: true,
            wallet: None,
            skip_wallet_debit: false,
        }
    }

    pub fn with_billable(mut self, billable: bool) -> Self {
        self.billable = billable;
        self
    }

    /// Attach a wallet store so billable platform-proxy usage debits the payer.
    pub fn with_wallet(mut self, wallet: Arc<dyn WalletStorePort>) -> Self {
        self.wallet = Some(wallet);
        self
    }

    /// BYOK / billing_mode: when true, metering still writes usage events but
    /// never debits the platform wallet (default false = platform path debits).
    pub fn with_skip_wallet_debit(mut self, skip: bool) -> Self {
        self.skip_wallet_debit = skip;
        self
    }

    /// Test/diagnostics: whether recorded rows count toward user rolling quotas.
    #[cfg(test)]
    pub(crate) fn is_billable(&self) -> bool {
        self.billable
    }

    #[cfg(test)]
    pub(crate) fn skips_wallet_debit(&self) -> bool {
        self.skip_wallet_debit
    }

    #[cfg(test)]
    pub(crate) fn has_wallet(&self) -> bool {
        self.wallet.is_some()
    }

    /// Map free-text feature tags set by `LlmClient::with_feature` to billable buckets.
    ///
    /// Prefer **exact / prefix** matches over substring `contains`, so tags like
    /// `planner` / `agent_loop` / `write:refine` stay deterministic.
    pub fn map_feature(feature: &str) -> BillableFeature {
        let f = feature.trim().to_ascii_lowercase();
        if f.is_empty() {
            return BillableFeature::Chat;
        }
        // Exact tags first.
        match f.as_str() {
            "summary" | "document_summary" => return BillableFeature::Summary,
            "planner" | "plan" | "retrieval_planner" => return BillableFeature::Planner,
            "search" | "web_search" => return BillableFeature::Search,
            "triplet" | "graph" | "graph_extraction" => {
                return BillableFeature::GraphExtraction;
            }
            "rag" | "answer" | "internal_answer" => return BillableFeature::Answer,
            "chat" | "agent_loop" | "section_index" | "ingestion" | "heavytail_writer" => {
                return BillableFeature::Chat;
            }
            "document_embedding" | "document_embedding_mm" | "embedding" => {
                // Embeddings roll under answer/RAG product meter today.
                return BillableFeature::Answer;
            }
            _ => {}
        }
        // Prefix tags (write phases, namespaced features).
        if f.starts_with("write:") || f.starts_with("write_") {
            return BillableFeature::Chat;
        }
        if f.starts_with("summary") {
            return BillableFeature::Summary;
        }
        if f.starts_with("planner") || f.starts_with("plan:") {
            return BillableFeature::Planner;
        }
        if f.starts_with("search") {
            return BillableFeature::Search;
        }
        if f.starts_with("triplet") || f.starts_with("graph") {
            return BillableFeature::GraphExtraction;
        }
        if f.starts_with("rag") || f.starts_with("answer") {
            return BillableFeature::Answer;
        }
        if f.starts_with("embedding") || f.contains("embedding") {
            return BillableFeature::Answer;
        }
        BillableFeature::Chat
    }

    /// Payer for wallet debit: metering user when set, else owner.
    fn payer_user_id(tenant: &TenantContext) -> Uuid {
        if tenant.user_id.is_nil() {
            tenant.owner_user_id
        } else {
            tenant.user_id
        }
    }

    async fn maybe_debit_wallet(
        &self,
        tenant: &TenantContext,
        provider: &str,
        model: &str,
        prompt_tokens: u32,
        completion_tokens: u32,
        cached_tokens: u32,
        usage_kind: &str,
        request_id: Option<String>,
    ) {
        if !self.billable || self.skip_wallet_debit {
            return;
        }
        let Some(wallet) = self.wallet.as_ref() else {
            return;
        };

        let event_id = Uuid::new_v4();
        let input = UsageDebitInput {
            user_id: Self::payer_user_id(tenant),
            provider: provider.to_string(),
            model: model.to_string(),
            prompt_tokens,
            completion_tokens,
            cached_tokens,
            usage_kind: usage_kind.to_string(),
            event_id,
            request_id,
        };

        match debit_platform_usage(wallet.clone(), &input).await {
            Ok(Some(result)) => {
                tracing::debug!(
                    user_id = %input.user_id,
                    event_id = %event_id,
                    applied = result.applied,
                    balance_fen = result.wallet.balance_fen,
                    "platform usage wallet debit applied"
                );
            }
            Ok(None) => {
                // zero fen — nothing to bill
            }
            Err(e) => {
                // Fail-open for the LLM path; loud log so ops see free-ride risk.
                tracing::error!(
                    user_id = %input.user_id,
                    owner_user_id = %tenant.owner_user_id,
                    event_id = %event_id,
                    provider = %provider,
                    model = %model,
                    prompt_tokens,
                    completion_tokens,
                    error = %e,
                    "PgUsageObserver wallet debit failed; usage recorded but not charged"
                );
            }
        }
    }

    pub async fn record_chat_for(&self, tenant: &TenantContext, record: &ChatUsageRecord) {
        let ctx = MeteringContext {
            user_id: tenant.user_id,
            owner_user_id: tenant.owner_user_id,
            feature: Self::map_feature(&record.feature),
            stage: if record.stage.is_empty() {
                record.feature.clone()
            } else {
                record.stage.clone()
            },
            session_id: record.session_id,
            document_id: record.document_id,
            request_id: record.request_id.clone(),
            trace_id: record.trace_id.clone(),
        };
        let usage = UsageLimitUsageRecord {
            provider: &record.provider,
            model: &record.model,
            prompt_tokens: record.prompt_tokens,
            completion_tokens: record.completion_tokens,
            total_tokens: record.total_tokens,
            cached_tokens: record.cached_tokens,
            reasoning_tokens: record.reasoning_tokens,
            usage_source: UsageSource::Actual,
            usage_kind: "chat",
            billable: self.billable,
        };
        match self.store.insert_llm_usage_event(&ctx, usage).await {
            Ok(_) => {
                self.maybe_debit_wallet(
                    tenant,
                    &record.provider,
                    &record.model,
                    record.prompt_tokens,
                    record.completion_tokens,
                    record.cached_tokens,
                    "chat",
                    record.request_id.clone(),
                )
                .await;
            }
            Err(e) => {
                tracing::warn!(
                    owner_user_id = %tenant.owner_user_id,
                    user_id = %tenant.user_id,
                    error = %e,
                    "PgUsageObserver::record_chat failed; continuing"
                );
            }
        }
    }

    pub async fn record_embedding_for(
        &self,
        tenant: &TenantContext,
        record: &EmbeddingUsageRecord,
    ) {
        let usage_kind = if record.actual_tokens.is_some() {
            "embedding_multimodal"
        } else {
            "embedding_text"
        };
        let usage_source = if record.actual_tokens.is_some() {
            UsageSource::Actual
        } else {
            UsageSource::Estimated
        };
        let total_tokens = record.actual_tokens.unwrap_or(record.estimated_tokens);
        let ctx = MeteringContext {
            user_id: tenant.user_id,
            owner_user_id: tenant.owner_user_id,
            feature: Self::map_feature(&record.feature),
            stage: "embedding".to_string(),
            session_id: None,
            document_id: None,
            request_id: None,
            trace_id: None,
        };
        let usage = UsageLimitUsageRecord {
            provider: &record.provider,
            model: &record.model,
            prompt_tokens: total_tokens,
            completion_tokens: 0,
            total_tokens,
            cached_tokens: 0,
            reasoning_tokens: 0,
            usage_source,
            usage_kind,
            billable: self.billable,
        };
        match self.store.insert_llm_usage_event(&ctx, usage).await {
            Ok(_) => {
                self.maybe_debit_wallet(
                    tenant,
                    &record.provider,
                    &record.model,
                    total_tokens,
                    0,
                    0,
                    usage_kind,
                    None,
                )
                .await;
            }
            Err(e) => {
                tracing::warn!(
                    owner_user_id = %tenant.owner_user_id,
                    user_id = %tenant.user_id,
                    error = %e,
                    "PgUsageObserver::record_embedding failed; continuing"
                );
            }
        }
    }
}

#[async_trait]
impl UsageObserver for PgUsageObserver {
    async fn record_chat(&self, tenant: &TenantContext, record: &ChatUsageRecord) {
        self.record_chat_for(tenant, record).await;
    }

    async fn record_embedding(&self, tenant: &TenantContext, record: &EmbeddingUsageRecord) {
        self.record_embedding_for(tenant, record).await;
    }
}

/// Worker-facing observer that attributes usage to the **current task tenant**,
/// ignoring the tenant baked into long-lived `LlmClient`/`EmbeddingClient`s.
#[derive(Clone)]
pub struct TaskTenantUsageObserver {
    inner: PgUsageObserver,
    tenant: Arc<RwLock<TenantContext>>,
}

impl std::fmt::Debug for TaskTenantUsageObserver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskTenantUsageObserver")
            .finish_non_exhaustive()
    }
}

impl TaskTenantUsageObserver {
    /// Worker metering: rebinds task tenant; rows are **non-billable** (ADR 0006 §7).
    /// Non-billable implies no wallet debit even if a wallet is later attached.
    pub fn new(store: Arc<dyn UsageLimitStorePort>, initial: TenantContext) -> Self {
        Self {
            inner: PgUsageObserver::new(store).with_billable(false),
            tenant: Arc::new(RwLock::new(initial)),
        }
    }

    pub async fn rebind(&self, tenant: TenantContext) {
        *self.tenant.write().await = tenant;
    }

    pub fn tenant_handle(&self) -> Arc<RwLock<TenantContext>> {
        self.tenant.clone()
    }

    #[cfg(test)]
    pub(crate) fn records_billable(&self) -> bool {
        self.inner.is_billable()
    }
}

#[async_trait]
impl UsageObserver for TaskTenantUsageObserver {
    async fn record_chat(&self, _tenant: &TenantContext, record: &ChatUsageRecord) {
        let tenant = self.tenant.read().await.clone();
        self.inner.record_chat_for(&tenant, record).await;
    }

    async fn record_embedding(&self, _tenant: &TenantContext, record: &EmbeddingUsageRecord) {
        let tenant = self.tenant.read().await.clone();
        self.inner.record_embedding_for(&tenant, record).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_core::{
        ApplyLedgerInput, ApplyLedgerResult, MeteringContext, UsageLimitOverrideRow,
        UsageLimitPlanPolicyRow, UsageLimitStorePort, UsageLimitUsageRecord, Wallet,
        WalletLedgerEntry, WalletStorePort,
    };
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use common::AppError;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    #[test]
    fn map_feature_is_deterministic_for_known_tags() {
        assert_eq!(
            PgUsageObserver::map_feature("summary"),
            BillableFeature::Summary
        );
        assert_eq!(
            PgUsageObserver::map_feature("planner"),
            BillableFeature::Planner
        );
        // "plan" as substring of "airplane" must NOT map to planner.
        assert_eq!(
            PgUsageObserver::map_feature("airplane_agent"),
            BillableFeature::Chat
        );
        assert_eq!(
            PgUsageObserver::map_feature("write:refine"),
            BillableFeature::Chat
        );
        assert_eq!(
            PgUsageObserver::map_feature("triplet"),
            BillableFeature::GraphExtraction
        );
        assert_eq!(
            PgUsageObserver::map_feature("document_embedding"),
            BillableFeature::Answer
        );
        assert_eq!(
            PgUsageObserver::map_feature("agent_loop"),
            BillableFeature::Chat
        );
    }

    struct StubUsageLimitStore;

    #[async_trait]
    impl UsageLimitStorePort for StubUsageLimitStore {
        async fn insert_llm_usage_event(
            &self,
            _ctx: &MeteringContext,
            _record: UsageLimitUsageRecord<'_>,
        ) -> Result<i64, AppError> {
            Ok(0)
        }

        async fn load_user_override(
            &self,
            _user_id: Uuid,
        ) -> Result<Option<UsageLimitOverrideRow>, AppError> {
            Ok(None)
        }

        async fn get_user_plan(&self, _user_id: Uuid) -> Result<String, AppError> {
            Ok("free".into())
        }

        async fn load_plan_policy(
            &self,
            _plan_id: &str,
        ) -> Result<Option<UsageLimitPlanPolicyRow>, AppError> {
            Ok(None)
        }

        async fn sum_usage_units_since(
            &self,
            _user_id: Uuid,
            _since: DateTime<Utc>,
        ) -> Result<i64, AppError> {
            Ok(0)
        }

        async fn oldest_usage_event_since(
            &self,
            _user_id: Uuid,
            _since: DateTime<Utc>,
        ) -> Result<Option<DateTime<Utc>>, AppError> {
            Ok(None)
        }

        async fn load_usage_breakdown(
            &self,
            _user_id: Uuid,
            _since: DateTime<Utc>,
        ) -> Result<HashMap<String, i64>, AppError> {
            Ok(HashMap::new())
        }

        async fn load_model_rates(
            &self,
            _provider: &str,
            _model: &str,
        ) -> Result<(f64, f64, f64), AppError> {
            Ok((1.0, 0.02, 2.0))
        }

        async fn has_user_override(&self, _user_id: Uuid) -> Result<bool, AppError> {
            Ok(false)
        }

        async fn has_estimated_usage(&self, _user_id: Uuid) -> Result<bool, AppError> {
            Ok(false)
        }
    }

    /// In-memory wallet for observer integration tests.
    struct MemoryWalletStore {
        wallets: Mutex<HashMap<Uuid, Wallet>>,
        ledger: Mutex<Vec<WalletLedgerEntry>>,
        by_key: Mutex<HashMap<String, Uuid>>,
    }

    impl MemoryWalletStore {
        fn new() -> Self {
            Self {
                wallets: Mutex::new(HashMap::new()),
                ledger: Mutex::new(Vec::new()),
                by_key: Mutex::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl WalletStorePort for MemoryWalletStore {
        async fn get_wallet(&self, user_id: Uuid) -> Result<Option<Wallet>, AppError> {
            Ok(self.wallets.lock().unwrap().get(&user_id).cloned())
        }

        async fn ensure_wallet(&self, user_id: Uuid) -> Result<Wallet, AppError> {
            let mut wallets = self.wallets.lock().unwrap();
            if let Some(w) = wallets.get(&user_id) {
                return Ok(w.clone());
            }
            let now = Utc::now();
            let w = Wallet {
                user_id,
                balance_fen: 0,
                lifetime_paid_topup_fen: 0,
                created_at: now,
                updated_at: now,
            };
            wallets.insert(user_id, w.clone());
            Ok(w)
        }

        async fn apply_ledger_entry(
            &self,
            input: &ApplyLedgerInput,
        ) -> Result<ApplyLedgerResult, AppError> {
            if input.amount_fen == 0 {
                return Err(AppError::validation(
                    "wallet_amount_zero",
                    "ledger amount_fen must be non-zero",
                ));
            }
            if input.idempotency_key.trim().is_empty() {
                return Err(AppError::validation(
                    "wallet_idempotency_required",
                    "idempotency_key is required",
                ));
            }
            {
                let by_key = self.by_key.lock().unwrap();
                if let Some(ledger_id) = by_key.get(&input.idempotency_key).copied() {
                    let ledger = self.ledger.lock().unwrap();
                    let entry = ledger
                        .iter()
                        .find(|e| e.id == ledger_id)
                        .cloned()
                        .ok_or_else(|| AppError::internal("idempotent ledger row missing"))?;
                    let wallet = self
                        .wallets
                        .lock()
                        .unwrap()
                        .get(&input.user_id)
                        .cloned()
                        .ok_or_else(|| AppError::internal("wallet missing after ledger"))?;
                    return Ok(ApplyLedgerResult {
                        wallet,
                        applied: false,
                        ledger_id: entry.id,
                    });
                }
            }

            let mut wallets = self.wallets.lock().unwrap();
            let now = Utc::now();
            let wallet = wallets.entry(input.user_id).or_insert_with(|| Wallet {
                user_id: input.user_id,
                balance_fen: 0,
                lifetime_paid_topup_fen: 0,
                created_at: now,
                updated_at: now,
            });
            let new_balance = wallet.balance_fen + input.amount_fen;
            if new_balance < 0 {
                return Err(AppError::validation(
                    "wallet_insufficient_balance",
                    "insufficient wallet balance",
                ));
            }
            wallet.balance_fen = new_balance;
            if input.counts_as_paid_topup && input.amount_fen > 0 {
                wallet.lifetime_paid_topup_fen += input.amount_fen;
            }
            wallet.updated_at = now;
            let wallet_snapshot = wallet.clone();
            drop(wallets);

            let entry = WalletLedgerEntry {
                id: Uuid::new_v4(),
                user_id: input.user_id,
                kind: input.kind.clone(),
                amount_fen: input.amount_fen,
                balance_after_fen: wallet_snapshot.balance_fen,
                idempotency_key: input.idempotency_key.clone(),
                metadata: input.metadata.clone(),
                created_at: now,
            };
            self.by_key
                .lock()
                .unwrap()
                .insert(input.idempotency_key.clone(), entry.id);
            self.ledger.lock().unwrap().push(entry.clone());
            Ok(ApplyLedgerResult {
                wallet: wallet_snapshot,
                applied: true,
                ledger_id: entry.id,
            })
        }

        async fn list_ledger(
            &self,
            user_id: Uuid,
            limit: i64,
        ) -> Result<Vec<WalletLedgerEntry>, AppError> {
            let mut rows: Vec<_> = self
                .ledger
                .lock()
                .unwrap()
                .iter()
                .filter(|e| e.user_id == user_id)
                .cloned()
                .collect();
            rows.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            rows.truncate(limit.max(0) as usize);
            Ok(rows)
        }
    }

    #[test]
    fn default_observer_is_billable() {
        let observer = PgUsageObserver::new(Arc::new(StubUsageLimitStore));
        assert!(observer.is_billable());
        assert!(!observer.skips_wallet_debit());
        assert!(!observer.has_wallet());
    }

    #[test]
    fn with_billable_false_marks_non_customer_rows() {
        let observer = PgUsageObserver::new(Arc::new(StubUsageLimitStore)).with_billable(false);
        assert!(!observer.is_billable());
    }

    #[test]
    fn byok_flag_skips_wallet_debit() {
        let observer = PgUsageObserver::new(Arc::new(StubUsageLimitStore))
            .with_wallet(Arc::new(MemoryWalletStore::new()))
            .with_skip_wallet_debit(true);
        assert!(observer.has_wallet());
        assert!(observer.skips_wallet_debit());
    }

    #[test]
    fn task_tenant_observer_is_non_billable_for_worker_path() {
        let tenant = TenantContext {
            owner_user_id: Uuid::nil(),
            user_id: Uuid::nil(),
        };
        let observer = TaskTenantUsageObserver::new(Arc::new(StubUsageLimitStore), tenant);
        assert!(
            !observer.records_billable(),
            "ADR 0006 §7: worker metering must not count toward user rolling quotas"
        );
    }

    #[tokio::test]
    async fn recorded_chat_usage_debits_wallet_at_list_price() {
        let wallet = Arc::new(MemoryWalletStore::new());
        let user_id = Uuid::new_v4();
        // Seed ¥20 grant.
        wallet
            .apply_ledger_entry(&ApplyLedgerInput {
                user_id,
                kind: app_core::WALLET_KIND_SIGNUP_GRANT.to_string(),
                amount_fen: app_core::SIGNUP_GRANT_FEN,
                idempotency_key: format!("signup_grant:{user_id}"),
                metadata: serde_json::json!({}),
                counts_as_paid_topup: false,
            })
            .await
            .unwrap();

        let observer = PgUsageObserver::new(Arc::new(StubUsageLimitStore))
            .with_wallet(wallet.clone() as Arc<dyn WalletStorePort>);

        let tenant = TenantContext {
            owner_user_id: user_id,
            user_id,
        };
        let record = ChatUsageRecord {
            prompt_tokens: 1_000_000,
            completion_tokens: 0,
            total_tokens: 1_000_000,
            cached_tokens: 0,
            reasoning_tokens: 0,
            provider: "deepseek".into(),
            model: "deepseek-v4-flash".into(),
            feature: "agent_loop".into(),
            stage: "chat".into(),
            session_id: None,
            document_id: None,
            request_id: Some("req-1".into()),
            trace_id: None,
        };
        observer.record_chat_for(&tenant, &record).await;

        let w = wallet.get_wallet(user_id).await.unwrap().unwrap();
        // 1M input flash → list 150 fen
        assert_eq!(w.balance_fen, app_core::SIGNUP_GRANT_FEN - 150);

        let ledger = wallet.list_ledger(user_id, 10).await.unwrap();
        let debits: Vec<_> = ledger
            .iter()
            .filter(|e| e.kind == app_core::WALLET_KIND_USAGE_DEBIT)
            .collect();
        assert_eq!(debits.len(), 1);
        assert_eq!(debits[0].amount_fen, -150);
    }

    #[tokio::test]
    async fn skip_wallet_debit_does_not_charge() {
        let wallet = Arc::new(MemoryWalletStore::new());
        let user_id = Uuid::new_v4();
        wallet
            .apply_ledger_entry(&ApplyLedgerInput {
                user_id,
                kind: app_core::WALLET_KIND_SIGNUP_GRANT.to_string(),
                amount_fen: app_core::SIGNUP_GRANT_FEN,
                idempotency_key: format!("signup_grant:{user_id}"),
                metadata: serde_json::json!({}),
                counts_as_paid_topup: false,
            })
            .await
            .unwrap();

        let observer = PgUsageObserver::new(Arc::new(StubUsageLimitStore))
            .with_wallet(wallet.clone() as Arc<dyn WalletStorePort>)
            .with_skip_wallet_debit(true);

        let tenant = TenantContext {
            owner_user_id: user_id,
            user_id,
        };
        observer
            .record_chat_for(
                &tenant,
                &ChatUsageRecord {
                    prompt_tokens: 1_000_000,
                    completion_tokens: 0,
                    total_tokens: 1_000_000,
                    cached_tokens: 0,
                    reasoning_tokens: 0,
                    provider: "deepseek".into(),
                    model: "deepseek-v4-flash".into(),
                    feature: "chat".into(),
                    stage: "chat".into(),
                    session_id: None,
                    document_id: None,
                    request_id: None,
                    trace_id: None,
                },
            )
            .await;

        assert_eq!(
            wallet.get_wallet(user_id).await.unwrap().unwrap().balance_fen,
            app_core::SIGNUP_GRANT_FEN
        );
        assert!(
            wallet
                .list_ledger(user_id, 10)
                .await
                .unwrap()
                .iter()
                .all(|e| e.kind != app_core::WALLET_KIND_USAGE_DEBIT)
        );
    }

    #[tokio::test]
    async fn non_billable_observer_does_not_debit() {
        let wallet = Arc::new(MemoryWalletStore::new());
        let user_id = Uuid::new_v4();
        wallet
            .apply_ledger_entry(&ApplyLedgerInput {
                user_id,
                kind: app_core::WALLET_KIND_SIGNUP_GRANT.to_string(),
                amount_fen: app_core::SIGNUP_GRANT_FEN,
                idempotency_key: format!("signup_grant:{user_id}"),
                metadata: serde_json::json!({}),
                counts_as_paid_topup: false,
            })
            .await
            .unwrap();

        let observer = PgUsageObserver::new(Arc::new(StubUsageLimitStore))
            .with_billable(false)
            .with_wallet(wallet.clone() as Arc<dyn WalletStorePort>);

        let tenant = TenantContext {
            owner_user_id: user_id,
            user_id,
        };
        observer
            .record_chat_for(
                &tenant,
                &ChatUsageRecord {
                    prompt_tokens: 1_000_000,
                    completion_tokens: 0,
                    total_tokens: 1_000_000,
                    cached_tokens: 0,
                    reasoning_tokens: 0,
                    provider: "deepseek".into(),
                    model: "deepseek-v4-flash".into(),
                    feature: "chat".into(),
                    stage: "chat".into(),
                    session_id: None,
                    document_id: None,
                    request_id: None,
                    trace_id: None,
                },
            )
            .await;

        assert_eq!(
            wallet.get_wallet(user_id).await.unwrap().unwrap().balance_fen,
            app_core::SIGNUP_GRANT_FEN
        );
    }
}
