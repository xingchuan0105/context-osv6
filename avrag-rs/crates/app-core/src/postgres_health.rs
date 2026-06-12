use async_trait::async_trait;

/// Postgres connectivity probe — implementations live in bootstrap adapters.
#[async_trait]
pub trait PostgresHealthPort: Send + Sync {
    async fn ping(&self) -> Result<(), String>;
}
