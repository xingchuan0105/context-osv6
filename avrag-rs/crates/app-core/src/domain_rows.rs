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
    pub org_id: String,
    pub workspace_id: String,
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

#[derive(Debug, Clone)]
pub struct DocumentScopeState {
    pub document_id: Uuid,
    pub status: DocumentStatus,
}
