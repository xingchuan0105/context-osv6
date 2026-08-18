use async_trait::async_trait;
use avrag_rag_core_ports::CachePort;
use redis::AsyncCommands;
use serde::de::DeserializeOwned;
use serde::Serialize;
use tracing::warn;

/// Generic Redis-backed cache store.
#[derive(Clone)]
pub struct CacheStore {
    conn: crate::SharedConn,
}

impl std::fmt::Debug for CacheStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheStore").finish_non_exhaustive()
    }
}

impl CacheStore {
    pub fn new(redis_url: &str) -> Result<Self, redis::RedisError> {
        Ok(Self {
            conn: crate::SharedConn::from_url(redis_url)?,
        })
    }

    /// Get a JSON-deserialized value from cache.
    pub async fn get_json<T: DeserializeOwned>(
        &self,
        key: &str,
    ) -> Result<Option<T>, redis::RedisError> {
        let mut conn = self.conn.get().await?;
        let raw: Option<String> = conn.get(key).await?;
        match raw {
            Some(json_str) => match serde_json::from_str(&json_str) {
                Ok(value) => Ok(Some(value)),
                Err(e) => {
                    warn!(key, error = %e, "failed to deserialize cached value, treating as miss");
                    let _: () = conn.del(key).await?;
                    Ok(None)
                }
            },
            None => Ok(None),
        }
    }

    /// Set a JSON-serialized value in cache with TTL.
    pub async fn set_json<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        ttl_secs: u64,
    ) -> Result<(), redis::RedisError> {
        let mut conn = self.conn.get().await?;
        let json_str = serde_json::to_string(value).unwrap_or_default();
        let _: () = redis::cmd("SET")
            .arg(key)
            .arg(json_str)
            .arg("EX")
            .arg(ttl_secs)
            .query_async(&mut conn)
            .await?;
        Ok(())
    }

    /// Delete a key from cache.
    pub async fn delete(&self, key: &str) -> Result<(), redis::RedisError> {
        let mut conn = self.conn.get().await?;
        let _: () = conn.del(key).await?;
        Ok(())
    }
}

#[async_trait]
impl CachePort for CacheStore {
    async fn get(&self, key: &str) -> Option<String> {
        let mut conn = self.conn.get().await.ok()?;
        conn.get(key).await.ok().flatten()
    }

    async fn set(&self, key: &str, value: &str, ttl_secs: u64) -> Result<(), String> {
        let mut conn = self.conn.get().await.map_err(|e| e.to_string())?;
        redis::cmd("SET")
            .arg(key)
            .arg(value)
            .arg("EX")
            .arg(ttl_secs)
            .query_async(&mut conn)
            .await
            .map_err(|e| e.to_string())
    }
}
