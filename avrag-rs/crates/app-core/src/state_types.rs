use common::{Document, NotificationRow, ParsedPreviewItem};
use contracts::chat::{ChatMessage};
use contracts::notebooks::{ChatSession, Notebook};
use contracts::UserPreferences;
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct MemoryState {
    pub notebooks: BTreeMap<String, Notebook>,
    pub documents: BTreeMap<String, StoredDocument>,
    pub sessions: BTreeMap<String, ChatSession>,
    pub messages: BTreeMap<String, Vec<ChatMessage>>,
    pub user_preferences: BTreeMap<String, UserPreferences>,
    pub notifications: Vec<NotificationRow>,
    pub next_message_id: i64,
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
