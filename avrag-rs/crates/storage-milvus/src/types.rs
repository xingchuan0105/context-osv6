use thiserror::Error;

#[derive(Debug, Error)]
pub enum MilvusStorageError {
    #[error("Milvus backend error: {message}")]
    Backend { message: String },
    #[error("Tenant access denied: {message}")]
    TenantAccessDenied { message: String },
    #[error("HTTP client error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Configuration error: {message}")]
    Config { message: String },
    #[error("Validation error: {message}")]
    Validation { message: String },
}

pub type Result<T> = std::result::Result<T, MilvusStorageError>;
