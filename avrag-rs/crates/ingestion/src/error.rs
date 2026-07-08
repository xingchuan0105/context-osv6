use thiserror::Error;

#[derive(Debug, Error)]
pub enum IngestionError {
    #[error("invalid state transition: {from:?} -> {to:?}")]
    InvalidStateTransition {
        from: contracts::documents::DocumentStatus,
        to: contracts::documents::DocumentStatus,
    },
    #[error("task source error: {0}")]
    TaskSource(String),
    #[error("audit sink error: {0}")]
    AuditSink(String),
    #[error("state sink error: {0}")]
    StateSink(String),
}

impl From<uuid::Error> for IngestionError {
    fn from(error: uuid::Error) -> Self {
        IngestionError::StateSink(error.to_string())
    }
}
