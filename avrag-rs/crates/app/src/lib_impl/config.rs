use anyhow::Result as AnyResult;
use avrag_auth::{ActorId, AuthContext, OrgId, SubjectKind};
use avrag_billing as billing;
use avrag_chatmemory::ChatMemory;
use avrag_guardrails::GuardPipeline;
use avrag_llm::{AnswerSynthesizer, EmbeddingClient, LlmClient, RerankerClient, RetrievalPlanner};
use avrag_rag_core::{
    RagConfig, RagRuntime, RetrievalDataPlane, context::SessionContext as RagSessionContext,
};
use avrag_search::SearchExecutor;
use avrag_storage_milvus::{MilvusConfig as StorageMilvusConfig, MilvusDataPlane};
use avrag_storage_pg::{
    DocumentAssetRow, DocumentDeletionOutcome, DocumentScopeState, DocumentTaskSeed,
    DocumentUploadMutationOutcome, DocumentUploadQueueOutcome, NotificationCreateParams, ObjectStoreHandle,
    ObjectStoreHeadError, PgAppRepository, PgStorageError, S3ObjectStore,
};
use common::{
    AddUrlSourceRequest, ApiKeyRow, AppError, ChatMessage, ChatRequest, ChatResponse, ChatSession,
    Citation, CitationLookupResponse, CreateApiKeyRequest, CreateApiKeyResponse,
    CreateChatSessionRequest, CreateDocumentRequest, CreateDocumentUploadResponse,
    CreateNotebookRequest, DegradeTraceItem, Document, DocumentContentResponse, DocumentStatus,
    ModeDebug, Notebook, NotificationRow, ParsedPreviewItem, ParsedPreviewResponse, PlannerOutput,
    RagModeDebug, RagPlan, RagPlanItem, RagTraceItem, RagTraceSummary, ShareTokenResponse,
    SourceRef, SourceRow, StatusOnlyResponse, SummaryInjectionTrace, UpdateChatSessionRequest,
    UpdateDocumentRequest, UpdateNotebookRequest, default_org_id, default_user_id, new_id,
    now_rfc3339,
};
use contracts::UserPreferences;
use hmac::{Hmac, Mac};
use ingestion::parser::{DocumentParser, HtmlParser};
use ingestion::{
    AuditAction, IngestDocumentPayload, ReindexDocumentPayload, ReindexReason, build_ingest_task,
    build_reindex_task, task_audit,
};
use reqwest::{Client as HttpClient, Url, header::CONTENT_TYPE, redirect::Policy};
use sha2::Sha256;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{
    fs,
    sync::RwLock,
    time::{Duration, sleep},
};
use tracing::info;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub public_base_url: String,
    pub org_id: String,
    pub user_id: String,
    pub database_url: Option<String>,
    pub auto_migrate: bool,
    pub object_root: String,
    pub milvus: MilvusConfig,
    pub embedding: ModelProviderConfig,
    pub mm_embedding: ModelProviderConfig,
    pub mm_rerank: ModelProviderConfig,
    pub rerank: ModelProviderConfig,
    pub intent_llm: ModelProviderConfig,
    pub answer_llm: ModelProviderConfig,
    pub summary_llm: ModelProviderConfig,
    pub search_llm: ModelProviderConfig,
    pub search: SearchConfig,
    pub redis: RedisConfig,
    pub object_storage: ObjectStorageConfig,
    pub prompts: PromptConfig,
    pub usage_limit: UsageLimitConfig,
}

#[derive(Debug, Clone)]
pub struct ModelProviderConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_ms: u64,
    pub temperature: Option<f32>,
    pub api_style: Option<String>,
    pub dimensions: Option<usize>,
    pub enable_thinking: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct MilvusConfig {
    pub url: String,
    pub token: String,
    pub database: String,
    pub collection_prefix: String,
    pub text_vector_dim: usize,
    pub multimodal_vector_dim: usize,
    pub metric_type: String,
}

#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub mode: String,
    pub enable_thinking: bool,
    pub tools: Vec<String>,
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub max_results: usize,
    pub max_sub_queries: usize,
    pub timeout_ms: u64,
    pub citation_required: bool,
    pub planner_enabled: bool,
    pub query_type_enabled: bool,
    pub extract_enabled: bool,
    pub perplexity_api_key: Option<String>,
    pub perplexity_model: String,
}

