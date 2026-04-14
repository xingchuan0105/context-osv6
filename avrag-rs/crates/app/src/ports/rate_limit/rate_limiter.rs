use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitDecision {
    pub allowed: bool,
    pub remaining: u32,
    pub limit: u32,
}

#[async_trait]
pub trait RateLimiter: Send + Sync {
    async fn check(&self, key: &str) -> anyhow::Result<RateLimitDecision>;
}
