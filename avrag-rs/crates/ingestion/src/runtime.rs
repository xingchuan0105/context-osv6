use async_trait::async_trait;
use serde_json::json;
use tracing::{info, warn};

pub use ingestion_types::{TaskCompletionOutcome, TaskFailureOutcome};

use crate::model::{AuditAction, AuditRecord, DocumentStateMachine, IngestionTask, Transition};
use crate::{IngestionError, model};

#[async_trait]
pub trait TaskSource {
    async fn fetch_next(&mut self) -> Result<Option<IngestionTask>, IngestionError>;
    async fn complete(
        &mut self,
        task: &IngestionTask,
    ) -> Result<TaskCompletionOutcome, IngestionError>;
    async fn fail(
        &mut self,
        task: &IngestionTask,
        error: &str,
    ) -> Result<TaskFailureOutcome, IngestionError>;
}

#[async_trait]
pub trait AuditSink {
    async fn record(&mut self, record: AuditRecord) -> Result<(), IngestionError>;
}

#[async_trait]
pub trait StateSink {
    async fn transition(
        &mut self,
        task: &IngestionTask,
        transition: Transition,
    ) -> Result<(), IngestionError>;
}

#[async_trait]
pub trait TaskProcessor {
    async fn process(&mut self, task: &IngestionTask) -> Result<(), IngestionError>;
}

#[derive(Default)]
pub struct NoopTaskSource;

#[async_trait]
impl TaskSource for NoopTaskSource {
    async fn fetch_next(&mut self) -> Result<Option<IngestionTask>, IngestionError> {
        Ok(None)
    }

    async fn complete(
        &mut self,
        _task: &IngestionTask,
    ) -> Result<TaskCompletionOutcome, IngestionError> {
        Ok(TaskCompletionOutcome::Completed)
    }

    async fn fail(
        &mut self,
        _task: &IngestionTask,
        _error: &str,
    ) -> Result<TaskFailureOutcome, IngestionError> {
        Ok(TaskFailureOutcome::Requeued)
    }
}

#[derive(Default)]
pub struct NoopAuditSink;

#[async_trait]
impl AuditSink for NoopAuditSink {
    async fn record(&mut self, _record: AuditRecord) -> Result<(), IngestionError> {
        Ok(())
    }
}

#[derive(Default)]
pub struct NoopStateSink;

#[async_trait]
impl StateSink for NoopStateSink {
    async fn transition(
        &mut self,
        _task: &IngestionTask,
        transition: Transition,
    ) -> Result<(), IngestionError> {
        DocumentStateMachine::validate(&transition)
    }
}

#[derive(Default)]
pub struct NoopTaskProcessor;

#[async_trait]
impl TaskProcessor for NoopTaskProcessor {
    async fn process(&mut self, _task: &IngestionTask) -> Result<(), IngestionError> {
        Ok(())
    }
}

pub struct WorkerRuntime<TTaskSource, TAuditSink, TStateSink, TProcessor> {
    task_source: TTaskSource,
    audit_sink: TAuditSink,
    state_sink: TStateSink,
    processor: TProcessor,
}

impl<TTaskSource, TAuditSink, TStateSink, TProcessor>
    WorkerRuntime<TTaskSource, TAuditSink, TStateSink, TProcessor>
where
    TTaskSource: TaskSource,
    TAuditSink: AuditSink,
    TStateSink: StateSink,
    TProcessor: TaskProcessor,
{
    pub fn new(
        task_source: TTaskSource,
        audit_sink: TAuditSink,
        state_sink: TStateSink,
        processor: TProcessor,
    ) -> Self {
        Self {
            task_source,
            audit_sink,
            state_sink,
            processor,
        }
    }

    pub async fn run_once(&mut self) -> Result<WorkerTick, IngestionError> {
        let Some(task) = self.task_source.fetch_next().await? else {
            return Ok(WorkerTick::Idle);
        };

        self.audit_sink
            .record(model::task_audit(
                &task,
                AuditAction::TaskStarted,
                json!({ "kind": task.kind }),
            ))
            .await?;

        let task_result: Result<(), IngestionError> = async {
            self.state_sink
                .transition(
                    &task,
                    Transition {
                        from: contracts::documents::DocumentStatus::Queued,
                        to: contracts::documents::DocumentStatus::Processing,
                    },
                )
                .await?;
            self.processor.process(&task).await?;
            Ok(())
        }
        .await;

        if let Err(error) = task_result {
            let error_message = error.to_string();
            let failure_outcome = self.task_source.fail(&task, &error_message).await?;
            match failure_outcome {
                TaskFailureOutcome::DeadLettered => {
                    if let Err(transition_error) = self
                        .state_sink
                        .transition(
                            &task,
                            Transition {
                                from: contracts::documents::DocumentStatus::Processing,
                                to: contracts::documents::DocumentStatus::Failed,
                            },
                        )
                        .await
                    {
                        warn!(
                            task_id = task.task_id,
                            document_id = task.document_id,
                            outcome = ?failure_outcome,
                            error = %transition_error,
                            processing_error = %error_message,
                            "failed to transition dead-lettered ingestion task document to failed"
                        );
                    }
                }
                TaskFailureOutcome::Requeued => {
                    if let Err(transition_error) = self
                        .state_sink
                        .transition(
                            &task,
                            Transition {
                                from: contracts::documents::DocumentStatus::Processing,
                                to: contracts::documents::DocumentStatus::Queued,
                            },
                        )
                        .await
                    {
                        warn!(
                            task_id = task.task_id,
                            document_id = task.document_id,
                            outcome = ?failure_outcome,
                            error = %transition_error,
                            processing_error = %error_message,
                            "failed to transition retryable ingestion task document back to queued"
                        );
                    }
                }
                TaskFailureOutcome::LeaseLost => {
                    warn!(
                        task_id = task.task_id,
                        document_id = task.document_id,
                        outcome = ?failure_outcome,
                        error = %error_message,
                        "ingestion task failure lease lost; leaving document state unchanged"
                    );
                }
            }
            self.audit_sink
                .record(model::task_audit(
                    &task,
                    AuditAction::TaskFailed,
                    json!({ "kind": task.kind, "error": error_message, "outcome": failure_outcome }),
                ))
                .await?;
            return Err(error);
        }

        self.state_sink
            .transition(
                &task,
                Transition {
                    from: contracts::documents::DocumentStatus::Processing,
                    to: contracts::documents::DocumentStatus::Completed,
                },
            )
            .await?;
        self.audit_sink
            .record(model::task_audit(
                &task,
                AuditAction::TaskCompleted,
                json!({ "kind": task.kind, "outcome": TaskCompletionOutcome::Completed }),
            ))
            .await?;
        let completion_outcome = self.task_source.complete(&task).await?;
        if matches!(completion_outcome, TaskCompletionOutcome::LeaseLost) {
            warn!(
                task_id = task.task_id,
                document_id = task.document_id,
                ?completion_outcome,
                "ingestion task completed document side effects but lost task lease before task row cleanup"
            );
        }
        info!(
            task_id = task.task_id,
            document_id = task.document_id,
            ?completion_outcome,
            "worker task processed"
        );
        Ok(WorkerTick::Processed(Box::new(task)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerTick {
    Idle,
    Processed(Box<IngestionTask>),
}
