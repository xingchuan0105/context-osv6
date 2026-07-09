use chrono::Utc;
use common::new_id;
use contracts::documents::DocumentStatus;
use serde_json::Value;

use crate::IngestionError;

pub use ingestion_types::{
    AuditAction, AuditRecord, DEFAULT_MAX_ATTEMPTS, IngestDocumentPayload, IngestUrlPayload,
    IngestionTask, IngestionTaskKind, IngestionTaskPayload, ReindexDocumentPayload,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition {
    pub from: DocumentStatus,
    pub to: DocumentStatus,
}

pub struct DocumentStateMachine;

impl DocumentStateMachine {
    pub fn validate(transition: &Transition) -> Result<(), IngestionError> {
        let allowed = matches!(
            (&transition.from, &transition.to),
            (DocumentStatus::Pending, DocumentStatus::Enqueueing)
                | (DocumentStatus::Pending, DocumentStatus::Queued)
                | (DocumentStatus::Enqueueing, DocumentStatus::Queued)
                | (DocumentStatus::Queued, DocumentStatus::Processing)
                | (DocumentStatus::Processing, DocumentStatus::Queued)
                | (DocumentStatus::Processing, DocumentStatus::Completed)
                | (DocumentStatus::Processing, DocumentStatus::Failed)
                | (DocumentStatus::Failed, DocumentStatus::Queued)
                | (DocumentStatus::Completed, DocumentStatus::Queued)
        );

        if allowed {
            return Ok(());
        }

        Err(IngestionError::InvalidStateTransition {
            from: transition.from.clone(),
            to: transition.to.clone(),
        })
    }

    pub fn ingest_lifecycle() -> &'static [DocumentStatus] {
        const STATES: &[DocumentStatus] = &[
            DocumentStatus::Pending,
            DocumentStatus::Enqueueing,
            DocumentStatus::Queued,
            DocumentStatus::Processing,
            DocumentStatus::Completed,
        ];
        STATES
    }
}

pub fn build_ingest_task(
    org_id: impl Into<String>,
    workspace_id: impl Into<String>,
    document_id: impl Into<String>,
    requested_by: Option<String>,
    payload: IngestDocumentPayload,
) -> IngestionTask {
    let org_id = org_id.into();
    let workspace_id = workspace_id.into();
    let document_id = document_id.into();
    let idempotency_key = format!("{}:{}:{}", org_id, document_id, payload.object_path);

    IngestionTask {
        task_id: new_id(),
        kind: IngestionTaskKind::IngestDocument,
        org_id,
        workspace_id,
        document_id,
        requested_by,
        idempotency_key,
        enqueued_at: Utc::now().to_rfc3339(),
        payload: IngestionTaskPayload::IngestDocument(payload),
        lock_token: None,
        attempt_count: 0,
        max_attempts: DEFAULT_MAX_ATTEMPTS,
    }
}

pub fn build_ingest_url_task(
    org_id: impl Into<String>,
    workspace_id: impl Into<String>,
    document_id: impl Into<String>,
    requested_by: Option<String>,
    payload: IngestUrlPayload,
) -> IngestionTask {
    let org_id = org_id.into();
    let workspace_id = workspace_id.into();
    let document_id = document_id.into();
    let idempotency_key = format!("{}:{}:{}", org_id, document_id, payload.url);

    IngestionTask {
        task_id: new_id(),
        kind: IngestionTaskKind::IngestUrl,
        org_id,
        workspace_id,
        document_id,
        requested_by,
        idempotency_key,
        enqueued_at: Utc::now().to_rfc3339(),
        payload: IngestionTaskPayload::IngestUrl(payload),
        lock_token: None,
        attempt_count: 0,
        max_attempts: DEFAULT_MAX_ATTEMPTS,
    }
}

pub fn build_reindex_task(
    org_id: impl Into<String>,
    workspace_id: impl Into<String>,
    document_id: impl Into<String>,
    requested_by: Option<String>,
    payload: ReindexDocumentPayload,
) -> IngestionTask {
    let org_id = org_id.into();
    let workspace_id = workspace_id.into();
    let document_id = document_id.into();
    let idempotency_key = format!(
        "{}:{}:reindex:{}",
        org_id, document_id, payload.requested_revision
    );

    IngestionTask {
        task_id: new_id(),
        kind: IngestionTaskKind::ReindexDocument,
        org_id,
        workspace_id,
        document_id,
        requested_by,
        idempotency_key,
        enqueued_at: Utc::now().to_rfc3339(),
        payload: IngestionTaskPayload::ReindexDocument(payload),
        lock_token: None,
        attempt_count: 0,
        max_attempts: DEFAULT_MAX_ATTEMPTS,
    }
}

pub fn task_audit(task: &IngestionTask, action: AuditAction, payload: Value) -> AuditRecord {
    AuditRecord {
        audit_id: new_id(),
        org_id: task.org_id.clone(),
        actor_id: task.requested_by.clone(),
        action,
        resource_type: "document_ingestion_task".to_string(),
        resource_id: task.task_id.clone(),
        payload,
        created_at: Utc::now().to_rfc3339(),
    }
}
