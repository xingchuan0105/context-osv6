use thiserror::Error;

/// Typed ingestion failures — prefer a concrete variant over string-erased bags.
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
    #[error("storage error: {0}")]
    Storage(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("security scan failed: {0}")]
    Security(String),
    #[error("indexing error: {0}")]
    Index(String),
    #[error("embedding error: {0}")]
    Embedding(String),
    #[error("invalid id: {0}")]
    InvalidId(String),
    #[error("task timeout after {0}s")]
    Timeout(u64),
    #[error("document seed not found")]
    SeedNotFound,
    #[error("internal: {0}")]
    Internal(String),
}

impl From<uuid::Error> for IngestionError {
    fn from(error: uuid::Error) -> Self {
        IngestionError::InvalidId(error.to_string())
    }
}

impl IngestionError {
    pub fn storage(error: impl ToString) -> Self {
        Self::Storage(error.to_string())
    }

    pub fn parse(error: impl ToString) -> Self {
        Self::Parse(error.to_string())
    }

    pub fn security(error: impl ToString) -> Self {
        Self::Security(error.to_string())
    }

    pub fn index(error: impl ToString) -> Self {
        Self::Index(error.to_string())
    }

    pub fn embedding(error: impl ToString) -> Self {
        Self::Embedding(error.to_string())
    }

    pub fn internal(error: impl ToString) -> Self {
        Self::Internal(error.to_string())
    }
}
