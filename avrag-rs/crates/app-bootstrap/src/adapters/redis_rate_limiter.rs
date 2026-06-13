use app_core::ports::rate_limit::rate_limiter::{RateLimitDecision, RateLimiter};
use async_trait::async_trait;
use redis::AsyncCommands;
use std::sync::Arc;

#[derive(Clone)]
pub struct RedisRateLimitBackend {
    client: redis::Client,
}

impl RedisRateLimitBackend {
    pub fn new(redis_url: &str) -> anyhow::Result<Self> {
        Ok(Self {
            client: redis::Client::open(redis_url)?,
        })
    }

    pub async fn check(&self, key: &str, limit: u32) -> anyhow::Result<RateLimitDecision> {
        let window = chrono::Utc::now().timestamp() / 60;
        let redis_key = format!("rate-limit:{window}:{key}");
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let count: u32 = conn.incr(&redis_key, 1_u32).await?;
        let _: bool = conn.expire(&redis_key, 120).await?;
        let allowed = count <= limit;
        let remaining = limit.saturating_sub(count.min(limit));

        Ok(RateLimitDecision {
            allowed,
            remaining,
            limit,
        })
    }
}

pub fn build_rate_limit_backend(redis_url: &str) -> Option<Arc<RedisRateLimitBackend>> {
    if redis_url.trim().is_empty() {
        return None;
    }
    RedisRateLimitBackend::new(redis_url)
        .ok()
        .map(Arc::new)
}

#[derive(Clone)]
pub struct RedisFixedWindowRateLimiter {
    backend: RedisRateLimitBackend,
    limit: u32,
}

impl RedisFixedWindowRateLimiter {
    pub async fn new(redis_url: String, limit: u32) -> anyhow::Result<Self> {
        Ok(Self {
            backend: RedisRateLimitBackend::new(&redis_url)?,
            limit,
        })
    }

    pub async fn check(&self, key: &str) -> anyhow::Result<RateLimitDecision> {
        self.backend.check(key, self.limit).await
    }
}

#[async_trait]
impl RateLimiter for RedisFixedWindowRateLimiter {
    async fn check(&self, key: &str) -> anyhow::Result<RateLimitDecision> {
        self.backend.check(key, self.limit).await
    }
}
