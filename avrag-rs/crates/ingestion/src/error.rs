use thiserror::Error;

#[derive(Debug, Error)]
pub enum IngestionError {
    #[error("invalid state transition: {from:?} -> {to:?}")]
    InvalidStateTransition {
        from: common::DocumentStatus,
        to: common::DocumentStatus,
    },
    #[error("task source error: {0}")]
    TaskSource(String),
    #[error("audit sink error: {0}")]
    AuditSink(String),
    #[error("state sink error: {0}")]
    StateSink(String),
}
