//! Crate-wide serialized access to `PLATFORM_OFFICIAL_RATES_JSON` for tests.
//!
//! One process-global mutex (review round-8 S6: the per-test-module Mutexes in
//! `wallet.rs` and `wallet_pricing.rs` were two independent locks guarding the
//! same process-wide env var, so parallel tests could interleave set/restore).
//! The RAII guard also restores the prior value on panic unwind — a failing
//! test can no longer poison the env for the rest of the suite.

use std::sync::{Mutex, MutexGuard};

static RATES_ENV_LOCK: Mutex<()> = Mutex::new(());

/// Set `PLATFORM_OFFICIAL_RATES_JSON` (None = removed) while holding the
/// crate-wide lock. The returned guard restores the prior value on drop,
/// including through panic unwind.
pub fn set_rates_env(raw: &str) -> RatesEnvGuard {
    let guard = lock();
    let prev = std::env::var_os("PLATFORM_OFFICIAL_RATES_JSON");
    // SAFETY: every billing test mutates the var behind RATES_ENV_LOCK.
    unsafe { std::env::set_var("PLATFORM_OFFICIAL_RATES_JSON", raw) };
    RatesEnvGuard { prev, _lock: guard }
}

pub struct RatesEnvGuard {
    prev: Option<std::ffi::OsString>,
    _lock: MutexGuard<'static, ()>,
}

impl Drop for RatesEnvGuard {
    fn drop(&mut self) {
        // SAFETY: the crate-wide lock is still held by `_lock`.
        unsafe {
            match self.prev.take() {
                Some(value) => std::env::set_var("PLATFORM_OFFICIAL_RATES_JSON", value),
                None => std::env::remove_var("PLATFORM_OFFICIAL_RATES_JSON"),
            }
        }
    }
}

fn lock() -> MutexGuard<'static, ()> {
    // Recover from a poisoned lock: the env-restore is idempotent and the
    // panic already failed its test — later tests still need serialization.
    RATES_ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}