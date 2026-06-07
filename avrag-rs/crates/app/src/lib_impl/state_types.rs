use crate::agents::service::UnifiedAgentService;
use avrag_auth::AuthContext;
use avrag_chatmemory::ChatMemory;
use avrag_guardrails::GuardPipeline;
use avrag_llm::LlmClient;
use avrag_rag_core::RagRuntime;
use avrag_storage_pg::{ObjectStoreHandle, PgAppRepository};
use common::key_vault::KeyVault;
use common::{
    ApiKeyRow, ChatMessage, ChatSession, Document, Notebook, NotificationRow, ParsedPreviewItem,
};
use contracts::UserPreferences;
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub(crate) auth: AuthContext,
    pub(crate) pg: Option<Arc<PgAppRepository>>,
    pub(crate) inner: Arc<RwLock<MemoryState>>,
    pub(crate) llm_client: Option<LlmClient>,
    pub(crate) memory_llm_client: Option<LlmClient>,
    pub(crate) chatmemory: Option<Arc<ChatMemory>>,
    pub(crate) analytics: Option<Arc<analytics::AnalyticsService>>,
    pub(crate) quota_manager: Option<Arc<avrag_billing::QuotaManager>>,
    pub(crate) rag_runtime: Option<Arc<RagRuntime>>,
    pub(crate) agent_service: Option<Arc<UnifiedAgentService>>,
    pub(crate) object_store: Arc<ObjectStoreHandle>,
    pub(crate) guard_pipeline: Arc<GuardPipeline>,
    pub(crate) uses_memory_adapters: bool,
    pub(crate) public_base_url: String,
    pub(crate) object_root: String,
    pub(crate) usage_limit_phase: String,
    pub(crate) search_provider: String,
    pub(crate) search_mode: String,
    pub(crate) redis_url: String,
    pub(crate) object_storage_upload_expire_sec: u64,
    pub(crate) object_storage_download_expire_sec: u64,
    pub(crate) max_upload_file_size_bytes: u64,
    pub(crate) api_keys: Arc<RwLock<BTreeMap<String, Vec<ApiKeyRow>>>>,
    pub(crate) key_vault: Arc<dyn KeyVault>,
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
