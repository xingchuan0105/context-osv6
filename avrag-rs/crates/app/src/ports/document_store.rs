use async_trait::async_trait;

#[async_trait]
pub trait DocumentStore: Send + Sync {}
