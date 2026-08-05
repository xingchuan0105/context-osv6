//! User wallet service — balance query, signup credit, paid top-up, usage debit
//! (ADR-0010 PR3/PR5/PR6).
//!
//! **Amount unit: integer fen (分).** 100 fen = ¥1; signup grant = [`SIGNUP_GRANT_FEN`] = 2000 fen = ¥20.
//! Referral bilateral grant lives in [`crate::referral`].
//!
//! # PR5 paid top-up
//!
//! After Creem/Alipay webhook confirms payment, call [`credit_paid_topup`] with
//! `counts_as_paid_topup: true` so `lifetime_paid_topup_fen` rises (referral quota steps).
//! Idempotency: `topup:{provider}:{order_or_event_id}`.
//!
//! # PR6 usage debit behavior
//!
//! - **Platform proxy (default):** after billable LLM/embedding metering, debit
//!   `usage_debit` at `list_fen = ceil(official * 1.5)` (see [`crate::wallet_pricing`]).
//! - **BYOK / `skip_wallet_debit`:** observer skips the ledger write.
//! - **Non-billable rows** (worker path): no debit.
//! - **Insufficient balance:** [`WalletStorePort::apply_ledger_entry`] rejects with
//!   `wallet_insufficient_balance` (balance never goes negative). The usage
//!   observer logs this loudly and **does not** fail the LLM path (fail-open
//!   metering). A hard pre-flight stop is not wired in PR6; rolling token walls
//!   remain interim protection until pre-check lands.
//! - **Idempotency:** debits use `usage_debit:{event_id}` (or request-scoped keys);
//!   replaying the same key does not double-charge.

use std::sync::Arc;

use app_core::{
    ApplyLedgerInput, ApplyLedgerResult, DEFAULT_TOPUP_PACKS, SIGNUP_GRANT_FEN, TopupPack,
    WALLET_KIND_SIGNUP_GRANT, WALLET_KIND_TOPUP, WALLET_KIND_USAGE_DEBIT, Wallet, WalletStorePort,
    signup_grant_idempotency_key, topup_idempotency_key, topup_pack_by_id,
};
use common::{ApiResponse, AppError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::wallet_pricing::{list_price_fen, usage_debit_idempotency_key};

/// HTTP/API view of a user's wallet balance (fen).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalletBalanceResponse {
    pub user_id: Uuid,
    /// Spendable balance in fen (分). 2000 = ¥20.
    pub balance_fen: i64,
    /// Lifetime paid top-ups in fen (excludes gifts / referral).
    pub lifetime_paid_topup_fen: i64,
}

impl From<Wallet> for WalletBalanceResponse {
    fn from(w: Wallet) -> Self {
        Self {
            user_id: w.user_id,
            balance_fen: w.balance_fen,
            lifetime_paid_topup_fen: w.lifetime_paid_topup_fen,
        }
    }
}

/// Grant the one-time ¥20 (2000 fen) signup gift. Idempotent per user.
///
/// Second call with the same user returns `applied = false` and does not double-credit.
pub async fn grant_signup_bonus(
    store: Arc<dyn WalletStorePort>,
    user_id: Uuid,
) -> Result<ApplyLedgerResult, AppError> {
    let input = ApplyLedgerInput {
        user_id,
        kind: WALLET_KIND_SIGNUP_GRANT.to_string(),
        amount_fen: SIGNUP_GRANT_FEN,
        idempotency_key: signup_grant_idempotency_key(user_id),
        metadata: serde_json::json!({ "source": "register", "grant_cny": 20 }),
        counts_as_paid_topup: false,
    };
    store.apply_ledger_entry(&input).await
}

/// Ensure wallet row exists and return balance (creates zero-balance wallet if needed).
pub async fn get_wallet_balance(
    store: Arc<dyn WalletStorePort>,
    user_id: Uuid,
) -> Result<WalletBalanceResponse, AppError> {
    let wallet = store.ensure_wallet(user_id).await?;
    Ok(WalletBalanceResponse::from(wallet))
}

