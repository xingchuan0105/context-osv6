use redis::AsyncCommands;
use tracing::info;
use uuid::Uuid;

const LOCK_TTL_SECS: u64 = 300;

/// Distributed document lock backed by Redis.
///
/// Uses `SET key 1 NX EX <ttl>` to atomically acquire a lock with a TTL.
/// This prevents multiple workers from processing the same document concurrently.
pub struct DocumentLock {
    client: redis::Client,
}

impl DocumentLock {
    pub fn new(redis_url: &str) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(redis_url)?;
        Ok(Self { client })
    }

    /// Try to acquire a lock for the given document.
    ///
    /// Returns `Some(guard)` if the lock was acquired. The guard releases the
    /// lock when dropped. Returns `None` if the lock is already held.
    /// The lock auto-expires after `LOCK_TTL_SECS` seconds as a safety net.
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
            .arg(LOCK_TTL_SECS)
            .query_async(&mut conn)
            .await?;
        if acquired.as_deref() == Some("OK") {
            Ok(Some(DocumentLockGuard {
                client: self.client.clone(),
                doc_id,
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
}

impl Drop for DocumentLockGuard {
    fn drop(&mut self) {
        let client = self.client.clone();
        let doc_id = self.doc_id;
        let key = lock_key(doc_id);
        tokio::spawn(async move {
            let mut conn = match client.get_multiplexed_async_connection().await {
                Ok(conn) => conn,
                Err(e) => {
                    info!(%doc_id, error = %e, "failed to connect to Redis for lock release");
                    return;
                }
            };
            if let Err(e) = conn.del::<_, ()>(&key).await {
                info!(%doc_id, error = %e, "failed to release document lock");
            }
        });
    }
}

fn lock_key(doc_id: Uuid) -> String {
    format!("doc:lock:{doc_id}")
}
