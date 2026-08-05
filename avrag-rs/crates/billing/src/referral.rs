//! Referral codes + bilateral ¥5 (500 fen) grant (ADR-0010 PR4 / §6–7).
//!
//! On register with a valid invite code (after signup grant): credit inviter and invitee
//! 500 fen each as `referral_bonus`. Quota: `5 + floor(lifetime_paid_topup_fen / 5000)`.
//! Only `rewarded` counts; over-quota rejects both sides without grant.
//! Does not increase share workspace quota.

use std::sync::Arc;

use app_core::{
    ApplyLedgerInput, REFERRAL_BONUS_FEN, REFERRAL_REJECT_CODE_INVALID, REFERRAL_REJECT_CODE_REVOKED,
    REFERRAL_REJECT_QUOTA_EXHAUSTED, REFERRAL_REJECT_SELF_INVITE, REFERRAL_STATUS_PENDING,
    REFERRAL_STATUS_REJECTED, REFERRAL_STATUS_REWARDED, Referral, ReferralStats, ReferralStorePort,
    WALLET_KIND_REFERRAL_BONUS, WalletStorePort, normalize_referral_code,
    referral_bonus_invitee_idempotency_key, referral_bonus_inviter_idempotency_key, referral_quota,
};
use common::{ApiResponse, AppError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Outcome of applying a referral at register time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyReferralOutcome {
    /// No code provided — nothing to do.
    SkippedEmpty,
    /// Code not found / revoked / self-invite without row — no grant.
    Rejected {
        reason: &'static str,
    },
    /// Over quota or other rejected status on the referrals row.
    RecordedRejected {
        referral: Referral,
    },
    /// Both sides credited (or already credited — idempotent replay).
    Rewarded {
        referral: Referral,
        inviter_applied: bool,
        invitee_applied: bool,
    },
    /// Invitee already bound to a prior outcome.
    AlreadyBound {
        referral: Referral,
    },
}

/// Apply referral after successful registration (verification = successful register).
///
/// Safe to call twice: invitee unique + ledger idempotency keys prevent double grants.
pub async fn apply_referral_on_register(
    wallet: Arc<dyn WalletStorePort>,
    referral_store: Arc<dyn ReferralStorePort>,
    invitee_id: Uuid,
    raw_code: Option<&str>,
) -> Result<ApplyReferralOutcome, AppError> {
    let Some(raw) = raw_code.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(ApplyReferralOutcome::SkippedEmpty);
    };
    let code = normalize_referral_code(raw);
    if code.is_empty() {
        return Ok(ApplyReferralOutcome::SkippedEmpty);
    }

    // Idempotent: invitee already has a binding.
    if let Some(existing) = referral_store.get_by_invitee(invitee_id).await? {
        return complete_or_return_existing(wallet, referral_store, existing).await;
    }

    let Some(code_row) = referral_store.find_code(&code).await? else {
        return Ok(ApplyReferralOutcome::Rejected {
            reason: REFERRAL_REJECT_CODE_INVALID,
        });
    };
    if code_row.revoked_at.is_some() {
        return Ok(ApplyReferralOutcome::Rejected {
            reason: REFERRAL_REJECT_CODE_REVOKED,
        });
    }

    let inviter_id = code_row.user_id;
    if inviter_id == invitee_id {
        return Ok(ApplyReferralOutcome::Rejected {
            reason: REFERRAL_REJECT_SELF_INVITE,
        });
    }

    let insert = referral_store
        .insert_pending(inviter_id, invitee_id, &code_row.code)
        .await?;
    if !insert.inserted {
        return complete_or_return_existing(wallet, referral_store, insert.referral).await;
    }

    finish_pending(wallet, referral_store, insert.referral).await
}

