#[derive(Debug, Error)]
pub enum PgStorageError {
    #[error("authorization failure: {0}")]
    Auth(#[from] AuthError),
    #[error("postgres error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("{0}")]
    NotFound(String),
}
