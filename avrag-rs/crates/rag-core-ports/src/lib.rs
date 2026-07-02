use async_trait::async_trait;

pub mod chat_persistence;
pub mod port_rows;

pub use chat_persistence::{AppendChatTurn, ChatPersistencePort};
pub use port_rows::{
    ConversationHistoryHit, ConversationHistoryScope, DocumentAssetRow, MultimodalChunkRow,
    NotificationCreateParams, UserProfileRow,
};

#[async_trait]
pub trait CachePort: Send + Sync {
    async fn get(&self, key: &str) -> Option<String>;
    async fn set(&self, key: &str, value: &str, ttl_secs: u64) -> Result<(), String>;
}
