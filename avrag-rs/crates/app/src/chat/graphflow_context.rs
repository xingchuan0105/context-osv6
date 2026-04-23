#[derive(Clone)]
struct ChatFlowContext(Context);

impl From<Context> for ChatFlowContext {
    fn from(value: Context) -> Self {
        Self(value)
    }
}

impl ChatFlowContext {
    async fn set_request(&self, request: &ChatRequest) {
        self.0.set(KEY_REQUEST, request).await;
    }

    async fn request(&self) -> graph_flow::Result<ChatRequest> {
        self.get(KEY_REQUEST).await
    }

    async fn set_preflight(&self, preflight: &ChatPreflight) {
        self.0.set(KEY_PREFLIGHT, preflight).await;
    }

    async fn preflight(&self) -> graph_flow::Result<ChatPreflight> {
        self.get(KEY_PREFLIGHT).await
    }

    async fn set_session(&self, session: &ChatSession) {
        self.0.set(KEY_SESSION, session).await;
    }

    async fn session(&self) -> graph_flow::Result<ChatSession> {
        self.get(KEY_SESSION).await
    }

    async fn set_execution(&self, execution: &ChatGraphExecution) {
        self.0.set(KEY_EXECUTION, execution).await;
    }

    async fn execution(&self) -> graph_flow::Result<ChatGraphExecution> {
        self.get(KEY_EXECUTION).await
    }

    async fn set_rag_session_context(
        &self,
        session_context: &avrag_rag_core::context::SessionContext,
    ) {
        self.0.set(KEY_RAG_SESSION_CONTEXT, session_context).await;
    }

    async fn rag_session_context(&self) -> Option<avrag_rag_core::context::SessionContext> {
        self.0.get(KEY_RAG_SESSION_CONTEXT).await
    }

    async fn set_rag_plan(&self, plan: &common::RagPlan) {
        self.0.set(KEY_RAG_PLAN, plan).await;
    }

    async fn rag_plan(&self) -> graph_flow::Result<common::RagPlan> {
        self.get(KEY_RAG_PLAN).await
    }

    async fn set_rag_execute_response(&self, response: &common::ExecutePlanResponse) {
        self.0.set(KEY_RAG_EXECUTE_RESPONSE, response).await;
    }

    async fn rag_execute_response(&self) -> graph_flow::Result<common::ExecutePlanResponse> {
        self.get(KEY_RAG_EXECUTE_RESPONSE).await
    }

    async fn set_docscope_metadata(&self, metadata: &common::DocScopeMetadata) {
        self.0.set(KEY_DOCSCOPE_METADATA, metadata).await;
    }

    async fn docscope_metadata(&self) -> Option<common::DocScopeMetadata> {
        self.0.get(KEY_DOCSCOPE_METADATA).await
    }

    async fn set_text_dense_lists(&self, lists: &[avrag_rag_core::runtime::WeightedChunkList]) {
        self.0.set(KEY_TEXT_DENSE_LISTS, lists).await;
    }

    async fn text_dense_lists(&self) -> Option<Vec<avrag_rag_core::runtime::WeightedChunkList>> {
        self.0.get(KEY_TEXT_DENSE_LISTS).await
    }

    async fn set_bm25_lists(&self, lists: &[avrag_rag_core::runtime::WeightedChunkList]) {
        self.0.set(KEY_BM25_LISTS, lists).await;
    }

    async fn bm25_lists(&self) -> Option<Vec<avrag_rag_core::runtime::WeightedChunkList>> {
        self.0.get(KEY_BM25_LISTS).await
    }

    async fn set_multimodal_pool(&self, chunks: &[avrag_rag_core::retrieval::ScoredChunk]) {
        self.0.set(KEY_MULTIMODAL_POOL, chunks).await;
    }

    async fn multimodal_pool(&self) -> Option<Vec<avrag_rag_core::retrieval::ScoredChunk>> {
        self.0.get(KEY_MULTIMODAL_POOL).await
    }

    async fn set_text_pool(&self, chunks: &[avrag_rag_core::retrieval::ScoredChunk]) {
        self.0.set(KEY_TEXT_POOL, chunks).await;
    }

    async fn text_pool(&self) -> Option<Vec<avrag_rag_core::retrieval::ScoredChunk>> {
        self.0.get(KEY_TEXT_POOL).await
    }

    async fn set_reranked_chunks(&self, chunks: &[avrag_rag_core::retrieval::ScoredChunk]) {
        self.0.set(KEY_RERANKED_CHUNKS, chunks).await;
    }