async fn complete_or_return_existing(
    wallet: Arc<dyn WalletStorePort>,
    referral_store: Arc<dyn ReferralStorePort>,
    existing: Referral,
) -> Result<ApplyReferralOutcome, AppError> {
    match existing.status.as_str() {
        s if s == REFERRAL_STATUS_REWARDED || s == REFERRAL_STATUS_REJECTED => {
            Ok(ApplyReferralOutcome::AlreadyBound {
                referral: existing,
            })
        }
        s if s == REFERRAL_STATUS_PENDING => {
            finish_pending(wallet, referral_store, existing).await
        }
        _ => Ok(ApplyReferralOutcome::AlreadyBound {
            referral: existing,
        }),
    }
}

async fn finish_pending(
    wallet: Arc<dyn WalletStorePort>,
    referral_store: Arc<dyn ReferralStorePort>,
    pending: Referral,
) -> Result<ApplyReferralOutcome, AppError> {
    let inviter_wallet = wallet.ensure_wallet(pending.inviter_id).await?;
    let quota = referral_quota(inviter_wallet.lifetime_paid_topup_fen);
    let rewarded = referral_store
        .count_rewarded_by_inviter(pending.inviter_id)
        .await?;

    if rewarded >= quota {
        let rejected = referral_store
            .mark_rejected(pending.id, REFERRAL_REJECT_QUOTA_EXHAUSTED)
            .await?;
        return Ok(ApplyReferralOutcome::RecordedRejected {
            referral: rejected,
        });
    }

    let inviter_grant = wallet
        .apply_ledger_entry(&ApplyLedgerInput {
            user_id: pending.inviter_id,
            kind: WALLET_KIND_REFERRAL_BONUS.to_string(),
            amount_fen: REFERRAL_BONUS_FEN,
            idempotency_key: referral_bonus_inviter_idempotency_key(pending.invitee_id),
            metadata: serde_json::json!({
                "source": "referral",
                "side": "inviter",
                "invitee_id": pending.invitee_id,
                "code": pending.code,
                "grant_cny": 5,
            }),
            counts_as_paid_topup: false,
        })
        .await?;

    let invitee_grant = wallet
        .apply_ledger_entry(&ApplyLedgerInput {
            user_id: pending.invitee_id,
            kind: WALLET_KIND_REFERRAL_BONUS.to_string(),
            amount_fen: REFERRAL_BONUS_FEN,
            idempotency_key: referral_bonus_invitee_idempotency_key(pending.invitee_id),
            metadata: serde_json::json!({
                "source": "referral",
                "side": "invitee",
                "inviter_id": pending.inviter_id,
                "code": pending.code,
                "grant_cny": 5,
            }),
            counts_as_paid_topup: false,
        })
        .await?;

    let rewarded_row = referral_store.mark_rewarded(pending.id).await?;
    Ok(ApplyReferralOutcome::Rewarded {
        referral: rewarded_row,
        inviter_applied: inviter_grant.applied,
        invitee_applied: invitee_grant.applied,
    })
}

