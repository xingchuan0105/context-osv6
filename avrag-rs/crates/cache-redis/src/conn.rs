//! Shared lazily-initialized `ConnectionManager` — one multiplexed, self-
//! reconnecting connection per client instead of a fresh TCP connection per op.
use std::sync::Arc;
use std::time::Duration;

/// Clone-cheap handle: all clones share the same underlying connection.
#[derive(Clone)]
pub struct SharedConn {
    client: redis::Client,
    manager: Arc<tokio::sync::OnceCell<redis::aio::ConnectionManager>>,
}

impl SharedConn {
    pub fn new(client: redis::Client) -> Self {
        Self {
            client,
            manager: Arc::new(tokio::sync::OnceCell::new()),
        }
    }

    pub fn from_url(redis_url: &str) -> Result<Self, redis::RedisError> {
        Ok(Self::new(redis::Client::open(redis_url)?))
    }

    /// Get the shared connection, establishing it on first use.
    ///
    /// Default `get_connection_manager()` uses backon factor=100 on a 1s min
    /// delay (second retry ~100s). E2E's `redis://127.0.0.1:1` blackhole then
    /// stalls every chat in the rate-limit middleware. Fail fast so callers
    /// can fall back (memory limiter, skip cache).
    pub async fn get(&self) -> Result<redis::aio::ConnectionManager, redis::RedisError> {
        let client = self.client.clone();
        self.manager
            .get_or_try_init(|| async move {
                let config = redis::aio::ConnectionManagerConfig::new()
                    .set_connection_timeout(Duration::from_secs(1))
                    .set_number_of_retries(1)
                    .set_max_delay(200);
                client.get_connection_manager_with_config(config).await
            })
            .await
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dead_port_fails_fast() {
        let conn = SharedConn::from_url("redis://127.0.0.1:1").expect("parse url");
        let started = std::time::Instant::now();
        let result = conn.get().await;
        let elapsed = started.elapsed();
        assert!(result.is_err(), "blackhole must error");
        assert!(
            elapsed < Duration::from_secs(3),
            "blackhole connect took {elapsed:?}"
        );
    }
}
