use async_trait::async_trait;
use avrag_rag_core_ports::CachePort;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 本地内存缓存
///
/// 替代 Redis，使用进程内 HashMap 实现缓存
#[derive(Clone)]
pub struct LocalCache {
    data: Arc<RwLock<HashMap<String, CacheEntry>>>,
}

#[derive(Clone)]
struct CacheEntry {
    value: String,
    expires_at: Option<std::time::Instant>,
}

impl LocalCache {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 清理过期缓存条目
    pub async fn cleanup_expired(&self) {
        let mut data = self.data.write().await;
        let now = std::time::Instant::now();
        data.retain(|_, entry| {
            entry.expires_at.map_or(true, |expires| expires > now)
        });
    }
}

impl Default for LocalCache {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CachePort for LocalCache {
    async fn get(&self, key: &str) -> Option<String> {
        let data = self.data.read().await;

        if let Some(entry) = data.get(key) {
            // 检查是否过期
            if let Some(expires) = entry.expires_at {
                if expires <= std::time::Instant::now() {
                    return None;
                }
            }
            Some(entry.value.clone())
        } else {
            None
        }
    }

    async fn set(&self, key: &str, value: &str, ttl_secs: u64) -> Result<(), String> {
        let mut data = self.data.write().await;

        let expires_at = if ttl_secs > 0 {
            Some(std::time::Instant::now() + std::time::Duration::from_secs(ttl_secs))
        } else {
            None
        };

        data.insert(
            key.to_string(),
            CacheEntry {
                value: value.to_string(),
                expires_at,
            },
        );

        Ok(())
    }
}
