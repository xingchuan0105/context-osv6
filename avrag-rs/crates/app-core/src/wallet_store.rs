//! Wallet persistence boundary — SQL implementations live in bootstrap adapters.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use common::AppError;
use uuid::Uuid;

use crate::wallet_domain::{
    ApplyLedgerInput, ApplyLedgerResult, WALLET_KIND_USAGE_DEBIT, Wallet, WalletLedgerEntry,
};

#[async_trait]
pub trait WalletStorePort: Send + Sync {
    /// Load wallet row, or `None` when the user has never touched wallet state.
    async fn get_wallet(&self, user_id: Uuid) -> Result<Option<Wallet>, AppError>;

    /// Ensure a zero-balance wallet row exists and return it.
    async fn ensure_wallet(&self, user_id: Uuid) -> Result<Wallet, AppError>;

    /// Atomically apply a signed ledger entry with idempotency.
    ///
    /// - Same `idempotency_key` always returns the same ledger outcome (`applied = false` on replay).
    /// - Debits that would drive balance below zero fail with a domain validation error.
    async fn apply_ledger_entry(
        &self,
        input: &ApplyLedgerInput,
    ) -> Result<ApplyLedgerResult, AppError>;

    /// Recent ledger rows for audit (newest first).
    async fn list_ledger(
        &self,
        user_id: Uuid,
        limit: i64,
    ) -> Result<Vec<WalletLedgerEntry>, AppError>;

    /// Sum of absolute fen for `usage_debit` rows since `since` (owner daily budget).
    /// Default falls back to scanning recent ledger rows.
    async fn sum_usage_debit_fen_since(
        &self,
        user_id: Uuid,
        since: DateTime<Utc>,
    ) -> Result<i64, AppError> {
        let entries = self.list_ledger(user_id, 5_000).await?;
        Ok(entries
            .iter()
            .filter(|e| {
                e.created_at >= since && e.kind == WALLET_KIND_USAGE_DEBIT && e.amount_fen < 0
            })
            .map(|e| -e.amount_fen)
            .sum())
    }
}
