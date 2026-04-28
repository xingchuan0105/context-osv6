use chrono::Utc;
use common::{DocumentStatus, new_id};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::IngestionError;

pub const DEFAULT_MAX_ATTEMPTS: i32 = 5;

fn default_max_attempts() -> i32 {
    DEFAULT_MAX_ATTEMPTS
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestionTaskKind {
    IngestDocument,
    ReindexDocument,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestionTask {
    pub task_id: String,
    pub kind: IngestionTaskKind,
    pub org_id: String,
    pub notebook_id: String,
    pub document_id: String,
    pub requested_by: Option<String>,
    pub idempotency_key: String,
    pub enqueued_at: String,
    pub payload: IngestionTaskPayload,
    #[serde(default)]
    pub lock_token: Option<String>,
    #[serde(default)]
    pub attempt_count: i32,
    #[serde(default = "default_max_attempts")]
    pub max_attempts: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IngestionTaskPayload {
    IngestDocument(IngestDocumentPayload),
    ReindexDocument(ReindexDocumentPayload),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestDocumentPayload {
    pub source_uri: String,
    pub object_path: String,
    pub mime_type: String,
    pub filename: String,
    pub file_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReindexDocumentPayload {
    pub reason: ReindexReason,
    pub requested_revision: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReindexReason {
    Manual,
    ParserUpgrade,
    EmbeddingUpgrade,
    DriftDetected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    TaskEnqueued,
    TaskStarted,
    TaskCompleted,
    TaskFailed,
    StateTransition,
    // Guardrail actions
    InputGuardBlock,
    OutputGuardBlock,
    OutputGuardRedact,
    OutputGuardFlag,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
    pub audit_id: String,
    pub org_id: String,
    pub actor_id: Option<String>,
    pub action: AuditAction,
    pub resource_type: String,
    pub resource_id: String,
    pub payload: Value,
    pub created_at: String,
}

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
    notebook_id: impl Into<String>,
    document_id: impl Into<String>,
    requested_by: Option<String>,
    payload: IngestDocumentPayload,
) -> IngestionTask {
    let org_id = org_id.into();
    let notebook_id = notebook_id.into();
    let document_id = document_id.into();
    let idempotency_key = format!("{}:{}:{}", org_id, document_id, payload.object_path);

    IngestionTask {
        task_id: new_id(),
        kind: IngestionTaskKind::IngestDocument,
        org_id,
        notebook_id,
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

pub fn build_reindex_task(
    org_id: impl Into<String>,
    notebook_id: impl Into<String>,
    document_id: impl Into<String>,
    requested_by: Option<String>,
    payload: ReindexDocumentPayload,
) -> IngestionTask {
    let org_id = org_id.into();
    let notebook_id = notebook_id.into();
    let document_id = document_id.into();
    let idempotency_key = format!(
        "{}:{}:reindex:{}",
        org_id, document_id, payload.requested_revision
    );

    IngestionTask {
        task_id: new_id(),
        kind: IngestionTaskKind::ReindexDocument,
        org_id,
        notebook_id,
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
