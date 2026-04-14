use async_trait::async_trait;

#[async_trait]
pub trait ChatStore: Send + Sync {}