pub async fn handle_get_wallet(
    store: Arc<dyn WalletStorePort>,
    user_id: Uuid,
) -> ApiResponse<WalletBalanceResponse> {
    match get_wallet_balance(store, user_id).await {
        Ok(balance) => ApiResponse::ok(balance),
        Err(error) => ApiResponse::err("billing_wallet_failed", &error.to_string()),
    }
}

/// API view of a fixed top-up pack.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TopupPackResponse {
    pub pack_id: String,
    pub amount_fen: i64,
    pub amount_yuan: i64,
    pub label_cny: String,
}

impl From<&TopupPack> for TopupPackResponse {
    fn from(p: &TopupPack) -> Self {
        Self {
            pack_id: p.id.to_string(),
            amount_fen: p.amount_fen,
            amount_yuan: p.amount_yuan,
            label_cny: format!("¥{}", p.amount_yuan),
        }
    }
}

/// List fixed wallet top-up packs (code defaults; env product ids optional on checkout).
pub fn list_topup_packs() -> Vec<TopupPackResponse> {
    DEFAULT_TOPUP_PACKS.iter().map(TopupPackResponse::from).collect()
}

pub fn handle_list_topup_packs() -> ApiResponse<Vec<TopupPackResponse>> {
    ApiResponse::ok(list_topup_packs())
}

/// Inputs for a paid top-up credit after provider webhook (PR5).
#[derive(Debug, Clone)]
pub struct PaidTopupInput {
    pub user_id: Uuid,
    pub pack_id: String,
    pub amount_fen: i64,
    /// `creem` / `alipay` (used in ledger idempotency key + metadata).
    pub provider: String,
    /// Provider order id or webhook event id.
    pub order_or_event_id: String,
}

/// Credit wallet for a confirmed paid top-up. Bumps `lifetime_paid_topup_fen`.
///
/// Same `provider` + `order_or_event_id` is always idempotent (`applied = false` on replay).
pub async fn credit_paid_topup(
    store: Arc<dyn WalletStorePort>,
    input: &PaidTopupInput,
) -> Result<ApplyLedgerResult, AppError> {
    if input.amount_fen <= 0 {
        return Err(AppError::validation(
            "wallet_topup_amount_invalid",
            "top-up amount_fen must be positive",
        ));
    }
    if let Some(pack) = topup_pack_by_id(&input.pack_id) {
        if pack.amount_fen != input.amount_fen {
            return Err(AppError::validation(
                "wallet_topup_pack_amount_mismatch",
                "top-up amount does not match pack catalog",
            ));
        }
    }
    let order_key = input.order_or_event_id.trim();
    if order_key.is_empty() {
        return Err(AppError::validation(
            "wallet_topup_order_required",
            "order_or_event_id is required for top-up idempotency",
        ));
    }

    let ledger = ApplyLedgerInput {
        user_id: input.user_id,
        kind: WALLET_KIND_TOPUP.to_string(),
        amount_fen: input.amount_fen,
        idempotency_key: topup_idempotency_key(&input.provider, order_key),
        metadata: serde_json::json!({
            "source": "paid_topup",
            "provider": input.provider,
            "pack_id": input.pack_id,
            "amount_fen": input.amount_fen,
            "order_or_event_id": order_key,
            "purpose": "wallet_topup",
        }),
        counts_as_paid_topup: true,
    };
    store.apply_ledger_entry(&ledger).await
}

/// Inputs for a platform-proxy usage debit (PR6).
#[derive(Debug, Clone)]
pub struct UsageDebitInput {
    /// Payer wallet (B2C user; share Owner-pays is PR8).
    pub user_id: Uuid,
    pub provider: String,
    pub model: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub cached_tokens: u32,
    pub usage_kind: String,
    /// Stable event identity for the ledger idempotency key.
    pub event_id: Uuid,
    /// Optional request correlation for metadata only.
    pub request_id: Option<String>,
}

