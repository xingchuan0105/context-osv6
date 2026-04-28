use super::*;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

struct SingleTaskSource {
    task: Option<IngestionTask>,
    fail_outcome: TaskFailureOutcome,
    complete_outcome: TaskCompletionOutcome,
}

impl SingleTaskSource {
    fn new(task: IngestionTask) -> Self {
        Self {
            task: Some(task),
            fail_outcome: TaskFailureOutcome::Requeued,
            complete_outcome: TaskCompletionOutcome::Completed,
        }
    }

    fn with_failure_outcome(mut self, outcome: TaskFailureOutcome) -> Self {
        self.fail_outcome = outcome;
        self
    }
}

#[async_trait]
impl TaskSource for SingleTaskSource {
    async fn fetch_next(&mut self) -> Result<Option<IngestionTask>, IngestionError> {
        Ok(self.task.take())
    }

    async fn complete(
        &mut self,
        _task: &IngestionTask,
    ) -> Result<TaskCompletionOutcome, IngestionError> {
        Ok(self.complete_outcome)
    }

    async fn fail(
        &mut self,
        _task: &IngestionTask,
        _error: &str,
    ) -> Result<TaskFailureOutcome, IngestionError> {
        Ok(self.fail_outcome)
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

#[derive(Default)]
struct FailingProcessor;

#[async_trait]
impl TaskProcessor for FailingProcessor {
    async fn process(&mut self, _task: &IngestionTask) -> Result<(), IngestionError> {
        Err(IngestionError::StateSink("boom".to_string()))
    }
}

#[derive(Clone, Default)]
struct SharedStateSink {
    transitions: Arc<Mutex<Vec<Transition>>>,
}

#[async_trait]
impl StateSink for SharedStateSink {
    async fn transition(
        &mut self,
        _task: &IngestionTask,
        transition: Transition,
    ) -> Result<(), IngestionError> {
        DocumentStateMachine::validate(&transition)?;
        self.transitions.lock().unwrap().push(transition);
        Ok(())
    }
}

type SharedRuntimeEvents = Arc<Mutex<Vec<&'static str>>>;

struct OrderingTaskSource {
    task: Option<IngestionTask>,
    events: SharedRuntimeEvents,
}

#[async_trait]
impl TaskSource for OrderingTaskSource {
    async fn fetch_next(&mut self) -> Result<Option<IngestionTask>, IngestionError> {
        Ok(self.task.take())
    }

    async fn complete(
        &mut self,
        _task: &IngestionTask,
    ) -> Result<TaskCompletionOutcome, IngestionError> {
        self.events.lock().unwrap().push("task_source.complete");
        Ok(TaskCompletionOutcome::Completed)
    }

    async fn fail(
        &mut self,
        _task: &IngestionTask,
        _error: &str,
    ) -> Result<TaskFailureOutcome, IngestionError> {
        self.events.lock().unwrap().push("task_source.fail");
        Ok(TaskFailureOutcome::Requeued)
    }
}

struct OrderingAuditSink {
    events: SharedRuntimeEvents,
}

#[async_trait]
impl AuditSink for OrderingAuditSink {
    async fn record(&mut self, record: AuditRecord) -> Result<(), IngestionError> {
        match record.action {
            AuditAction::TaskStarted => self.events.lock().unwrap().push("audit.started"),
            AuditAction::TaskCompleted => self.events.lock().unwrap().push("audit.completed"),
            AuditAction::TaskFailed => self.events.lock().unwrap().push("audit.failed"),
            _ => {}
        }
        Ok(())
    }
}

struct OrderingStateSink {
    events: SharedRuntimeEvents,
}

#[async_trait]
impl StateSink for OrderingStateSink {
    async fn transition(
        &mut self,
        _task: &IngestionTask,
        transition: Transition,
    ) -> Result<(), IngestionError> {
        DocumentStateMachine::validate(&transition)?;
        match transition.to {
            common::DocumentStatus::Processing => {
                self.events.lock().unwrap().push("state.processing")
            }
            common::DocumentStatus::Completed => {
                self.events.lock().unwrap().push("state.completed")
            }
            _ => {}
        }
        Ok(())
    }
}

struct OrderingProcessor {
    events: SharedRuntimeEvents,
}

#[async_trait]
impl TaskProcessor for OrderingProcessor {
    async fn process(&mut self, _task: &IngestionTask) -> Result<(), IngestionError> {
        self.events.lock().unwrap().push("processor.process");
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
            from: common::DocumentStatus::Processing,
            to: common::DocumentStatus::Queued,
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
fn ingest_task_defaults_retry_fields() {
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

    assert_eq!(task.lock_token, None);
    assert_eq!(task.attempt_count, 0);
    assert_eq!(task.max_attempts, DEFAULT_MAX_ATTEMPTS);
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
        SingleTaskSource::new(task.clone()),
        CapturingAuditSink::default(),
        CapturingStateSink::default(),
        CapturingProcessor,
    );

    let tick = worker.run_once().await.expect("worker should process task");
    assert_eq!(tick, WorkerTick::Processed(task));
}

#[tokio::test]
async fn worker_runtime_requeues_retryable_failures_without_marking_failed() {
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
    let state = SharedStateSink::default();
    let transitions = state.transitions.clone();

    let mut worker = WorkerRuntime::new(
        SingleTaskSource::new(task).with_failure_outcome(TaskFailureOutcome::Requeued),
        CapturingAuditSink::default(),
        state,
        FailingProcessor,
    );

    assert!(worker.run_once().await.is_err());
    let transitions = transitions.lock().unwrap();
    assert!(transitions.iter().any(|transition| {
        transition.from == common::DocumentStatus::Processing
            && transition.to == common::DocumentStatus::Queued
    }));
    assert!(
        !transitions
            .iter()
            .any(|transition| transition.to == common::DocumentStatus::Failed)
    );
}

#[tokio::test]
async fn worker_runtime_records_success_state_and_audit_before_task_completion() {
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
    let events: SharedRuntimeEvents = Arc::new(Mutex::new(Vec::new()));

    let mut worker = WorkerRuntime::new(
        OrderingTaskSource {
            task: Some(task),
            events: events.clone(),
        },
        OrderingAuditSink {
            events: events.clone(),
        },
        OrderingStateSink {
            events: events.clone(),
        },
        OrderingProcessor {
            events: events.clone(),
        },
    );

    worker.run_once().await.expect("worker should process task");
    let events = events.lock().unwrap().clone();
    let state_completed = events
        .iter()
        .position(|event| *event == "state.completed")
        .expect("completed state transition should be recorded");
    let audit_completed = events
        .iter()
        .position(|event| *event == "audit.completed")
        .expect("completed audit should be recorded");
    let task_complete = events
        .iter()
        .position(|event| *event == "task_source.complete")
        .expect("task source complete should be called");

    assert!(
        state_completed < audit_completed,
        "state should complete before audit"
    );
    assert!(
        audit_completed < task_complete,
        "audit should record before task row completion"
    );
}
