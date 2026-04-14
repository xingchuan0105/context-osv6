#[derive(Clone)]
pub struct AppState {
    config: AppConfig,
    auth: AuthContext,
    pg: Option<Arc<PgAppRepository>>,
    inner: Arc<RwLock<MemoryState>>,
    llm_client: Option<LlmClient>,
    summary_llm_client: Option<LlmClient>,
    chatmemory: Option<Arc<ChatMemory>>,
    analytics: Option<Arc<analytics::AnalyticsService>>,
    usage_limit: Option<Arc<avrag_usage_limit::UsageLimitService>>,
    search_executor: Option<Arc<SearchExecutor>>,
    rag_runtime: Option<Arc<RagRuntime>>,
    object_store: Arc<ObjectStoreHandle>,
    guard_pipeline: Arc<GuardPipeline>,
    uses_memory_adapters: bool,
}

#[derive(Debug, Default)]
struct MemoryState {
    notebooks: BTreeMap<String, Notebook>,
    documents: BTreeMap<String, StoredDocument>,
    sessions: BTreeMap<String, ChatSession>,
    messages: BTreeMap<String, Vec<ChatMessage>>,
    api_keys: BTreeMap<String, Vec<ApiKeyRow>>,
    user_preferences: BTreeMap<String, UserPreferences>,
    notifications: Vec<NotificationRow>,
    next_message_id: i64,
}

#[derive(Debug, Clone)]
struct StoredDocument {
    document: Document,
    content: String,
    summary: Option<String>,
    parsed_items: Vec<ParsedPreviewItem>,
}

#[derive(Debug, Clone)]
struct RetrievedContext {
    stored_document: StoredDocument,
    chunk_id: String,
    page: Option<usize>,
    score: f32,
    source_count: usize,
    source_ids: Vec<String>,
    sparse_hits: usize,
    dense_hits: usize,
}
