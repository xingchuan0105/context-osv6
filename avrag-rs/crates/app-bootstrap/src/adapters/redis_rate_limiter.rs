use app_core::ports::rate_limit::rate_limiter::{RateLimitDecision, RateLimiter};
use async_trait::async_trait;
use std::sync::Arc;

const INCR_EXPIRE_LUA: &str = r#"
local n = redis.call('INCR', KEYS[1])
if n == 1 then
  redis.call('EXPIRE', KEYS[1], tonumber(ARGV[1]))
end
return n
"#;

#[derive(Clone)]
pub struct RedisRateLimitBackend {
    conn: avrag_cache_redis::SharedConn,
}

impl RedisRateLimitBackend {
    pub fn new(redis_url: &str) -> anyhow::Result<Self> {
        Ok(Self {
            conn: avrag_cache_redis::SharedConn::from_url(redis_url)?,
        })
    }

    pub async fn check(&self, key: &str, limit: u32) -> anyhow::Result<RateLimitDecision> {
        self.check_window(key, limit, 60).await
    }

    /// Fixed-window check with an arbitrary window size (e.g. 86400 for daily caps).
    pub async fn check_window(
        &self,
        key: &str,
        limit: u32,
        window_secs: u64,
    ) -> anyhow::Result<RateLimitDecision> {
        let window = chrono::Utc::now().timestamp() / window_secs.max(1) as i64;
        let redis_key = format!("rate-limit:{window_secs}:{window}:{key}");
        let mut conn = self.conn.get().await?;
        // One round trip: INCR and EXPIRE must be atomic (Redis INCR pattern).
        let ttl = (window_secs.max(1) * 2) as i64;
        let count: i64 = redis::cmd("EVAL")
            .arg(INCR_EXPIRE_LUA)
            .arg(1)
            .arg(&redis_key)
            .arg(ttl)
            .query_async(&mut conn)
            .await?;
        let count = u32::try_from(count.max(0)).unwrap_or(u32::MAX);
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
    RedisRateLimitBackend::new(redis_url).ok().map(Arc::new)
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