/// Debit the wallet for one platform-proxy usage event.
///
/// - Computes `list_fen = ceil(official * 1.5)`; returns `Ok(None)` when fen is 0.
/// - Unknown (non-whitelist) models → validation error (no silent free ride).
/// - Writes a negative `usage_debit` ledger row via [`WalletStorePort::apply_ledger_entry`].
/// - Same `event_id` is always idempotent (`applied = false` on replay).
/// - Insufficient balance → `wallet_insufficient_balance` validation error; balance unchanged.
pub async fn debit_platform_usage(
    store: Arc<dyn WalletStorePort>,
    input: &UsageDebitInput,
) -> Result<Option<ApplyLedgerResult>, AppError> {
    let list_fen = list_price_fen(
        &input.provider,
        &input.model,
        input.prompt_tokens,
        input.completion_tokens,
        input.cached_tokens,
    )
    .ok_or_else(|| {
        AppError::validation(
            "wallet_model_not_whitelisted",
            format!(
                "model not on platform-proxy price whitelist: {}/{}",
                input.provider, input.model
            ),
        )
    })?;
    if list_fen <= 0 {
        return Ok(None);
    }

    let ledger = ApplyLedgerInput {
        user_id: input.user_id,
        kind: WALLET_KIND_USAGE_DEBIT.to_string(),
        amount_fen: -list_fen,
        idempotency_key: usage_debit_idempotency_key(input.event_id),
        metadata: serde_json::json!({
            "provider": input.provider,
            "model": input.model,
            "prompt_tokens": input.prompt_tokens,
            "completion_tokens": input.completion_tokens,
            "cached_tokens": input.cached_tokens,
            "usage_kind": input.usage_kind,
            "list_fen": list_fen,
            "list_price_multiplier": crate::wallet_pricing::LIST_PRICE_MULTIPLIER,
            "request_id": input.request_id,
            "event_id": input.event_id,
        }),
        counts_as_paid_topup: false,
    };
    store.apply_ledger_entry(&ledger).await.map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_core::{WalletLedgerEntry, WalletStorePort};
    use async_trait::async_trait;
    use chrono::Utc;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// In-memory wallet store for unit tests (no Postgres).
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

    #[tokio::test]
    async fn signup_grant_credits_2000_fen_once() {
        let store: Arc<dyn WalletStorePort> = Arc::new(MemoryWalletStore::new());
        let user_id = Uuid::new_v4();

        let first = grant_signup_bonus(store.clone(), user_id).await.unwrap();
        assert!(first.applied);
        assert_eq!(first.wallet.balance_fen, SIGNUP_GRANT_FEN);
        assert_eq!(first.wallet.lifetime_paid_topup_fen, 0);

        let second = grant_signup_bonus(store.clone(), user_id).await.unwrap();
        assert!(!second.applied);
        assert_eq!(second.wallet.balance_fen, SIGNUP_GRANT_FEN);
        assert_eq!(second.ledger_id, first.ledger_id);

        let ledger = store.list_ledger(user_id, 10).await.unwrap();
        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger[0].kind, WALLET_KIND_SIGNUP_GRANT);
        assert_eq!(ledger[0].amount_fen, SIGNUP_GRANT_FEN);
        assert_eq!(ledger[0].balance_after_fen, SIGNUP_GRANT_FEN);
        assert_eq!(
            ledger[0].idempotency_key,
            signup_grant_idempotency_key(user_id)
        );

        let balance = get_wallet_balance(store, user_id).await.unwrap();
        assert_eq!(balance.balance_fen, 2000);
    }

    #[tokio::test]
    async fn ledger_is_auditable_for_multiple_kinds() {
        let store: Arc<dyn WalletStorePort> = Arc::new(MemoryWalletStore::new());
        let user_id = Uuid::new_v4();

        grant_signup_bonus(store.clone(), user_id).await.unwrap();
        store
            .apply_ledger_entry(&ApplyLedgerInput {
                user_id,
                kind: app_core::WALLET_KIND_TOPUP.to_string(),
                amount_fen: 5000,
                idempotency_key: format!("topup:test:{user_id}"),
                metadata: serde_json::json!({ "provider": "test" }),
                counts_as_paid_topup: true,
            })
            .await
            .unwrap();

        let wallet = store.get_wallet(user_id).await.unwrap().unwrap();
        assert_eq!(wallet.balance_fen, 2000 + 5000);
        assert_eq!(wallet.lifetime_paid_topup_fen, 5000);

        let ledger = store.list_ledger(user_id, 10).await.unwrap();
        assert_eq!(ledger.len(), 2);
        // Newest first
        assert_eq!(ledger[0].kind, app_core::WALLET_KIND_TOPUP);
        assert_eq!(ledger[1].kind, WALLET_KIND_SIGNUP_GRANT);
    }

    #[tokio::test]
    async fn paid_topup_increases_balance_and_lifetime_paid() {
        let store: Arc<dyn WalletStorePort> = Arc::new(MemoryWalletStore::new());
        let user_id = Uuid::new_v4();
        grant_signup_bonus(store.clone(), user_id).await.unwrap();

        let first = credit_paid_topup(
            store.clone(),
            &PaidTopupInput {
                user_id,
                pack_id: app_core::TOPUP_PACK_50.to_string(),
                amount_fen: 5000,
                provider: "alipay".into(),
                order_or_event_id: "order-abc".into(),
            },
        )
        .await
        .unwrap();
        assert!(first.applied);
        assert_eq!(first.wallet.balance_fen, SIGNUP_GRANT_FEN + 5000);
        assert_eq!(first.wallet.lifetime_paid_topup_fen, 5000);

        let second = credit_paid_topup(
            store.clone(),
            &PaidTopupInput {
                user_id,
                pack_id: app_core::TOPUP_PACK_50.to_string(),
                amount_fen: 5000,
                provider: "alipay".into(),
                order_or_event_id: "order-abc".into(),
            },
        )
        .await
        .unwrap();
        assert!(!second.applied);
        assert_eq!(second.wallet.balance_fen, SIGNUP_GRANT_FEN + 5000);
        assert_eq!(second.wallet.lifetime_paid_topup_fen, 5000);
        assert_eq!(second.ledger_id, first.ledger_id);

        let ledger = store.list_ledger(user_id, 10).await.unwrap();
        assert_eq!(
            ledger
                .iter()
                .filter(|e| e.kind == WALLET_KIND_TOPUP)
                .count(),
            1
        );
        assert_eq!(
            ledger[0].idempotency_key,
            topup_idempotency_key("alipay", "order-abc")
        );
    }

    #[tokio::test]
    async fn list_topup_packs_matches_catalog() {
        let packs = list_topup_packs();
        assert_eq!(packs.len(), 3);
        assert_eq!(packs[0].pack_id, app_core::TOPUP_PACK_50);
        assert_eq!(packs[0].amount_fen, 5000);
        assert_eq!(packs[1].amount_fen, 10_000);
        assert_eq!(packs[2].amount_fen, 20_000);
    }

    #[tokio::test]
    async fn usage_debit_reduces_balance_by_list_fen() {
        let store: Arc<dyn WalletStorePort> = Arc::new(MemoryWalletStore::new());
        let user_id = Uuid::new_v4();
        grant_signup_bonus(store.clone(), user_id).await.unwrap();

        // 1M input deepseek-flash → list 150 fen
        let event_id = Uuid::new_v4();
        let result = debit_platform_usage(
            store.clone(),
            &UsageDebitInput {
                user_id,
                provider: "deepseek".into(),
                model: "deepseek-v4-flash".into(),
                prompt_tokens: 1_000_000,
                completion_tokens: 0,
                cached_tokens: 0,
                usage_kind: "chat".into(),
                event_id,
                request_id: Some("req-a".into()),
            },
        )
        .await
        .unwrap()
        .expect("should debit");

        assert!(result.applied);
        assert_eq!(result.wallet.balance_fen, SIGNUP_GRANT_FEN - 150);

        let ledger = store.list_ledger(user_id, 10).await.unwrap();
        assert_eq!(ledger[0].kind, WALLET_KIND_USAGE_DEBIT);
        assert_eq!(ledger[0].amount_fen, -150);
        assert_eq!(
            ledger[0].idempotency_key,
            usage_debit_idempotency_key(event_id)
        );
    }

    #[tokio::test]
    async fn usage_debit_is_idempotent_for_same_event_id() {
        let store: Arc<dyn WalletStorePort> = Arc::new(MemoryWalletStore::new());
        let user_id = Uuid::new_v4();
        grant_signup_bonus(store.clone(), user_id).await.unwrap();
        let event_id = Uuid::new_v4();
        let input = UsageDebitInput {
            user_id,
            provider: "deepseek".into(),
            model: "deepseek-v4-flash".into(),
            prompt_tokens: 1_000_000,
            completion_tokens: 0,
            cached_tokens: 0,
            usage_kind: "chat".into(),
            event_id,
            request_id: None,
        };

        let first = debit_platform_usage(store.clone(), &input)
            .await
            .unwrap()
            .unwrap();
        assert!(first.applied);
        let second = debit_platform_usage(store.clone(), &input)
            .await
            .unwrap()
            .unwrap();
        assert!(!second.applied);
        assert_eq!(second.wallet.balance_fen, first.wallet.balance_fen);
        assert_eq!(second.ledger_id, first.ledger_id);

        let ledger = store.list_ledger(user_id, 10).await.unwrap();
        assert_eq!(
            ledger
                .iter()
                .filter(|e| e.kind == WALLET_KIND_USAGE_DEBIT)
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn zero_balance_usage_debit_fails_without_changing_balance() {
        let store: Arc<dyn WalletStorePort> = Arc::new(MemoryWalletStore::new());
        let user_id = Uuid::new_v4();
        // Ensure wallet exists at 0 fen (no signup grant).
        store.ensure_wallet(user_id).await.unwrap();

        let err = debit_platform_usage(
            store.clone(),
            &UsageDebitInput {
                user_id,
                provider: "deepseek".into(),
                model: "deepseek-v4-flash".into(),
                prompt_tokens: 1_000_000,
                completion_tokens: 0,
                cached_tokens: 0,
                usage_kind: "chat".into(),
                event_id: Uuid::new_v4(),
                request_id: None,
            },
        )
        .await
        .expect_err("insufficient balance must error");

        assert_eq!(err.code(), "wallet_insufficient_balance");
        let wallet = store.get_wallet(user_id).await.unwrap().unwrap();
        assert_eq!(wallet.balance_fen, 0);
        assert!(store.list_ledger(user_id, 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn zero_token_usage_skips_ledger() {
        let store: Arc<dyn WalletStorePort> = Arc::new(MemoryWalletStore::new());
        let user_id = Uuid::new_v4();
        grant_signup_bonus(store.clone(), user_id).await.unwrap();

        let result = debit_platform_usage(
            store.clone(),
            &UsageDebitInput {
                user_id,
                provider: "deepseek".into(),
                model: "deepseek-v4-flash".into(),
                prompt_tokens: 0,
                completion_tokens: 0,
                cached_tokens: 0,
                usage_kind: "chat".into(),
                event_id: Uuid::new_v4(),
                request_id: None,
            },
        )
        .await
        .unwrap();
        assert!(result.is_none());
        assert_eq!(
            store.get_wallet(user_id).await.unwrap().unwrap().balance_fen,
            SIGNUP_GRANT_FEN
        );
    }
}
