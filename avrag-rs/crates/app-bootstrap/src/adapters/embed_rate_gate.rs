//! Redis token-bucket embed gate shared by API + worker replicas.
//!
//! Falls back to the process-local gate when Redis is down so ingest still waits
//! instead of failing closed on a cache blip.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use avrag_llm::{
    EmbedLane, EmbedRateGate, EmbedRateRequest, EmbeddingBudget, LocalEmbedRateGate,
    embed_wait_timeout_message,
};
use tracing::warn;

const ACQUIRE_LUA: &str = r#"
local key = KEYS[1]
local capacity = tonumber(ARGV[1])
local refill_per_ms = tonumber(ARGV[2])
local now_ms = tonumber(ARGV[3])
local cost = tonumber(ARGV[4])
local data = redis.call('HMGET', key, 'tokens', 'ts')
local tokens = tonumber(data[1])
local ts = tonumber(data[2])
if tokens == nil then
  tokens = capacity
  ts = now_ms
end
local elapsed = math.max(0, now_ms - ts)
tokens = math.min(capacity, tokens + elapsed * refill_per_ms)
if tokens >= cost then
  tokens = tokens - cost
  redis.call('HSET', key, 'tokens', tokens, 'ts', now_ms)
  redis.call('PEXPIRE', key, 120000)
  return {1, 0}
end
local wait_ms = 1
if refill_per_ms > 0 then
  wait_ms = math.ceil((cost - tokens) / refill_per_ms)
end
redis.call('HSET', key, 'tokens', tokens, 'ts', now_ms)
redis.call('PEXPIRE', key, 120000)
return {0, wait_ms}
"#;

const REFUND_LUA: &str = r#"
local key = KEYS[1]
local capacity = tonumber(ARGV[1])
local now_ms = tonumber(ARGV[2])
local cost = tonumber(ARGV[3])
local data = redis.call('HMGET', key, 'tokens', 'ts')
local tokens = tonumber(data[1]) or 0
local ts = tonumber(data[2]) or now_ms
tokens = math.min(capacity, tokens + cost)
redis.call('HSET', key, 'tokens', tokens, 'ts', ts)
redis.call('PEXPIRE', key, 120000)
return 1
"#;

pub struct RedisEmbedRateGate {
    conn: avrag_cache_redis::SharedConn,
    budget: EmbeddingBudget,
    fallback: LocalEmbedRateGate,
}

impl RedisEmbedRateGate {
    pub fn new(redis_url: &str, budget: EmbeddingBudget) -> anyhow::Result<Self> {
        Ok(Self {
            conn: avrag_cache_redis::SharedConn::from_url(redis_url)?,
            fallback: LocalEmbedRateGate::new(budget),
            budget,
        })
    }

    fn rpm_key(&self, lane: EmbedLane) -> &'static str {
        match lane {
            EmbedLane::Ingest => "embed:rpm:ingest",
            EmbedLane::Query => "embed:rpm:query",
        }
    }

    fn rpm_capacity(&self, lane: EmbedLane) -> u32 {
        match lane {
            EmbedLane::Ingest => self.budget.ingest_rpm(),
            EmbedLane::Query => self.budget.query_rpm(),
        }
    }

    async fn try_bucket(
        &self,
        key: &str,
        capacity: u32,
        cost: f64,
    ) -> anyhow::Result<Result<(), u64>> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let refill_per_ms = (capacity as f64) / 60_000.0;
        let mut conn = self.conn.get().await?;
        let reply: Vec<i64> = redis::cmd("EVAL")
            .arg(ACQUIRE_LUA)
            .arg(1)
            .arg(key)
            .arg(capacity)
            .arg(refill_per_ms)
            .arg(now_ms)
            .arg(cost)
            .query_async(&mut conn)
            .await?;
        let allowed = reply.first().copied().unwrap_or(0) == 1;
        let wait_ms = reply.get(1).copied().unwrap_or(1).max(1) as u64;
        if allowed {
            Ok(Ok(()))
        } else {
            Ok(Err(wait_ms))
        }
    }

    async fn refund(&self, key: &str, capacity: u32, cost: f64) {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let Ok(mut conn) = self.conn.get().await else {
            return;
        };
        let _: Result<i64, _> = redis::cmd("EVAL")
            .arg(REFUND_LUA)
            .arg(1)
            .arg(key)
            .arg(capacity)
            .arg(now_ms)
            .arg(cost)
            .query_async(&mut conn)
            .await;
    }

    async fn acquire_redis(&self, req: &EmbedRateRequest) -> anyhow::Result<()> {
        let deadline = tokio::time::Instant::now() + self.budget.max_wait(req.lane);
        let rpm_key = self.rpm_key(req.lane);
        let rpm_cap = self.rpm_capacity(req.lane);
        let tpm_key = "embed:tpm:global";
        let tpm_cap = self.budget.usable_tpm();
        let tpm_cost = (req.tokens as f64).max(1.0);
        loop {
            match self.try_bucket(rpm_key, rpm_cap, 1.0).await? {
                Err(wait_ms) => {
                    sleep_or_timeout(req.lane, wait_ms, deadline).await?;
                    continue;
                }
                Ok(()) => match self.try_bucket(tpm_key, tpm_cap, tpm_cost).await? {
                    Ok(()) => return Ok(()),
                    Err(wait_ms) => {
                        self.refund(rpm_key, rpm_cap, 1.0).await;
                        sleep_or_timeout(req.lane, wait_ms, deadline).await?;
                    }
                },
            }
        }
    }
}

async fn sleep_or_timeout(
    lane: EmbedLane,
    wait_ms: u64,
    deadline: tokio::time::Instant,
) -> anyhow::Result<()> {
    let wait = Duration::from_millis(wait_ms.min(500));
    if tokio::time::Instant::now() + wait > deadline {
        anyhow::bail!(embed_wait_timeout_message(lane));
    }
    tokio::time::sleep(wait).await;
    Ok(())
}

#[async_trait]
impl EmbedRateGate for RedisEmbedRateGate {
    async fn acquire(&self, req: EmbedRateRequest) -> anyhow::Result<()> {
        match self.acquire_redis(&req).await {
            Ok(()) => Ok(()),
            Err(err) if is_redis_transport_error(&err) => {
                warn!(error = %err, "embed rate gate redis failed; using local bucket");
                self.fallback.acquire(req).await
            }
            Err(err) => Err(err),
        }
    }
}

fn is_redis_transport_error(err: &anyhow::Error) -> bool {
    err.downcast_ref::<redis::RedisError>().is_some()
        || err.to_string().contains("redis")
        || err.to_string().contains("connection")
}

pub fn build_embed_rate_gate(
    redis_url: &str,
    budget: EmbeddingBudget,
) -> Arc<dyn EmbedRateGate> {
    if redis_url.trim().is_empty() {
        return avrag_llm::shared_local_embed_gate(budget);
    }
    match RedisEmbedRateGate::new(redis_url, budget) {
        Ok(gate) => Arc::new(gate),
        Err(err) => {
            warn!(error = %err, "embed rate gate redis unavailable; local only");
            avrag_llm::shared_local_embed_gate(budget)
        }
    }
}
