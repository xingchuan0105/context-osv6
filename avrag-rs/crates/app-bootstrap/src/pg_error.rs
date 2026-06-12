use avrag_storage_pg::PgStorageError;
use common::AppError;

pub fn map_pg_error(error: PgStorageError) -> AppError {
    match error {
        PgStorageError::NotFound(message) => AppError::not_found("not_found", message),
        other => AppError::internal(other.to_string()),
    }
}
