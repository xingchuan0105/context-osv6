use thiserror::Error;

/// Storage failure class — matchable without parsing message strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageKind {
    Database,
    ObjectStore,
    LeaseLost,
    Other,
}

/// Security / validation failure class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityKind {
    Malware,
    ZipBomb,
    ScannerUnavailable,
    UploadTampered,
    Other,
}

/// Parse failure class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseKind {
    Local,
    OfficeService,
    IrValidation,
    Other,
}

/// Embedding / index failure class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexKind {
    Embedding,
    VectorWrite,
    Graph,
    Other,
}

/// Typed ingestion failures with matchable kinds for metrics/retry policy.
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
    #[error("storage error ({kind:?}): {message}")]
    Storage {
        kind: StorageKind,
        message: String,
    },
    #[error("parse error ({kind:?}): {message}")]
    Parse { kind: ParseKind, message: String },
    #[error("security ({kind:?}): {message}")]
    Security {
        kind: SecurityKind,
        message: String,
    },
    #[error("indexing error ({kind:?}): {message}")]
    Index { kind: IndexKind, message: String },
    #[error("embedding error ({kind:?}): {message}")]
    Embedding { kind: IndexKind, message: String },
    #[error("invalid id: {0}")]
    InvalidId(String),
    #[error("task timeout after {0}s")]
    Timeout(u64),
    /// Document is locked by another worker (or a stale lock). Retryable — must
    /// **not** be treated as successful completion by the worker runtime.
    #[error("document locked by another worker: {0}")]
    DocumentLocked(String),
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
    /// Stable metric / log class (no free-form message parsing).
    pub fn class(&self) -> &'static str {
        match self {
            Self::InvalidStateTransition { .. } => "invalid_state_transition",
            Self::TaskSource(_) => "task_source",
            Self::AuditSink(_) => "audit_sink",
            Self::Storage { kind, .. } => match kind {
                StorageKind::Database => "storage_database",
                StorageKind::ObjectStore => "storage_object",
                StorageKind::LeaseLost => "storage_lease_lost",
                StorageKind::Other => "storage_other",
            },
            Self::Parse { kind, .. } => match kind {
                ParseKind::Local => "parse_local",
                ParseKind::OfficeService => "parse_office",
                ParseKind::IrValidation => "parse_ir",
                ParseKind::Other => "parse_other",
            },
            Self::Security { kind, .. } => match kind {
                SecurityKind::Malware => "security_malware",
                SecurityKind::ZipBomb => "security_zip_bomb",
                SecurityKind::ScannerUnavailable => "security_scanner_down",
                SecurityKind::UploadTampered => "security_upload_tampered",
                SecurityKind::Other => "security_other",
            },
            Self::Index { kind, .. } | Self::Embedding { kind, .. } => match kind {
                IndexKind::Embedding => "index_embedding",
                IndexKind::VectorWrite => "index_vector_write",
                IndexKind::Graph => "index_graph",
                IndexKind::Other => "index_other",
            },
            Self::InvalidId(_) => "invalid_id",
            Self::Timeout(_) => "timeout",
            Self::DocumentLocked(_) => "document_locked",
            Self::SeedNotFound => "seed_not_found",
            Self::Internal(_) => "internal",
        }
    }

    pub fn document_locked(message: impl ToString) -> Self {
        Self::DocumentLocked(message.to_string())
    }

    pub fn storage(error: impl ToString) -> Self {
        Self::Storage {
            kind: StorageKind::Other,
            message: error.to_string(),
        }
    }

    pub fn storage_database(error: impl ToString) -> Self {
        Self::Storage {
            kind: StorageKind::Database,
            message: error.to_string(),
        }
    }

    pub fn storage_object(error: impl ToString) -> Self {
        Self::Storage {
            kind: StorageKind::ObjectStore,
            message: error.to_string(),
        }
    }

    pub fn storage_lease_lost(error: impl ToString) -> Self {
        Self::Storage {
            kind: StorageKind::LeaseLost,
            message: error.to_string(),
        }
    }

    pub fn parse(error: impl ToString) -> Self {
        Self::Parse {
            kind: ParseKind::Other,
            message: error.to_string(),
        }
    }

    pub fn parse_local(error: impl ToString) -> Self {
        Self::Parse {
            kind: ParseKind::Local,
            message: error.to_string(),
        }
    }

    pub fn parse_office(error: impl ToString) -> Self {
        Self::Parse {
            kind: ParseKind::OfficeService,
            message: error.to_string(),
        }
    }

    pub fn security(error: impl ToString) -> Self {
        Self::Security {
            kind: SecurityKind::Other,
            message: error.to_string(),
        }
    }

    pub fn malware(threat_name: impl ToString) -> Self {
        Self::Security {
            kind: SecurityKind::Malware,
            message: format!("malware detected ({})", threat_name.to_string()),
        }
    }

    pub fn zip_bomb(ratio: f64) -> Self {
        Self::Security {
            kind: SecurityKind::ZipBomb,
            message: format!("ZIP bomb detected (compression ratio {ratio:.1})"),
        }
    }

    pub fn scanner_unavailable(error: impl ToString) -> Self {
        Self::Security {
            kind: SecurityKind::ScannerUnavailable,
            message: format!("scanner unavailable: {}", error.to_string()),
        }
    }

    pub fn upload_tampered(message: impl ToString) -> Self {
        Self::Security {
            kind: SecurityKind::UploadTampered,
            message: message.to_string(),
        }
    }

    pub fn index(error: impl ToString) -> Self {
        Self::Index {
            kind: IndexKind::Other,
            message: error.to_string(),
        }
    }

    pub fn embedding(error: impl ToString) -> Self {
        Self::Embedding {
            kind: IndexKind::Embedding,
            message: error.to_string(),
        }
    }

    pub fn internal(error: impl ToString) -> Self {
        Self::Internal(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_distinguishes_security_kinds() {
        assert_eq!(IngestionError::malware("eicar").class(), "security_malware");
        assert_eq!(IngestionError::zip_bomb(99.0).class(), "security_zip_bomb");
        assert_eq!(
            IngestionError::storage_database("pg down").class(),
            "storage_database"
        );
        assert_eq!(IngestionError::Timeout(30).class(), "timeout");
        assert_eq!(
            IngestionError::document_locked("busy").class(),
            "document_locked"
        );
    }
}
