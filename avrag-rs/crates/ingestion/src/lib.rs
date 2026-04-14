mod error;
mod model;
mod runtime;

pub mod chunker;
pub mod parser;

pub use error::IngestionError;
pub use model::{
    build_ingest_task, build_reindex_task, task_audit, AuditAction, AuditRecord, DocumentStateMachine,
    IngestDocumentPayload, IngestionTask, IngestionTaskKind, IngestionTaskPayload, ReindexDocumentPayload,
    ReindexReason, Transition,
};
pub use runtime::{
    AuditSink, NoopAuditSink, NoopStateSink, NoopTaskProcessor, NoopTaskSource, StateSink,
    TaskProcessor, TaskSource, WorkerRuntime, WorkerTick,
};

#[cfg(test)]
mod tests_impl;
