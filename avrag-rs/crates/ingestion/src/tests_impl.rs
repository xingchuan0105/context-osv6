use super::*;
use async_trait::async_trait;

struct SingleTaskSource(Option<IngestionTask>);

#[async_trait]
impl TaskSource for SingleTaskSource {
    async fn fetch_next(&mut self) -> Result<Option<IngestionTask>, IngestionError> {
        Ok(self.0.take())
    }

    async fn complete(&mut self, _task: &IngestionTask) -> Result<(), IngestionError> {
        Ok(())
    }

    async fn fail(&mut self, _task: &IngestionTask, _error: &str) -> Result<(), IngestionError> {
        Ok(())
    }
}

#[derive(Default)]
struct CapturingAuditSink {
    records: Vec<AuditRecord>,
}

#[async_trait]
impl AuditSink for CapturingAuditSink {
    async fn record(&mut self, record: AuditRecord) -> Result<(), IngestionError> {
        self.records.push(record);
        Ok(())
    }
}

#[derive(Default)]
struct CapturingStateSink {
    transitions: Vec<Transition>,
}

#[async_trait]
impl StateSink for CapturingStateSink {
    async fn transition(
        &mut self,
        _task: &IngestionTask,
        transition: Transition,
    ) -> Result<(), IngestionError> {
        DocumentStateMachine::validate(&transition)?;
        self.transitions.push(transition);
        Ok(())
    }
}

#[derive(Default)]
struct CapturingProcessor;

#[async_trait]
impl TaskProcessor for CapturingProcessor {
    async fn process(&mut self, _task: &IngestionTask) -> Result<(), IngestionError> {
        Ok(())
    }
}

#[test]
fn validates_state_machine_rules() {
    assert!(
        DocumentStateMachine::validate(&Transition {
            from: common::DocumentStatus::Queued,
            to: common::DocumentStatus::Processing,
        })
        .is_ok()
    );

    assert!(
        DocumentStateMachine::validate(&Transition {
            from: common::DocumentStatus::Pending,
            to: common::DocumentStatus::Completed,
        })
        .is_err()
    );
}

#[test]
fn builds_reindex_task_with_deterministic_idempotency_key() {
    let task = build_reindex_task(
        "org-1",
        "notebook-1",
        "doc-1",
        Some("user-1".to_string()),
        ReindexDocumentPayload {
            reason: ReindexReason::Manual,
            requested_revision: 7,
        },
    );

    assert_eq!(task.kind, IngestionTaskKind::ReindexDocument);
    assert_eq!(task.idempotency_key, "org-1:doc-1:reindex:7");
}

#[tokio::test]
async fn worker_runtime_processes_ingest_task() {
    let task = build_ingest_task(
        "org-1",
        "notebook-1",
        "doc-1",
        Some("user-1".to_string()),
        IngestDocumentPayload {
            source_uri: "s3://bucket/org/notebook/doc/file.pdf".to_string(),
            object_path: "org/notebook/doc/file.pdf".to_string(),
            mime_type: "application/pdf".to_string(),
            filename: "file.pdf".to_string(),
            file_size: 128,
        },
    );

    let mut worker = WorkerRuntime::new(
        SingleTaskSource(Some(task.clone())),
        CapturingAuditSink::default(),
        CapturingStateSink::default(),
        CapturingProcessor,
    );

    let tick = worker.run_once().await.expect("worker should process task");
    assert_eq!(tick, WorkerTick::Processed(task));
}
