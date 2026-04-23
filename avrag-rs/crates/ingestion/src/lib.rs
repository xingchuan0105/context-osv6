mod error;
mod model;
mod runtime;

pub mod chunker;
pub mod ir;
pub mod ir_validator;
pub mod parser;

pub use error::IngestionError;
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
    AuditAction, AuditRecord, DocumentStateMachine, IngestDocumentPayload, IngestionTask,
    IngestionTaskKind, IngestionTaskPayload, ReindexDocumentPayload, ReindexReason, Transition,
    build_ingest_task, build_reindex_task, task_audit,
};
pub use runtime::{
    AuditSink, NoopAuditSink, NoopStateSink, NoopTaskProcessor, NoopTaskSource, StateSink,
    TaskProcessor, TaskSource, WorkerRuntime, WorkerTick,
};

#[cfg(test)]
mod tests_impl;
