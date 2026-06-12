use async_trait::async_trait;

// Re-export ContentStore and related types from common.
pub use common::{ContentStore, ContentStoreError, IndexedChunk};

#[async_trait]
pub trait CachePort: Send + Sync {
    async fn get(&self, key: &str) -> Option<String>;
    async fn set(&self, key: &str, value: &str, ttl_secs: u64) -> Result<(), String>;
}
