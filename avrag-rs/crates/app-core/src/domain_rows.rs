//! Domain row types shared across ports (mapped from storage adapters).

pub use avrag_storage_pg::{
    DocumentAssetRow, DocumentDeletionOutcome, DocumentScopeState, DocumentTaskSeed,
    DocumentUploadMutationOutcome, DocumentUploadQueueOutcome, IndexedChunk, MultimodalChunkRow,
    NotificationCreateParams, TaggedMessage, UserProfileRow,
};
