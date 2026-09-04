use common::{Document, NotificationRow, ParsedPreviewItem};
use contracts::UserPreferences;
use contracts::chat::ChatMessage;
use contracts::documents::DocumentStatus;
use contracts::workspaces::{ChatSession, Workspace};
use std::collections::BTreeMap;

/// One zero-binding orphan judgment, THE single fact for every memory-adapter
/// deletion path (workspace delete ×2 in `MemoryDocumentStore`, the per-session
/// cascade in the same file, and `MemoryChatPersistence::delete_session`;
/// `MemoryDocumentStore::delete_document_if_unbound` shares this exact
/// transition after its own eligibility checks): mark the artifact `Deleting`
/// iff no workspace and no conversation binding remains, returning whether a
/// live artifact actually changed state (review round-7 S3 extracted the
/// duplicates; round-9 made the explicit-GC result consume this same fact).
pub(crate) fn mark_artifact_if_unbound(state: &mut MemoryState, artifact_id: &str) -> bool {
    let still_bound = state
        .workspace_document_bindings
        .iter()
        .any(|row| row.artifact_id == artifact_id)
        || state
            .conversation_document_bindings
            .iter()
            .any(|row| row.artifact_id == artifact_id);
    if still_bound {
        return false;
    }
    if let Some(stored) = state.documents.get_mut(artifact_id) {
        if matches!(
            stored.document.status,
            DocumentStatus::Deleting | DocumentStatus::Deleted
        ) {
            return false;
        }
        stored.document.status = DocumentStatus::Deleting;
        stored.document.updated_at = common::now_rfc3339();
        return true;
    }
    false
}

#[derive(Debug, Default)]
pub struct MemoryState {
    pub workspaces: BTreeMap<String, Workspace>,
    pub documents: BTreeMap<String, StoredDocument>,
    /// Typed binding truth: workspace binding rows (chat-first W2). Each row
    /// carries its own binding id + parse version, mirroring the PG
    /// `workspace_document_bindings` contract (review round-4: the memory
    /// adapter must exercise the production contract, not fabricate ids).
    pub workspace_document_bindings: Vec<WorkspaceBindingRow>,
    /// Typed binding truth: conversation binding rows (chat-first W2).
    pub conversation_document_bindings: Vec<ConversationBindingRow>,
    pub sessions: BTreeMap<String, ChatSession>,
    pub messages: BTreeMap<String, Vec<ChatMessage>>,
    pub user_preferences: BTreeMap<String, UserPreferences>,
    pub notifications: Vec<NotificationRow>,
    pub next_message_id: i64,
}

/// One workspace binding as PG stores it: own binding id, bound artifact +
/// workspace, and the artifact's latest parse run (version fact).
#[derive(Debug, Clone)]
pub struct WorkspaceBindingRow {
    pub binding_id: String,
    pub artifact_id: String,
    pub workspace_id: String,
    /// Latest completed parse run for the artifact; absent before the first
    /// parse completes.
    pub parse_version: Option<String>,
}

/// One conversation binding: own binding id + bound artifact (chat-first W2).
#[derive(Debug, Clone)]
pub struct ConversationBindingRow {
    pub binding_id: String,
    pub artifact_id: String,
    pub conversation_id: String,
    pub parse_version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StoredDocument {
    pub document: Document,
    pub content: String,
    pub summary: Option<String>,
    pub parsed_items: Vec<ParsedPreviewItem>,
}

#[derive(Debug, Clone)]
pub struct RetrievedContext {
    pub stored_document: StoredDocument,
    pub chunk_id: String,
    pub page: Option<usize>,
    pub score: f32,
    pub source_count: usize,
    pub source_ids: Vec<String>,
    pub sparse_hits: usize,
    pub dense_hits: usize,
}
