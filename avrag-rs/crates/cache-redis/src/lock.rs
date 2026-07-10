use redis::AsyncCommands;
use tracing::info;
use uuid::Uuid;

/// Default TTL must exceed typical `AVRAG_INGESTION_TASK_TIMEOUT_SECS` (300) so a
/// long embed/index cannot outlive the lock and allow a second worker to claim
/// the same document. Override with `AVRAG_DOCUMENT_LOCK_TTL_SECS`.
const DEFAULT_LOCK_TTL_SECS: u64 = 900;

fn lock_ttl_secs() -> u64 {
    std::env::var("AVRAG_DOCUMENT_LOCK_TTL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|v| *v >= 60)
        .unwrap_or(DEFAULT_LOCK_TTL_SECS)
}

/// Distributed document lock backed by Redis.
///
/// Uses `SET key 1 NX EX <ttl>` to atomically acquire a lock with a TTL.
/// This prevents multiple workers from processing the same document concurrently.
pub struct DocumentLock {
    client: redis::Client,
    ttl_secs: u64,
}

impl DocumentLock {
    pub fn new(redis_url: &str) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(redis_url)?;
        Ok(Self {
            client,
            ttl_secs: lock_ttl_secs(),
        })
    }

    /// Try to acquire a lock for the given document.
    ///
    /// Returns `Some(guard)` if the lock was acquired. The guard releases the
    /// lock when dropped. Returns `None` if the lock is already held.
    /// The lock auto-expires after the configured TTL as a safety net.
    pub async fn try_acquire(
        &self,
        doc_id: Uuid,
    ) -> Result<Option<DocumentLockGuard>, redis::RedisError> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = lock_key(doc_id);
        let acquired: Option<String> = redis::cmd("SET")
            .arg(&key)
            .arg("1")
            .arg("NX")
            .arg("EX")
            .arg(self.ttl_secs)
            .query_async(&mut conn)
            .await?;
        if acquired.as_deref() == Some("OK") {
            Ok(Some(DocumentLockGuard {
                client: self.client.clone(),
                doc_id,
                released: false,
            }))
        } else {
            Ok(None)
        }
    }
}

/// RAII guard that releases the document lock on drop.
pub struct DocumentLockGuard {
    client: redis::Client,
    doc_id: Uuid,
    released: bool,
}

impl DocumentLockGuard {
    /// Best-effort release before drop so the next claim does not race Drop.
    pub async fn release(mut self) {
        if self.released {
            return;
        }
        let key = lock_key(self.doc_id);
        let mut conn = match self.client.get_multiplexed_async_connection().await {
            Ok(conn) => conn,
            Err(e) => {
                info!(doc_id = %self.doc_id, error = %e, "failed to connect to Redis for lock release");
                self.released = true;
                return;
            }
        };
        if let Err(e) = conn.del::<_, ()>(&key).await {
            info!(doc_id = %self.doc_id, error = %e, "failed to release document lock");
        }
        self.released = true;
    }
}

impl Drop for DocumentLockGuard {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        let client = self.client.clone();
        let doc_id = self.doc_id;
        let key = lock_key(doc_id);
        // Prefer blocking connection on a detached thread so release still runs
        // when the async runtime is shutting down or the timed future was cancelled.
        std::thread::Builder::new()
            .name("redis-doc-lock-release".into())
            .spawn(move || {
                let mut conn = match client.get_connection() {
                    Ok(conn) => conn,
                    Err(e) => {
                        info!(%doc_id, error = %e, "failed to connect to Redis for lock release");
                        return;
                    }
                };
                if let Err(e) = redis::cmd("DEL").arg(&key).query::<()>(&mut conn) {
                    info!(%doc_id, error = %e, "failed to release document lock");
                }
            })
            .ok();
    }
}

fn lock_key(doc_id: Uuid) -> String {
    format!("doc:lock:{doc_id}")
}
