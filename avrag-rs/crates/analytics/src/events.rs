use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    Dashboard,
    Workspace,
    Search,
    SharedKb,
    Settings,
    Api,
    Worker,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResultTag {
    Success,
    Failure,
    Cancelled,
    Degraded,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProductEventName {
    UserRegistered,
    UserLoggedIn,
    PasswordResetRequested,
    PasswordResetVerified,
    PasswordResetCompleted,
    NotebookCreated,
    NotebookOpened,
    SessionCreated,
    SessionRenamed,
    SessionPinned,
    SessionDeleted,
    DocumentUploadStarted,
    DocumentUploadCompleted,
    DocumentUploadFailed,
    UrlSourceAdded,
    DocumentReindexed,
    ChatStarted,
    ChatCompleted,
    ChatFailed,
    SearchStarted,
    SearchCompleted,
    SearchFailed,
    SharedKbOpened,
    SharedKbChatStarted,
    SharedKbChatCompleted,
    CitationOpened,
    SourceFocused,
    NoteEdited,
    NoteSynced,
    ShareLinkCreated,
    ShareLinkDisabled,
    MessageFeedback,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CostEventName {
    LlmUsageMetered,
    EmbeddingUsageMetered,
    SummaryUsageMetered,
    UploadBytesMetered,
    StorageSnapshotRecorded,
    ExternalSearchUsageMetered,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductEvent {
    pub event_id: Uuid,
    pub event_time: DateTime<Utc>,
    pub user_id: Uuid,
    pub session_id: Option<Uuid>,
    pub notebook_id: Option<Uuid>,
    pub surface: Surface,
    pub event_name: ProductEventName,
    pub result: ResultTag,
    pub request_id: Option<String>,
    pub trace_id: Option<String>,
    pub client_platform: String,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEvent {
    pub event_id: Uuid,
    pub event_time: DateTime<Utc>,
    pub user_id: Uuid,
    pub session_id: Option<Uuid>,
    pub notebook_id: Option<Uuid>,
    pub event_name: CostEventName,
    pub feature: String,
    pub provider: String,
    pub model: String,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub embedding_tokens: i64,
    pub usage_units: i64,
    pub storage_bytes_delta: i64,
    pub external_call_count: i64,
    pub source: String,
    pub metadata: serde_json::Value,
}
