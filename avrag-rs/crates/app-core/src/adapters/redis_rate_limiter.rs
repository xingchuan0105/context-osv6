use redis::AsyncCommands;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitDecision {
    pub allowed: bool,
    pub remaining: u32,
    pub limit: u32,
}

#[derive(Clone)]
pub struct RedisFixedWindowRateLimiter {
    client: redis::Client,
    limit: u32,
}

impl RedisFixedWindowRateLimiter {
    pub async fn new(redis_url: String, limit: u32) -> anyhow::Result<Self> {
        Ok(Self {
            client: redis::Client::open(redis_url)?,
            limit,
        })
    }

    pub async fn check(&self, key: &str) -> anyhow::Result<RateLimitDecision> {
        let window = chrono::Utc::now().timestamp() / 60;
        let redis_key = format!("rate-limit:{window}:{key}");
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let count: u32 = conn.incr(&redis_key, 1_u32).await?;
        let _: bool = conn.expire(&redis_key, 120).await?;
        let allowed = count <= self.limit;
        let remaining = self.limit.saturating_sub(count.min(self.limit));

        Ok(RateLimitDecision {
            allowed,
            remaining,
            limit: self.limit,
        })
    }
}