#[derive(Debug, Clone)]
pub struct RedisConfig {
    pub url: String,
    pub addr: String,
    pub password: String,
    pub db: i64,
}

#[derive(Debug, Clone)]
pub struct ObjectStorageConfig {
    pub endpoint: String,
    pub bucket: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
    pub use_path_style: bool,
    pub upload_url_expire_sec: u64,
    pub download_url_expire_sec: u64,
}

#[derive(Debug, Clone)]
pub struct PromptConfig {
    pub dir: String,
    pub intent_template_path: String,
    pub intent_version: String,
    pub summary_version: String,
}

#[derive(Debug, Clone)]
pub struct UsageLimitConfig {
    /// Which enforcement phases are active.
    /// Options: "shadow", "visibility", "5h_enforcement", "7d_enforcement"
    pub enforcement_phase: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            public_base_url: "http://127.0.0.1:8080".to_string(),
            org_id: default_org_id(),
            user_id: default_user_id(),
            database_url: None,
            auto_migrate: true,
            object_root: default_object_root(),
            milvus: MilvusConfig {
                url: "http://127.0.0.1:19530".to_string(),
                token: String::new(),
                database: "default".to_string(),
                collection_prefix: "avrag".to_string(),
                text_vector_dim: 1024,
                multimodal_vector_dim: 1024,
                metric_type: "COSINE".to_string(),
            },
            embedding: ModelProviderConfig {
                base_url: "https://api.siliconflow.cn/v1".to_string(),
                api_key: String::new(),
                model: "Qwen/Qwen3-Embedding-8B".to_string(),
                timeout_ms: 15000,
                temperature: None,
                api_style: None,
                dimensions: None,
                enable_thinking: None,
            },
            mm_embedding: ModelProviderConfig {
                base_url: "https://dashscope.aliyuncs.com/api/v1/services/embeddings/multimodal-embedding/multimodal-embedding".to_string(),
                api_key: String::new(),
                model: "qwen3-vl-embedding".to_string(),
                timeout_ms: 30000,
                temperature: None,
                api_style: Some("dashscope_multimodal_embedding".to_string()),
                dimensions: None,
                enable_thinking: None,
            },
            mm_rerank: ModelProviderConfig {
                base_url: "https://dashscope.aliyuncs.com/api/v1/services/rerank/text-rerank/text-rerank".to_string(),
                api_key: String::new(),
                model: "qwen3-vl-rerank".to_string(),
                timeout_ms: 30000,
                temperature: None,
                api_style: Some("dashscope_vl_rerank".to_string()),
                dimensions: None,
                enable_thinking: None,
            },
            rerank: ModelProviderConfig {
                base_url: "https://api.siliconflow.cn/v1".to_string(),
                api_key: String::new(),
                model: "Qwen/Qwen3-Reranker-8B".to_string(),
                timeout_ms: 15000,
                temperature: None,
                api_style: None,
                dimensions: None,
                enable_thinking: None,
            },
            intent_llm: ModelProviderConfig {
                base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
                api_key: String::new(),
                model: "qwen3.5-flash".to_string(),
                timeout_ms: 30000,
                temperature: None,
                api_style: None,
                dimensions: None,
                enable_thinking: Some(false),
            },
            answer_llm: ModelProviderConfig {
                base_url: "https://www.dmxapi.cn/v1".to_string(),
                api_key: String::new(),
                model: "gemini-3-flash-preview-thinking".to_string(),
                timeout_ms: 180000,
                temperature: Some(0.2),
                api_style: Some("openai".to_string()),
                dimensions: None,
                enable_thinking: None,
            },
            summary_llm: ModelProviderConfig {
                base_url: "https://www.dmxapi.cn/v1".to_string(),
                api_key: String::new(),
                model: "gemini-3.1-flash-lite-preview".to_string(),
                timeout_ms: 30000,
                temperature: None,
                api_style: Some("openai".to_string()),
                dimensions: None,
                enable_thinking: None,
            },
            search_llm: ModelProviderConfig {
                base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
                api_key: String::new(),
                model: "qwen3.5-plus".to_string(),
                timeout_ms: 30000,
                temperature: None,
                api_style: Some("auto".to_string()),
                dimensions: None,
                enable_thinking: None,
            },
            search: SearchConfig {
                mode: "llm_tools".to_string(),
                enable_thinking: true,
                tools: vec![
                    "web_search".to_string(),
                    "web_extractor".to_string(),
                    "code_interpreter".to_string(),
                ],
                provider: "perplexity".to_string(), // Changed default to perplexity
                base_url: "https://api.exa.ai".to_string(),
                api_key: String::new(),
                max_results: 10,
                max_sub_queries: 3,
                timeout_ms: 30000,
                citation_required: true,
                planner_enabled: true,
                query_type_enabled: true,
                extract_enabled: false,
                perplexity_api_key: None,
                perplexity_model: "nvidia/nemotron-3-super-120b-a12b".to_string(),
            },
            redis: RedisConfig {
                url: "redis://127.0.0.1:6379".to_string(),
                addr: "127.0.0.1:6379".to_string(),
                password: String::new(),
                db: 0,
            },
            object_storage: ObjectStorageConfig {
                endpoint: String::new(),
                bucket: String::new(),
                region: "us-east-1".to_string(),
                access_key: String::new(),
                secret_key: String::new(),
                use_path_style: true,
                upload_url_expire_sec: 900,
                download_url_expire_sec: 900,
            },
            prompts: PromptConfig {
                dir: "./prompts".to_string(),
                intent_template_path: String::new(),
                intent_version: "freeze-v2".to_string(),
                summary_version: "v1".to_string(),
            },
            usage_limit: UsageLimitConfig {
                enforcement_phase: "shadow".to_string(),
            },
        }
    }
}

