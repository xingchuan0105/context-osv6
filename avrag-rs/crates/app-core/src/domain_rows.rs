//! Domain row types shared across ports (mapped from storage adapters).

use contracts::documents::DocumentStatus;
use uuid::Uuid;

pub use avrag_rag_core_ports::{
    ConversationHistoryHit, ConversationHistoryScope, DocumentAssetRow, MultimodalChunkRow,
    NotificationCreateParams, UserProfileRow,
};
pub use common::IndexedChunk;

#[derive(Debug, Clone)]
pub struct DocumentTaskSeed {
    pub document_id: String,
    pub owner_user_id: String,
    /// Workspace binding of the artifact when one exists (lineage); session
    /// artifacts are `None`. Scope truth is the typed binding tables.
    pub workspace_id: Option<String>,
    pub filename: String,
    pub mime_type: String,
    pub file_size: u64,
    pub object_path: String,
    pub status: DocumentStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentUploadMutationOutcome {
    Updated,
    NotFound,
    StatusConflict(DocumentStatus),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentUploadQueueOutcome {
    Queued { task_inserted: bool },
    NotFound,
    StatusConflict(DocumentStatus),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentDeletionOutcome {
    Queued { task_inserted: bool },
    AlreadyDeleting { task_inserted: bool },
    AlreadyDeleted,
    NotFound,
}

/// Workspace binding version facts (chat-first W2d snapshot §4.4).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceBindingVersion {
    pub binding_id: String,
    pub artifact_id: String,
    pub parse_version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DocumentScopeState {
    pub document_id: Uuid,
    pub status: DocumentStatus,
}
