use avrag_auth::AuthContext;
use common::{ChatMessage, ChatSession, Document, Notebook, NotificationRow, ParsedPreviewItem};
use contracts::UserPreferences;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone)]
pub struct AppState {
    pub(crate) auth: AuthContext,
    pub(crate) storage: crate::storage_context::StorageContext,
    pub(crate) llm_ctx: crate::llm_context::LlmContext,
    pub(crate) orchestrator: crate::orchestrator_context::OrchestratorContext,
    pub(crate) analytics: crate::analytics_context::AnalyticsServiceCtx,
    pub(crate) billing: crate::billing_context::BillingContext,
    pub(crate) redis_url: String,
}

#[derive(Debug, Default)]
pub(crate) struct MemoryState {
    pub(crate) notebooks: BTreeMap<String, Notebook>,
    pub(crate) documents: BTreeMap<String, StoredDocument>,
    pub(crate) sessions: BTreeMap<String, ChatSession>,
    pub(crate) messages: BTreeMap<String, Vec<ChatMessage>>,
    pub(crate) user_preferences: BTreeMap<String, UserPreferences>,
    pub(crate) notifications: Vec<NotificationRow>,
    pub(crate) next_message_id: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct StoredDocument {
    pub(crate) document: Document,
    pub(crate) content: String,
    pub(crate) summary: Option<String>,
    pub(crate) parsed_items: Vec<ParsedPreviewItem>,
}

#[derive(Debug, Clone)]
pub(crate) struct RetrievedContext {
    pub(crate) stored_document: StoredDocument,
    pub(crate) chunk_id: String,
    pub(crate) page: Option<usize>,
    pub(crate) score: f32,
    pub(crate) source_count: usize,
    pub(crate) source_ids: Vec<String>,
    pub(crate) sparse_hits: usize,
    pub(crate) dense_hits: usize,
}