/// Ensure code exists and return inviter-facing stats.
pub async fn get_my_referral_stats(
    wallet: Arc<dyn WalletStorePort>,
    referral_store: Arc<dyn ReferralStorePort>,
    user_id: Uuid,
) -> Result<ReferralStats, AppError> {
    let code_row = referral_store.ensure_code(user_id).await?;
    let inviter_wallet = wallet.ensure_wallet(user_id).await?;
    let rewarded_count = referral_store.count_rewarded_by_inviter(user_id).await?;
    let quota = referral_quota(inviter_wallet.lifetime_paid_topup_fen);
    let remaining = (quota - rewarded_count).max(0);
    Ok(ReferralStats {
        code: code_row.code,
        rewarded_count,
        quota,
        remaining,
        lifetime_paid_topup_fen: inviter_wallet.lifetime_paid_topup_fen,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReferralStatsResponse {
    pub code: String,
    pub rewarded_count: i64,
    pub quota: i64,
    pub remaining: i64,
    pub lifetime_paid_topup_fen: i64,
}

impl From<ReferralStats> for ReferralStatsResponse {
    fn from(s: ReferralStats) -> Self {
        Self {
            code: s.code,
            rewarded_count: s.rewarded_count,
            quota: s.quota,
            remaining: s.remaining,
            lifetime_paid_topup_fen: s.lifetime_paid_topup_fen,
        }
    }
}

pub async fn handle_get_referral(
    wallet: Arc<dyn WalletStorePort>,
    referral_store: Arc<dyn ReferralStorePort>,
    user_id: Uuid,
) -> ApiResponse<ReferralStatsResponse> {
    match get_my_referral_stats(wallet, referral_store, user_id).await {
        Ok(stats) => ApiResponse::ok(ReferralStatsResponse::from(stats)),
        Err(error) => ApiResponse::err("billing_referral_failed", &error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_core::{
        ApplyLedgerResult, InsertPendingResult, REFERRAL_BASE_QUOTA, REFERRAL_TOPUP_STEP_FEN,
        ReferralCode, SIGNUP_GRANT_FEN, WALLET_KIND_SIGNUP_GRANT, WALLET_KIND_TOPUP, Wallet,
        WalletLedgerEntry, generate_referral_code,
    };
    use async_trait::async_trait;
    use chrono::Utc;
    use std::collections::HashMap;
    use std::sync::Mutex;

    use crate::wallet::grant_signup_bonus;

    /// Combined in-memory wallet + referral store for unit tests.
    struct MemoryBillingStore {
        wallets: Mutex<HashMap<Uuid, Wallet>>,
        ledger: Mutex<Vec<WalletLedgerEntry>>,
        by_key: Mutex<HashMap<String, Uuid>>,
        codes: Mutex<HashMap<Uuid, ReferralCode>>,
        codes_by_value: Mutex<HashMap<String, Uuid>>,
        referrals: Mutex<Vec<Referral>>,
    }

    impl MemoryBillingStore {
        fn new() -> Self {
            Self {
                wallets: Mutex::new(HashMap::new()),
                ledger: Mutex::new(Vec::new()),
                by_key: Mutex::new(HashMap::new()),
                codes: Mutex::new(HashMap::new()),
                codes_by_value: Mutex::new(HashMap::new()),
                referrals: Mutex::new(Vec::new()),
            }
        }

        fn arc(self) -> Arc<Self> {
            Arc::new(self)
        }
    }

    #[async_trait]
    impl WalletStorePort for MemoryBillingStore {
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

    #[async_trait]
    impl ReferralStorePort for MemoryBillingStore {
        async fn ensure_code(&self, user_id: Uuid) -> Result<ReferralCode, AppError> {
            {
                let codes = self.codes.lock().unwrap();
                if let Some(c) = codes.get(&user_id) {
                    return Ok(c.clone());
                }
            }
            let now = Utc::now();
            let mut code = generate_referral_code();
            // Rare collision: regenerate.
            for _ in 0..5 {
                let by_val = self.codes_by_value.lock().unwrap();
                if !by_val.contains_key(&code) {
                    break;
                }
                drop(by_val);
                code = generate_referral_code();
            }
            let row = ReferralCode {
                user_id,
                code: code.clone(),
                created_at: now,
                revoked_at: None,
            };
            self.codes.lock().unwrap().insert(user_id, row.clone());
            self.codes_by_value
                .lock()
                .unwrap()
                .insert(code, user_id);
            Ok(row)
        }

        async fn find_code(&self, code: &str) -> Result<Option<ReferralCode>, AppError> {
            let normalized = normalize_referral_code(code);
            let by_val = self.codes_by_value.lock().unwrap();
            let Some(user_id) = by_val.get(&normalized).copied() else {
                return Ok(None);
            };
            Ok(self.codes.lock().unwrap().get(&user_id).cloned())
        }

        async fn count_rewarded_by_inviter(&self, inviter_id: Uuid) -> Result<i64, AppError> {
            let n = self
                .referrals
                .lock()
                .unwrap()
                .iter()
                .filter(|r| r.inviter_id == inviter_id && r.status == REFERRAL_STATUS_REWARDED)
                .count();
            Ok(n as i64)
        }

        async fn get_by_invitee(&self, invitee_id: Uuid) -> Result<Option<Referral>, AppError> {
            Ok(self
                .referrals
                .lock()
                .unwrap()
                .iter()
                .find(|r| r.invitee_id == invitee_id)
                .cloned())
        }

        async fn insert_pending(
            &self,
            inviter_id: Uuid,
            invitee_id: Uuid,
            code: &str,
        ) -> Result<InsertPendingResult, AppError> {
            let mut rows = self.referrals.lock().unwrap();
            if let Some(existing) = rows.iter().find(|r| r.invitee_id == invitee_id) {
                return Ok(InsertPendingResult {
                    referral: existing.clone(),
                    inserted: false,
                });
            }
            let now = Utc::now();
            let referral = Referral {
                id: Uuid::new_v4(),
                inviter_id,
                invitee_id,
                code: code.to_string(),
                status: REFERRAL_STATUS_PENDING.to_string(),
                rewarded_at: None,
                reject_reason: None,
                created_at: now,
            };
            rows.push(referral.clone());
            Ok(InsertPendingResult {
                referral,
                inserted: true,
            })
        }

        async fn mark_rewarded(&self, referral_id: Uuid) -> Result<Referral, AppError> {
            let mut rows = self.referrals.lock().unwrap();
            let row = rows
                .iter_mut()
                .find(|r| r.id == referral_id)
                .ok_or_else(|| AppError::not_found("referral_not_found", "referral not found"))?;
            row.status = REFERRAL_STATUS_REWARDED.to_string();
            row.rewarded_at = Some(Utc::now());
            row.reject_reason = None;
            Ok(row.clone())
        }

        async fn mark_rejected(
            &self,
            referral_id: Uuid,
            reason: &str,
        ) -> Result<Referral, AppError> {
            let mut rows = self.referrals.lock().unwrap();
            let row = rows
                .iter_mut()
                .find(|r| r.id == referral_id)
                .ok_or_else(|| AppError::not_found("referral_not_found", "referral not found"))?;
            row.status = REFERRAL_STATUS_REJECTED.to_string();
            row.reject_reason = Some(reason.to_string());
            Ok(row.clone())
        }
    }

    async fn seed_inviter(store: Arc<MemoryBillingStore>) -> (Uuid, String) {
        let inviter = Uuid::new_v4();
        let code = store.ensure_code(inviter).await.unwrap().code;
        (inviter, code)
    }

    #[tokio::test]
    async fn valid_code_credits_both_sides_500_fen_rewarded() {
        let store = MemoryBillingStore::new().arc();
        let wallet: Arc<dyn WalletStorePort> = store.clone();
        let referral: Arc<dyn ReferralStorePort> = store.clone();

        let (inviter, code) = seed_inviter(store.clone()).await;
        let invitee = Uuid::new_v4();

        let outcome = apply_referral_on_register(
            wallet.clone(),
            referral.clone(),
            invitee,
            Some(&code),
        )
        .await
        .unwrap();

        match outcome {
            ApplyReferralOutcome::Rewarded {
                referral: row,
                inviter_applied,
                invitee_applied,
            } => {
                assert!(inviter_applied);
                assert!(invitee_applied);
                assert_eq!(row.status, REFERRAL_STATUS_REWARDED);
                assert_eq!(row.inviter_id, inviter);
                assert_eq!(row.invitee_id, invitee);
            }
            other => panic!("expected Rewarded, got {other:?}"),
        }

        let inviter_w = wallet.get_wallet(inviter).await.unwrap().unwrap();
        let invitee_w = wallet.get_wallet(invitee).await.unwrap().unwrap();
        assert_eq!(inviter_w.balance_fen, REFERRAL_BONUS_FEN);
        assert_eq!(invitee_w.balance_fen, REFERRAL_BONUS_FEN);
        assert_eq!(inviter_w.lifetime_paid_topup_fen, 0);
        assert_eq!(invitee_w.lifetime_paid_topup_fen, 0);
    }

    #[tokio::test]
    async fn sixth_invite_without_topup_rejected_no_grant() {
        let store = MemoryBillingStore::new().arc();
        let wallet: Arc<dyn WalletStorePort> = store.clone();
        let referral: Arc<dyn ReferralStorePort> = store.clone();

        let (inviter, code) = seed_inviter(store.clone()).await;

        for i in 0..REFERRAL_BASE_QUOTA {
            let invitee = Uuid::new_v4();
            let out = apply_referral_on_register(
                wallet.clone(),
                referral.clone(),
                invitee,
                Some(&code),
            )
            .await
            .unwrap();
            assert!(
                matches!(out, ApplyReferralOutcome::Rewarded { .. }),
                "invite {i} should succeed"
            );
        }

        let inviter_after_5 = wallet.get_wallet(inviter).await.unwrap().unwrap();
        assert_eq!(
            inviter_after_5.balance_fen,
            REFERRAL_BASE_QUOTA * REFERRAL_BONUS_FEN
        );

        let sixth = Uuid::new_v4();
        let out = apply_referral_on_register(wallet.clone(), referral.clone(), sixth, Some(&code))
            .await
            .unwrap();
        match out {
            ApplyReferralOutcome::RecordedRejected { referral: row } => {
                assert_eq!(row.status, REFERRAL_STATUS_REJECTED);
                assert_eq!(
                    row.reject_reason.as_deref(),
                    Some(REFERRAL_REJECT_QUOTA_EXHAUSTED)
                );
            }
            other => panic!("expected RecordedRejected, got {other:?}"),
        }

        // Sixth invitee got nothing; inviter balance unchanged.
        assert!(wallet.get_wallet(sixth).await.unwrap().is_none()
            || wallet.get_wallet(sixth).await.unwrap().unwrap().balance_fen == 0);
        let inviter_after_6 = wallet.get_wallet(inviter).await.unwrap().unwrap();
        assert_eq!(
            inviter_after_6.balance_fen,
            REFERRAL_BASE_QUOTA * REFERRAL_BONUS_FEN
        );
        assert_eq!(
            referral.count_rewarded_by_inviter(inviter).await.unwrap(),
            REFERRAL_BASE_QUOTA
        );
    }

    #[tokio::test]
    async fn after_topup_step_quota_allows_one_more() {
        let store = MemoryBillingStore::new().arc();
        let wallet: Arc<dyn WalletStorePort> = store.clone();
        let referral: Arc<dyn ReferralStorePort> = store.clone();

        let (inviter, code) = seed_inviter(store.clone()).await;

        // Exhaust base quota of 5.
        for _ in 0..REFERRAL_BASE_QUOTA {
            let invitee = Uuid::new_v4();
            apply_referral_on_register(wallet.clone(), referral.clone(), invitee, Some(&code))
                .await
                .unwrap();
        }

        // Still blocked.
        let blocked = Uuid::new_v4();
        let out =
            apply_referral_on_register(wallet.clone(), referral.clone(), blocked, Some(&code))
                .await
                .unwrap();
        assert!(matches!(out, ApplyReferralOutcome::RecordedRejected { .. }));

        // Simulate paid top-up of ¥50 = 5000 fen → quota becomes 6.
        wallet
            .apply_ledger_entry(&ApplyLedgerInput {
                user_id: inviter,
                kind: WALLET_KIND_TOPUP.to_string(),
                amount_fen: REFERRAL_TOPUP_STEP_FEN,
                idempotency_key: format!("topup:test:{inviter}"),
                metadata: serde_json::json!({ "provider": "test" }),
                counts_as_paid_topup: true,
            })
            .await
            .unwrap();

        let w = wallet.get_wallet(inviter).await.unwrap().unwrap();
        assert_eq!(w.lifetime_paid_topup_fen, REFERRAL_TOPUP_STEP_FEN);
        assert_eq!(referral_quota(w.lifetime_paid_topup_fen), 6);

        let seventh = Uuid::new_v4();
        let out =
            apply_referral_on_register(wallet.clone(), referral.clone(), seventh, Some(&code))
                .await
                .unwrap();
        assert!(
            matches!(out, ApplyReferralOutcome::Rewarded { .. }),
            "quota +1 after topup step"
        );
        assert_eq!(
            referral.count_rewarded_by_inviter(inviter).await.unwrap(),
            6
        );
    }

    #[tokio::test]
    async fn signup_grant_stacks_with_referral_invitee_total_2500() {
        let store = MemoryBillingStore::new().arc();
        let wallet: Arc<dyn WalletStorePort> = store.clone();
        let referral: Arc<dyn ReferralStorePort> = store.clone();

        let (_inviter, code) = seed_inviter(store.clone()).await;
        let invitee = Uuid::new_v4();

        // Register path order: signup grant then referral.
        let signup = grant_signup_bonus(wallet.clone(), invitee).await.unwrap();
        assert!(signup.applied);
        assert_eq!(signup.wallet.balance_fen, SIGNUP_GRANT_FEN);

        let outcome =
            apply_referral_on_register(wallet.clone(), referral.clone(), invitee, Some(&code))
                .await
                .unwrap();
        assert!(matches!(outcome, ApplyReferralOutcome::Rewarded { .. }));

        let invitee_w = wallet.get_wallet(invitee).await.unwrap().unwrap();
        assert_eq!(
            invitee_w.balance_fen,
            SIGNUP_GRANT_FEN + REFERRAL_BONUS_FEN,
            "invitee gift path = ¥20 + ¥5 = 2500 fen"
        );

        let ledger = wallet.list_ledger(invitee, 10).await.unwrap();
        assert_eq!(ledger.len(), 2);
        let kinds: Vec<_> = ledger.iter().map(|e| e.kind.as_str()).collect();
        assert!(kinds.contains(&WALLET_KIND_SIGNUP_GRANT));
        assert!(kinds.contains(&WALLET_KIND_REFERRAL_BONUS));
    }

    #[tokio::test]
    async fn double_apply_is_idempotent_no_double_credit() {
        let store = MemoryBillingStore::new().arc();
        let wallet: Arc<dyn WalletStorePort> = store.clone();
        let referral: Arc<dyn ReferralStorePort> = store.clone();

        let (inviter, code) = seed_inviter(store.clone()).await;
        let invitee = Uuid::new_v4();

        let first =
            apply_referral_on_register(wallet.clone(), referral.clone(), invitee, Some(&code))
                .await
                .unwrap();
        assert!(matches!(first, ApplyReferralOutcome::Rewarded { .. }));

        let second =
            apply_referral_on_register(wallet.clone(), referral.clone(), invitee, Some(&code))
                .await
                .unwrap();
        assert!(
            matches!(second, ApplyReferralOutcome::AlreadyBound { .. }),
            "second path must not re-grant"
        );

        let inviter_w = wallet.get_wallet(inviter).await.unwrap().unwrap();
        let invitee_w = wallet.get_wallet(invitee).await.unwrap().unwrap();
        assert_eq!(inviter_w.balance_fen, REFERRAL_BONUS_FEN);
        assert_eq!(invitee_w.balance_fen, REFERRAL_BONUS_FEN);
        assert_eq!(referral.count_rewarded_by_inviter(inviter).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn self_invite_rejected_no_row() {
        let store = MemoryBillingStore::new().arc();
        let wallet: Arc<dyn WalletStorePort> = store.clone();
        let referral: Arc<dyn ReferralStorePort> = store.clone();

        let (user, code) = seed_inviter(store.clone()).await;
        let out = apply_referral_on_register(wallet, referral.clone(), user, Some(&code))
            .await
            .unwrap();
        assert!(matches!(
            out,
            ApplyReferralOutcome::Rejected {
                reason: REFERRAL_REJECT_SELF_INVITE
            }
        ));
        assert!(referral.get_by_invitee(user).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn referral_quota_formula_unit() {
        assert_eq!(referral_quota(0), 5);
        assert_eq!(referral_quota(4999), 5);
        assert_eq!(referral_quota(5000), 6);
        assert_eq!(referral_quota(9999), 6);
        assert_eq!(referral_quota(10_000), 7);
        assert_eq!(referral_quota(-1), 5);
    }
}
