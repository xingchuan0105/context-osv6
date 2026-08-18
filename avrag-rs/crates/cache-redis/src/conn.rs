//! Shared lazily-initialized `ConnectionManager` — one multiplexed, self-
//! reconnecting connection per client instead of a fresh TCP connection per op.
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// First connect (and reconnect after cooldown) waits at most this long.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(1);

/// After a failed connect, further `get()` calls fail immediately so a down
/// Redis cannot add this timeout to every remaining Redis op (HTTP rate-limit
/// or ingest lock) when Redis is down.
const FAIL_COOLDOWN: Duration = Duration::from_secs(5);

fn unavailable() -> redis::RedisError {
    redis::RedisError::from((
        redis::ErrorKind::IoError,
        "redis unavailable (connect failed; circuit open)",
    ))
}

fn connect_config() -> redis::aio::ConnectionManagerConfig {
    // Default `get_connection_manager()` uses backon factor=100 on a 1s min
    // delay (second retry ~100s). Fail fast so HTTP rate-limit can fall back
    // to the in-process limiter.
    redis::aio::ConnectionManagerConfig::new()
        .set_connection_timeout(CONNECT_TIMEOUT)
        .set_number_of_retries(1)
        .set_max_delay(200)
}

/// Clone-cheap handle: all clones share the same underlying connection.
#[derive(Clone)]
pub struct SharedConn {
    client: redis::Client,
    manager: Arc<tokio::sync::OnceCell<redis::aio::ConnectionManager>>,
    last_fail: Arc<Mutex<Option<Instant>>>,
}

impl SharedConn {
    pub fn new(client: redis::Client) -> Self {
        Self {
            client,
            manager: Arc::new(tokio::sync::OnceCell::new()),
            last_fail: Arc::new(Mutex::new(None)),
        }
    }

    pub fn from_url(redis_url: &str) -> Result<Self, redis::RedisError> {
        Ok(Self::new(redis::Client::open(redis_url)?))
    }

    /// Get the shared connection, establishing it on first use.
    ///
    /// A failed connect is remembered for [`FAIL_COOLDOWN`]: later `get()`
    /// returns immediately so one dead Redis does not tax every agent round.
    /// After the cooldown, one more connect is attempted (Redis restart).
    pub async fn get(&self) -> Result<redis::aio::ConnectionManager, redis::RedisError> {
        if let Some(manager) = self.manager.get() {
            return Ok(manager.clone());
        }
        if let Ok(guard) = self.last_fail.lock() {
            if guard.is_some_and(|at| at.elapsed() < FAIL_COOLDOWN) {
                return Err(unavailable());
            }
        }

        let client = self.client.clone();
        match self
            .manager
            .get_or_try_init(|| async move {
                client
                    .get_connection_manager_with_config(connect_config())
                    .await
            })
            .await
        {
            Ok(manager) => {
                if let Ok(mut guard) = self.last_fail.lock() {
                    *guard = None;
                }
                Ok(manager.clone())
            }
            Err(error) => {
                if let Ok(mut guard) = self.last_fail.lock() {
                    *guard = Some(Instant::now());
                }
                Err(error)
            }
        }
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

    #[tokio::test]
    async fn dead_port_followup_gets_do_not_repay_connect_timeout() {
        let conn = SharedConn::from_url("redis://127.0.0.1:1").expect("parse url");
        assert!(conn.get().await.is_err(), "first get must fail");

        let started = std::time::Instant::now();
        for _ in 0..20 {
            assert!(conn.get().await.is_err());
        }
        let elapsed = started.elapsed();
        assert!(
            elapsed < Duration::from_millis(200),
            "20 follow-up gets on an open circuit took {elapsed:?} (must not reconnect)"
        );
    }
}