impl AppConfig {
    pub fn from_env() -> Self {
        let mut config = Self::default();

        config.public_base_url = env_string("AVRAG_PUBLIC_BASE_URL", &config.public_base_url);
        config.org_id = env_string("NEXT_PUBLIC_DEV_ORG_ID", &config.org_id);
        config.user_id = env_string("NEXT_PUBLIC_DEV_USER_ID", &config.user_id);
        config.database_url = env_optional_string("DATABASE_URL");
        config.auto_migrate = env_bool("AVRAG_RUN_MIGRATIONS", config.auto_migrate);
        config.object_root = env_string("AVRAG_OBJECT_ROOT", &config.object_root);

        config.milvus.url = env_string("MILVUS_URL", &config.milvus.url);
        config.milvus.token = env_string("MILVUS_TOKEN", &config.milvus.token);
        config.milvus.database = env_string("MILVUS_DATABASE", &config.milvus.database);
        config.milvus.collection_prefix =
            env_string("MILVUS_COLLECTION_PREFIX", &config.milvus.collection_prefix);
        config.milvus.metric_type = env_string("MILVUS_METRIC_TYPE", &config.milvus.metric_type);

        config.embedding = model_config_from_env(
            "EMBEDDING",
            &config.embedding,
            env_optional_string("SILICONFLOW_API_KEY"),
        );
        config.embedding.dimensions =
            env_usize_optional("AVRAG_EMBEDDING_DIM").or(config.embedding.dimensions);
        config.mm_embedding = model_config_from_env(
            "MM_EMBEDDING",
            &config.mm_embedding,
            env_optional_string("DASHSCOPE_API_KEY"),
        );
        config.mm_rerank = model_config_from_env(
            "MM_RERANK",
            &config.mm_rerank,
            env_optional_string("DASHSCOPE_API_KEY"),
        );
        config.rerank = model_config_from_env(
            "RERANK",
            &config.rerank,
            env_optional_string("SILICONFLOW_API_KEY"),
        );
        config.milvus.text_vector_dim = env_usize(
            "MILVUS_TEXT_VECTOR_DIM",
            config
                .embedding
                .dimensions
                .unwrap_or(config.milvus.text_vector_dim),
        );
        config.milvus.multimodal_vector_dim = env_usize(
            "MILVUS_MULTIMODAL_VECTOR_DIM",
            config
                .mm_embedding
                .dimensions
                .unwrap_or(config.milvus.multimodal_vector_dim),
        );
        config.intent_llm = model_config_from_env("INTENT_LLM", &config.intent_llm, None);
        config.answer_llm = model_config_from_env("ANSWER_LLM", &config.answer_llm, None);
        config.summary_llm = model_config_from_env("SUMMARY_LLM", &config.summary_llm, None);
        config.search_llm = model_config_from_env("SEARCH_LLM", &config.search_llm, None);

        config.search.mode = env_string("SEARCH_MODE", &config.search.mode);
        config.search.enable_thinking =
            env_bool("SEARCH_ENABLE_THINKING", config.search.enable_thinking);
        config.search.tools = env_csv("SEARCH_TOOLS", &config.search.tools);
        config.search.provider = env_string("SEARCH_PROVIDER", &config.search.provider);
        config.search.base_url = env_string("SEARCH_BASE_URL", &config.search.base_url);
        config.search.api_key = env_string("SEARCH_API_KEY", &config.search.api_key);
        config.search.max_results = env_usize("SEARCH_MAX_RESULTS", config.search.max_results);
        config.search.max_sub_queries =
            env_usize("SEARCH_MAX_SUB_QUERIES", config.search.max_sub_queries);
        config.search.timeout_ms = env_u64("SEARCH_TIMEOUT_MS", config.search.timeout_ms);
        config.search.citation_required =
            env_bool("SEARCH_CITATION_REQUIRED", config.search.citation_required);
        config.search.planner_enabled =
            env_bool("SEARCH_PLANNER_ENABLED", config.search.planner_enabled);
        config.search.query_type_enabled = env_bool(
            "SEARCH_QUERY_TYPE_ENABLED",
            config.search.query_type_enabled,
        );
        config.search.extract_enabled =
            env_bool("SEARCH_EXTRACT_ENABLED", config.search.extract_enabled);
        config.search.perplexity_api_key = env_optional_string("PERPLEXITY_API_KEY");
        config.search.perplexity_model =
            env_string("PERPLEXITY_MODEL", &config.search.perplexity_model);
        config.redis.addr = env_string("REDIS_ADDR", &config.redis.addr);
        config.redis.password = env_string("REDIS_PASSWORD", &config.redis.password);
        config.redis.db = env_i64("REDIS_DB", config.redis.db);
        config.redis.url = env_string(
            "REDIS_URL",
            &build_redis_url(&config.redis.addr, &config.redis.password, config.redis.db),
        );

        config.object_storage.endpoint = env_string(
            "S3_ENDPOINT",
            &env_string("MINIO_ENDPOINT", &config.object_storage.endpoint),
        );
        config.object_storage.bucket = env_string(
            "S3_BUCKET",
            &env_string("MINIO_BUCKET", &config.object_storage.bucket),
        );
        config.object_storage.region = env_string("S3_REGION", &config.object_storage.region);
        config.object_storage.access_key = env_string(
            "S3_ACCESS_KEY",
            &env_string("MINIO_ACCESS_KEY", &config.object_storage.access_key),
        );
        config.object_storage.secret_key = env_string(
            "S3_SECRET_KEY",
            &env_string("MINIO_SECRET_KEY", &config.object_storage.secret_key),
        );
        config.object_storage.use_path_style =
            env_bool("S3_USE_PATH_STYLE", config.object_storage.use_path_style);
        config.object_storage.upload_url_expire_sec = env_u64(
            "UPLOAD_URL_EXPIRE_SEC",
            config.object_storage.upload_url_expire_sec,
        );
        config.object_storage.download_url_expire_sec = env_u64(
            "DOWNLOAD_URL_EXPIRE_SEC",
            config.object_storage.download_url_expire_sec,
        );

        config.prompts.dir = env_string("PROMPT_DIR", &config.prompts.dir);
        config.prompts.intent_template_path = env_string(
            "PROMPT_INTENT_TEMPLATE_PATH",
            &config.prompts.intent_template_path,
        );
        config.prompts.intent_version =
            env_string("PROMPT_INTENT_VERSION", &config.prompts.intent_version);
        config.prompts.summary_version =
            env_string("PROMPT_SUMMARY_VERSION", &config.prompts.summary_version);

        config.usage_limit.enforcement_phase = env_string(
            "USAGE_LIMIT_ENFORCEMENT_PHASE",
            &config.usage_limit.enforcement_phase,
        );

        config
    }
}
