use anyhow::Result;
use avrag_storage_pg::{NotificationCreateParams, PgAppRepository};
use ingestion::{
    AuditRecord, AuditSink, IngestionError, IngestionTask, StateSink, TaskCompletionOutcome,
    TaskFailureOutcome, TaskSource, Transition,
};
use tracing::info;
use uuid::Uuid;

use crate::ingestion_guard::{ensure_ingestion_side_effects_allowed, from_storage_error};
use crate::runtime_support::task_context;

pub(crate) struct PgTaskSource {
    pub(crate) repo: PgAppRepository,
    pub(crate) worker_id: String,
    pub(crate) worker_queue_group: String,
}

#[async_trait::async_trait]
impl TaskSource for PgTaskSource {
    async fn fetch_next(&mut self) -> Result<Option<IngestionTask>, IngestionError> {
        self.repo
            .ingestion_queue()
            .claim_next_ingestion_task(&self.worker_id, &self.worker_queue_group)
            .await
            .map_err(|error| IngestionError::TaskSource(error.to_string()))
    }

    async fn complete(
        &mut self,
        task: &IngestionTask,
    ) -> Result<TaskCompletionOutcome, IngestionError> {
        self.repo
            .ingestion_queue()
            .complete_ingestion_task(&task.task_id, task.lock_token.as_deref())
            .await
            .map_err(|error| IngestionError::TaskSource(error.to_string()))
    }

    async fn fail(
        &mut self,
        task: &IngestionTask,
        error: &str,
    ) -> Result<TaskFailureOutcome, IngestionError> {
        self.repo
            .ingestion_queue()
            .fail_ingestion_task(&task.task_id, task.lock_token.as_deref(), error)
            .await
            .map_err(|err| IngestionError::TaskSource(err.to_string()))
    }
}

pub(crate) struct PgAuditSink {
    pub(crate) repo: PgAppRepository,
}

#[async_trait::async_trait]
impl AuditSink for PgAuditSink {
    async fn record(&mut self, record: AuditRecord) -> Result<(), IngestionError> {
        self.repo
            .audit()
            .append_audit_record(&record)
            .await
            .map_err(|error| IngestionError::AuditSink(error.to_string()))
    }
}

pub(crate) struct PgStateSink {
    pub(crate) repo: PgAppRepository,
}

#[async_trait::async_trait]
impl StateSink for PgStateSink {
    async fn transition(
        &mut self,
        task: &IngestionTask,
        transition: Transition,
    ) -> Result<(), IngestionError> {
        ingestion::DocumentStateMachine::validate(&transition)?;
        let context = task_context(task);
        let document_id = Uuid::parse_str(&task.document_id)
            .map_err(IngestionError::from)?;

        if matches!(
            transition.to,
            contracts::documents::DocumentStatus::Processing
                | contracts::documents::DocumentStatus::Completed
        ) {
            ensure_ingestion_side_effects_allowed(
                &self.repo,
                &context,
                task,
                document_id,
                "document status transition",
            )
            .await?;
        }

        let updated = if matches!(
            transition.to,
            contracts::documents::DocumentStatus::Processing
                | contracts::documents::DocumentStatus::Completed
        ) {
            self.repo
                .documents()
                .set_document_status_for_ingestion_task(
                    &context,
                    document_id,
                    transition.to.clone(),
                    &task.task_id,
                    task.lock_token.as_deref(),
                )
                .await
                .map_err(from_storage_error)?
        } else {
            self.repo
                .documents()
                .set_document_status(&context, document_id, transition.to.clone())
                .await
                .map_err(from_storage_error)?
        };
        if !updated {
            return Err(IngestionError::StateSink(format!(
                "document status transition to {:?} rejected: ingestion task lease lost or document is deleting",
                transition.to
            )));
        }

        self.repo
            .audit()
            .append_audit_record(&AuditRecord {
                audit_id: common::new_id(),
                org_id: task.org_id.clone(),
                actor_id: task.requested_by.clone(),
                action: ingestion::AuditAction::StateTransition,
                resource_type: "document".to_string(),
                resource_id: task.document_id.clone(),
                payload: serde_json::json!({
                    "from": transition.from,
                    "to": transition.to,
                    "task_id": task.task_id,
                }),
                created_at: chrono::Utc::now().to_rfc3339(),
            })
            .await
            .map_err(from_storage_error)?;

        if matches!(
            transition.to,
            contracts::documents::DocumentStatus::Completed
                | contracts::documents::DocumentStatus::Failed
        ) {
            if matches!(transition.to, contracts::documents::DocumentStatus::Failed)
                && let Some(user_id) = task
                    .requested_by
                    .as_deref()
                    .and_then(|value| Uuid::parse_str(value).ok())
            {
                let analytics = analytics::AnalyticsService::new(self.repo.raw().clone());
                let event = analytics::ProductEvent {
                    event_id: Uuid::new_v4(),
                    event_time: chrono::Utc::now(),
                    user_id,
                    session_id: None,
                    notebook_id: Uuid::parse_str(&task.notebook_id).ok(),
                    surface: analytics::Surface::Workspace,
                    event_name: analytics::ProductEventName::DocumentUploadFailed,
                    result: analytics::ResultTag::Failure,
                    request_id: None,
                    trace_id: None,
                    client_platform: "worker".to_string(),
                    metadata: serde_json::json!({
                        "document_id": task.document_id.clone(),
                        "task_id": task.task_id.clone(),
                    }),
                };
                if let Err(error) = analytics.record_product_event(&event).await {
                    info!(error = %error, document_id = task.document_id, "failed to record document upload failure event");
                }
            }
            if let Some(user_id) = task
                .requested_by
                .as_deref()
                .and_then(|value| Uuid::parse_str(value).ok())
            {
                let title = if matches!(
                    transition.to,
                    contracts::documents::DocumentStatus::Completed
                ) {
                    "Document ingestion completed"
                } else {
                    "Document ingestion failed"
                };
                let body = if matches!(
                    transition.to,
                    contracts::documents::DocumentStatus::Completed
                ) {
                    "A document finished ingestion and is ready for retrieval."
                } else {
                    "A document failed ingestion and needs attention."
                };
                let _ = self
                    .repo
                    .auth()
                    .create_notification(
                        &context,
                        NotificationCreateParams {
                            user_id,
                            event_type: if matches!(
                                transition.to,
                                contracts::documents::DocumentStatus::Completed
                            ) {
                                "ingestion.success".to_string()
                            } else {
                                "ingestion.failed".to_string()
                            },
                            title: title.to_string(),
                            body: body.to_string(),
                            data: serde_json::json!({
                                "document_id": task.document_id.clone(),
                                "task_id": task.task_id.clone(),
                                "status": format!("{:?}", transition.to),
                            }),
                            channels: vec!["in_app".to_string()],
                        },
                    )
                    .await;
            }
        }

        Ok(())
    }
}
