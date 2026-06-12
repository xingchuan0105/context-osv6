use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_MAX_ATTEMPTS: i32 = 5;

fn default_max_attempts() -> i32 {
    DEFAULT_MAX_ATTEMPTS
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestionTaskKind {
    IngestDocument,
    ReindexDocument,
    IngestUrl,
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
    IngestUrl(IngestUrlPayload),
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
pub struct IngestUrlPayload {
    pub url: String,
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
    InputGuardBlock,
    OutputGuardBlock,
    OutputGuardRedact,
    OutputGuardFlag,
    ChatRequest,
    SearchRequest,
    RagRequest,
    MessageFeedback,
    CitationClick,
    RoutingDecision,
    HighRiskToolCall,
    PolicyDeny,
    PolicyRequireApproval,
    BudgetExhausted,
    DegradeEvent,
    PermissionDenied,
}

impl AuditAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TaskEnqueued => "task_enqueued",
            Self::TaskStarted => "task_started",
            Self::TaskCompleted => "task_completed",
            Self::TaskFailed => "task_failed",
            Self::StateTransition => "state_transition",
            Self::InputGuardBlock => "input_guard_block",
            Self::OutputGuardBlock => "output_guard_block",
            Self::OutputGuardRedact => "output_guard_redact",
            Self::OutputGuardFlag => "output_guard_flag",
            Self::ChatRequest => "chat_request",
            Self::SearchRequest => "search_request",
            Self::RagRequest => "rag_request",
            Self::MessageFeedback => "message_feedback",
            Self::CitationClick => "citation_click",
            Self::RoutingDecision => "routing_decision",
            Self::HighRiskToolCall => "high_risk_tool_call",
            Self::PolicyDeny => "policy_deny",
            Self::PolicyRequireApproval => "policy_require_approval",
            Self::BudgetExhausted => "budget_exhausted",
            Self::DegradeEvent => "degrade_event",
            Self::PermissionDenied => "permission_denied",
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskFailureOutcome {
    Requeued,
    DeadLettered,
    LeaseLost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskCompletionOutcome {
    Completed,
    LeaseLost,
}