    async fn reranked_chunks(&self) -> Option<Vec<avrag_rag_core::retrieval::ScoredChunk>> {
        self.0.get(KEY_RERANKED_CHUNKS).await
    }

    async fn set_summary_chunks(&self, summaries: &[(Uuid, String)]) {
        self.0.set(KEY_SUMMARY_CHUNKS, summaries).await;
    }

    async fn summary_chunks(&self) -> Option<Vec<(Uuid, String)>> {
        self.0.get(KEY_SUMMARY_CHUNKS).await
    }

    async fn set_answer_context(&self, chunks: &[common::AnswerContextChunk]) {
        self.0.set(KEY_ANSWER_CONTEXT, chunks).await;
    }

    async fn answer_context(&self) -> Option<Vec<common::AnswerContextChunk>> {
        self.0.get(KEY_ANSWER_CONTEXT).await
    }

    async fn set_retrieved_chunks(&self, chunks: &[avrag_rag_core::retrieval::ScoredChunk]) {
        self.0.set(KEY_RETRIEVED_CHUNKS, chunks).await;
    }

    async fn retrieved_chunks(
        &self,
    ) -> graph_flow::Result<Vec<avrag_rag_core::retrieval::ScoredChunk>> {
        self.get(KEY_RETRIEVED_CHUNKS).await
    }

    async fn set_item_trace(&self, trace: &[common::RagTraceItem]) {
        self.0.set(KEY_ITEM_TRACE, trace).await;
    }

    async fn item_trace(&self) -> graph_flow::Result<Vec<common::RagTraceItem>> {
        self.get(KEY_ITEM_TRACE).await
    }

    async fn set_degrade_trace(&self, trace: &[common::DegradeTraceItem]) {
        self.0.set(KEY_DEGRADE_TRACE, trace).await;
    }

    async fn degrade_trace(&self) -> Option<Vec<common::DegradeTraceItem>> {
        self.0.get(KEY_DEGRADE_TRACE).await
    }

    async fn set_response(&self, response: &ChatResponse) {
        self.0.set(KEY_RESPONSE, response).await;
    }

    async fn response(&self) -> Option<ChatResponse> {
        self.0.get(KEY_RESPONSE).await
    }

    async fn raw_response(&self) -> Option<serde_json::Value> {
        self.0.get(KEY_RESPONSE).await
    }

    async fn get<T>(&self, key: &str) -> graph_flow::Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        self.0
            .get(key)
            .await
            .ok_or_else(|| GraphError::ContextError(format!("missing graph context key: {key}")))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ChatPreflight {
    pub trace_id: String,
    pub user_uuid: Uuid,
    pub notebook_uuid: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ChatGraphExecution {
    pub mode: String,
    pub input_usage_text: String,
    pub apply_output_guard: bool,
    pub response: ChatResponse,
    pub llm_usage: Option<avrag_llm::LlmUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FlowAppErrorData {
    kind: String,
    code: String,
    message: String,
    http_status: u16,
    retry_after_secs: Option<u64>,
}

impl From<AppError> for FlowAppErrorData {
    fn from(value: AppError) -> Self {
        let kind = match value {
            AppError::Validation { .. } => "validation",
            AppError::NotFound { .. } => "not_found",
            AppError::Conflict { .. } => "conflict",
            AppError::Internal { .. } => "internal",
            AppError::RateLimited { .. } => "rate_limited",
        }
        .to_string();

        Self {
            kind,
            code: value.code().to_string(),
            message: value.message().to_string(),
            http_status: value.http_status(),
            retry_after_secs: value.retry_after_secs(),
        }
    }
}

impl FlowAppErrorData {
    fn into_app_error(self) -> AppError {
        let code: &'static str = Box::leak(self.code.into_boxed_str());
        match self.kind.as_str() {
            "validation" => AppError::Validation {
                code,
                message: self.message,
                http_status: self.http_status,
            },
            "not_found" => AppError::NotFound {
                code,
                message: self.message,
                http_status: self.http_status,
            },
            "conflict" => AppError::Conflict {
                code,
                message: self.message,
                http_status: self.http_status,
            },
            "rate_limited" => AppError::RateLimited {
                code,
                message: self.message,
                http_status: self.http_status,
                retry_after_secs: self.retry_after_secs.unwrap_or_default(),
            },
            _ => AppError::Internal {
                code,
                message: self.message,
                http_status: self.http_status,
            },
        }
    }
}
