mod error;
mod model;
mod runtime;

pub mod chunker;
pub mod ir;
pub mod ir_validator;
pub mod parser;
pub mod security_scanner;
pub mod semantic;

pub use error::{
    IngestionError, IndexKind, ParseKind, SecurityKind, StorageKind,
};
pub use ingestion_types::{
    AuditAction, AuditRecord, DEFAULT_MAX_ATTEMPTS, IngestDocumentPayload, IngestUrlPayload,
    IngestionTask, IngestionTaskKind, IngestionTaskPayload, ReindexDocumentPayload, ReindexReason,
    TaskCompletionOutcome, TaskFailureOutcome,
};
pub use ir::{
    AssetIr, AssetKind, BlockIr, BlockModality, BlockType, DocumentIr, DocumentType, PageIr,
    ParseBackend, ParseWarning, SourceLocator,
};
pub use ir_validator::{
    DocumentIrValidationError, DocumentIrValidationIssue, DocumentIrValidationOptions,
    DocumentIrValidationReport, sanitize_and_validate_document_ir, sanitize_document_ir,
    validate_document_ir,
};
pub use model::{
    DocumentStateMachine, Transition, build_ingest_task, build_ingest_url_task, build_reindex_task,
    task_audit,
};
pub use parser::{
    LiteParseConfig, LiteParseService, PageParseStatus, PageStatusEntry, ParsedPdfSnapshot,
    blocks_to_document_ir, parse_page_status_from_ir,
};
pub use runtime::{
    AuditSink, NoopAuditSink, NoopStateSink, NoopTaskProcessor, NoopTaskSource, StateSink,
    TaskProcessor, TaskSource, WorkerRuntime, WorkerTick,
};

#[cfg(test)]
mod tests_impl;
