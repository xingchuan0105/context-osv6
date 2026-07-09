use async_trait::async_trait;

pub mod chat_persistence;
pub mod embedding;
pub mod llm_types;
pub mod port_rows;

pub use chat_persistence::{
    AppendChatTurn, ChatCatalogPort, ChatContentPort, ChatPersistencePort, ChatSideEffectPort,
    MessagePort, ProfilePort, SessionPort,
};
pub use embedding::{
    EmbeddingPort, MultiModalEmbeddingInput, MultiModalRerankDocument, PlannerPort, RerankPort,
    RerankResult,
};
pub use llm_types::{LlmUsage, SynthesisOutput};
pub use port_rows::{
    ConversationHistoryHit, ConversationHistoryScope, DocumentAssetRow, MultimodalChunkRow,
    NotificationCreateParams, UserProfileRow,
};

/// Object-safe string cache (JSON serialization stays at the call site).
#[async_trait]
pub trait CachePort: Send + Sync {
    async fn get(&self, key: &str) -> Option<String>;
    async fn set(&self, key: &str, value: &str, ttl_secs: u64) -> Result<(), String>;
}
