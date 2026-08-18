//! Process-local `CachePort` for chat-path embedding / planner / search hits.
//!
//! Redis is not on this path: a user query is one event, not one Redis round
//! trip per embed text or retrieve tool. Cross-replica sharing is out of scope
//! until there is more than one api process that needs it.
use super::CachePort;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

struct Entry {
    value: String,
    expires_at: Option<Instant>,
}

/// Process-local string cache.
///
/// ponytail: unbounded HashMap; add LRU eviction if a long-lived replica
/// retains too many embed/search keys.
#[derive(Default)]
pub struct MemoryCache {
    data: Mutex<HashMap<String, Entry>>,
}

impl MemoryCache {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl CachePort for MemoryCache {
    async fn get(&self, key: &str) -> Option<String> {
        let mut data = self.data.lock().unwrap_or_else(|p| p.into_inner());
        let expired = data
            .get(key)
            .is_some_and(|e| e.expires_at.is_some_and(|t| t <= Instant::now()));
        if expired {
            data.remove(key);
            return None;
        }
        data.get(key).map(|e| e.value.clone())
    }

    async fn set(&self, key: &str, value: &str, ttl_secs: u64) -> Result<(), String> {
        let expires_at = (ttl_secs > 0).then(|| Instant::now() + Duration::from_secs(ttl_secs));
        let mut data = self.data.lock().unwrap_or_else(|p| p.into_inner());
        data.insert(
            key.to_string(),
            Entry {
                value: value.to_string(),
                expires_at,
            },
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CachePort;

    #[tokio::test]
    async fn set_then_get_returns_value() {
        let cache = MemoryCache::new();
        cache.set("k", "v", 60).await.unwrap();
        assert_eq!(cache.get("k").await.as_deref(), Some("v"));
    }

    #[tokio::test]
    async fn missing_key_is_none() {
        let cache = MemoryCache::new();
        assert!(cache.get("missing").await.is_none());
    }

    #[tokio::test]
    async fn expired_key_is_evicted() {
        let cache = MemoryCache::new();
        cache.set("k", "v", 1).await.unwrap();
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert!(cache.get("k").await.is_none());
    }

    #[tokio::test]
    async fn zero_ttl_does_not_expire() {
        let cache = MemoryCache::new();
        cache.set("k", "v", 0).await.unwrap();
        assert_eq!(cache.get("k").await.as_deref(), Some("v"));
    }
}
