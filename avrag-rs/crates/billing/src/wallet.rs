//! User wallet service — balance query + ledger credits (ADR-0010 PR3).
//!
//! **Amount unit: integer fen (分).** 100 fen = ¥1; signup grant = [`SIGNUP_GRANT_FEN`] = 2000 fen = ¥20.
//! Referral (PR4), top-up webhook (PR5), and usage debit wiring (PR6) are not implemented here;
//! their ledger kinds are reserved on the schema.

use std::sync::Arc;

use app_core::{
    ApplyLedgerInput, ApplyLedgerResult, SIGNUP_GRANT_FEN, WALLET_KIND_SIGNUP_GRANT, Wallet,
    WalletStorePort, signup_grant_idempotency_key,
};
use common::{ApiResponse, AppError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
}
