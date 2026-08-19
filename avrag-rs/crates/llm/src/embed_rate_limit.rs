//! Shared embedding budget + acquire gate (ingest waits, query fails fast).
//!
//! Provider RPM/TPM is the account ceiling. Usable budget is a fraction of that
//! so API + worker replicas stay under the vendor cap. Ingest and query are
//! separate RPM lanes sharing one TPM pool.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use crate::rate_limiter::RateLimiter;

const DEFAULT_USABLE_RATIO: f64 = 0.8;
const DEFAULT_INGEST_SHARE: f64 = 0.8;
const DEFAULT_INGEST_WAIT_SECS: u64 = 600;
const DEFAULT_QUERY_WAIT_MS: u64 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbedLane {
    Ingest,
    Query,
}

impl EmbedLane {
    pub fn from_feature(feature: &str) -> Self {
        if feature.starts_with("document_embedding") {
            Self::Ingest
        } else {
            Self::Query
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmbeddingBudget {
    pub provider_rpm: u32,
    pub provider_tpm: u32,
    pub usable_ratio: f64,
    pub ingest_share: f64,
    pub ingest_wait: Duration,
    pub query_wait: Duration,
}

impl EmbeddingBudget {
    pub fn from_env(fallback_rpm: u32, fallback_tpm: u32) -> Self {
        let provider_rpm = env_u32("EMBEDDING_PROVIDER_RPM", fallback_rpm.max(1));
        let provider_tpm = env_u32("EMBEDDING_PROVIDER_TPM", fallback_tpm.max(1));
        Self {
            provider_rpm,
            provider_tpm,
            usable_ratio: env_f64("EMBEDDING_USABLE_RATIO", DEFAULT_USABLE_RATIO).clamp(0.1, 1.0),
            ingest_share: env_f64("EMBEDDING_INGEST_SHARE", DEFAULT_INGEST_SHARE).clamp(0.1, 0.95),
            ingest_wait: Duration::from_secs(env_u64(
                "EMBEDDING_INGEST_WAIT_SECS",
                DEFAULT_INGEST_WAIT_SECS,
            )),
            query_wait: Duration::from_millis(env_u64(
                "EMBEDDING_QUERY_WAIT_MS",
                DEFAULT_QUERY_WAIT_MS,
            )),
        }
    }

    pub fn usable_rpm(&self) -> u32 {
        scale(self.provider_rpm, self.usable_ratio)
    }

    pub fn usable_tpm(&self) -> u32 {
        scale(self.provider_tpm, self.usable_ratio)
    }

    pub fn ingest_rpm(&self) -> u32 {
        scale(self.usable_rpm(), self.ingest_share)
    }

    pub fn query_rpm(&self) -> u32 {
        self.usable_rpm().saturating_sub(self.ingest_rpm()).max(1)
    }

    pub fn max_wait(&self, lane: EmbedLane) -> Duration {
        match lane {
            EmbedLane::Ingest => self.ingest_wait,
            EmbedLane::Query => self.query_wait,
        }
    }
}

fn scale(value: u32, ratio: f64) -> u32 {
    ((value as f64) * ratio).floor().max(1.0) as u32
}

fn env_u32(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
        .max(1)
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[derive(Debug, Clone)]
pub struct EmbedRateRequest {
    pub lane: EmbedLane,
    pub tokens: usize,
}

#[async_trait]
pub trait EmbedRateGate: Send + Sync {
    async fn acquire(&self, req: EmbedRateRequest) -> anyhow::Result<()>;
}

/// Process-local dual-lane gate. Used when Redis is absent, and as Redis fallback.
pub struct LocalEmbedRateGate {
    ingest: RateLimiter,
    query: RateLimiter,
    budget: EmbeddingBudget,
}

impl LocalEmbedRateGate {
    pub fn new(budget: EmbeddingBudget) -> Self {
        let tpm = budget.usable_tpm();
        Self {
            ingest: RateLimiter::new(budget.ingest_rpm(), tpm),
            query: RateLimiter::new(budget.query_rpm(), tpm),
            budget,
        }
    }

    fn limiter(&self, lane: EmbedLane) -> &RateLimiter {
        match lane {
            EmbedLane::Ingest => &self.ingest,
            EmbedLane::Query => &self.query,
        }
    }
}

#[async_trait]
impl EmbedRateGate for LocalEmbedRateGate {
    async fn acquire(&self, req: EmbedRateRequest) -> anyhow::Result<()> {
        let deadline = tokio::time::Instant::now() + self.budget.max_wait(req.lane);
        let limiter = self.limiter(req.lane);
        loop {
            match limiter.try_acquire_or_wait_ms(req.tokens) {
                Ok(_) => return Ok(()),
                Err(wait_ms) => {
                    let wait = Duration::from_millis(wait_ms.min(500));
                    if tokio::time::Instant::now() + wait > deadline {
                        anyhow::bail!(embed_wait_timeout_message(req.lane));
                    }
                    tokio::time::sleep(wait).await;
                }
            }
        }
    }
}

pub fn embed_wait_timeout_message(lane: EmbedLane) -> String {
    match lane {
        EmbedLane::Ingest => {
            "Embedding ingest rate limit: waited out the budget (too many requests per minute)"
                .to_string()
        }
        EmbedLane::Query => {
            "Embedding query rate limit: no slot within wait window".to_string()
        }
    }
}

pub fn shared_local_embed_gate(budget: EmbeddingBudget) -> Arc<dyn EmbedRateGate> {
    Arc::new(LocalEmbedRateGate::new(budget))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RateLimiter;

    #[test]
    fn budget_splits_2000_rpm_account() {
        let budget = EmbeddingBudget {
            provider_rpm: 2000,
            provider_tpm: 1_000_000,
            usable_ratio: 0.8,
            ingest_share: 0.8,
            ingest_wait: Duration::from_secs(600),
            query_wait: Duration::from_millis(200),
        };
        assert_eq!(budget.usable_rpm(), 1600);
        assert_eq!(budget.ingest_rpm(), 1280);
        assert_eq!(budget.query_rpm(), 320);
        assert_eq!(budget.usable_tpm(), 800_000);
        assert!(budget.ingest_rpm() + budget.query_rpm() <= budget.usable_rpm());
        assert!(budget.usable_rpm() < budget.provider_rpm);
    }

    #[test]
    fn document_embedding_is_ingest_lane() {
        assert_eq!(
            EmbedLane::from_feature("document_embedding"),
            EmbedLane::Ingest
        );
        assert_eq!(
            EmbedLane::from_feature("document_embedding_mm"),
            EmbedLane::Ingest
        );
        assert_eq!(EmbedLane::from_feature("query"), EmbedLane::Query);
    }

    #[test]
    fn ingest_reports_wait_instead_of_hard_fail() {
        let limiter = RateLimiter::new(1, 10_000);
        assert!(limiter.try_acquire_or_wait_ms(1).is_ok());
        let wait = limiter.try_acquire_or_wait_ms(1).expect_err("second take waits");
        assert!(wait > 0);
    }

    #[tokio::test]
    async fn query_times_out_fast_when_bucket_empty() {
        let budget = EmbeddingBudget {
            provider_rpm: 10,
            provider_tpm: 100_000,
            usable_ratio: 1.0,
            ingest_share: 0.8,
            ingest_wait: Duration::from_secs(2),
            query_wait: Duration::from_millis(30),
        };
        let gate = LocalEmbedRateGate::new(budget);
        for _ in 0..budget.query_rpm() {
            gate.acquire(EmbedRateRequest {
                lane: EmbedLane::Query,
                tokens: 1,
            })
            .await
            .expect("fill query bucket");
        }
        let err = gate
            .acquire(EmbedRateRequest {
                lane: EmbedLane::Query,
                tokens: 1,
            })
            .await
            .expect_err("query must fail fast");
        assert!(err.to_string().contains("query rate limit"));
    }
}
