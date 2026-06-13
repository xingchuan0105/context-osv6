//! E2E bootstrap config — build [`app::AppConfig`] without polluting process env.

use app::AppConfig;

/// Default multimodal embedding model for mock E2E (override via `MM_EMBEDDING_MODEL`).
pub(crate) fn default_e2e_mm_embedding_model() -> String {
    std::env::var("MM_EMBEDDING_MODEL")
        .unwrap_or_else(|_| "tongyi-embedding-vision-plus-2026-03-06".to_string())
}

/// Parameters captured during smoke bootstrap and passed to API + worker.
#[derive(Debug, Clone)]
pub(crate) struct E2eBootstrapConfig {
    /// Aligns API bootstrap auth with per-test `x-org-id` / `x-user-id` headers.
    pub org_id: String,
    pub user_id: String,
    pub database_url: String,
    pub auto_migrate: bool,
    pub object_root: String,
    pub enable_rag: bool,
    pub redis_url: String,
    pub milvus_url: Option<String>,
    pub milvus_collection_prefix: Option<String>,
    pub mock_llm_base_url: Option<String>,
    pub mock_embedding_base_url: Option<String>,
    pub mock_search_base_url: Option<String>,
    pub mock_paddle_ocr_base_url: Option<String>,
    pub use_real_llm: bool,
    pub has_real_search: bool,
    pub worker_timeout_secs: u64,
    /// Worker writes its bound health port here when `AVRAG_WORKER_HEALTH_PORT=0`.
    pub worker_health_port_file: String,
}

impl E2eBootstrapConfig {
    fn force_local_object_store(config: &mut AppConfig) {
        config.object_storage.endpoint.clear();
        config.object_storage.bucket.clear();
        config.object_storage.access_key.clear();
        config.object_storage.secret_key.clear();
    }

    fn apply_infra_overrides(&self, config: &mut AppConfig, base_url: &str) {
        config.org_id = self.org_id.clone();
        config.user_id = self.user_id.clone();
        config.database_url = Some(self.database_url.clone());
        config.auto_migrate = self.auto_migrate;
        config.object_root = self.object_root.clone();
        config.public_base_url = base_url.to_string();
        config.enable_rag = self.enable_rag;
        config.redis.url = self.redis_url.clone();
        config.redis.addr = self.redis_url.clone();
        Self::force_local_object_store(config);

        if let Some(ref url) = self.milvus_url {
            config.milvus.url = url.clone();
            config.milvus.token = String::new();
            config.milvus.database = "default".to_string();
            if let Some(ref prefix) = self.milvus_collection_prefix {
                config.milvus.collection_prefix = prefix.clone();
            }
        }
    }

    fn apply_search_from_env(config: &mut AppConfig) {
        avrag_search::sync_resolved_proxy_env();
        if let Ok(v) = std::env::var("SEARCH_PROVIDER") {
            config.search.provider = v;
        }
        if let Ok(v) = std::env::var("SEARCH_BASE_URL") {
            config.search.base_url = v;
        }
        if let Ok(v) = std::env::var("SEARCH_API_KEY") {
            config.search.api_key = v;
        }
        if let Ok(v) = std::env::var("SEARCH_MODE") {
            config.search.mode = v;
        }
        if let Ok(v) = std::env::var("SEARCH_MAX_RESULTS") {
            if let Ok(n) = v.parse() {
                config.search.max_results = n;
            }
        }
        if let Ok(v) = std::env::var("SEARCH_TIMEOUT_MS") {
            if let Ok(n) = v.parse() {
                config.search.timeout_ms = n;
            }
        }
    }

    pub(crate) fn build_app_config(&self, base_url: &str) -> AppConfig {
        let mut config = if self.use_real_llm {
            AppConfig::from_env()
        } else {
            AppConfig::default()
        };
        self.apply_infra_overrides(&mut config, base_url);

        if self.use_real_llm {
            return config;
        }

        if let Some(ref url) = self.mock_llm_base_url {
            for llm in [
                &mut config.agent_llm,
                &mut config.memory_llm,
                &mut config.ingestion_llm,
            ] {
                llm.base_url = url.clone();
                llm.api_key = "mock".to_string();
                llm.model = "mock-llm".to_string();
            }
        }

        if let Some(ref url) = self.mock_embedding_base_url {
            let mm_model = default_e2e_mm_embedding_model();
            config.embedding.base_url = url.clone();
            config.embedding.api_key = "mock".to_string();
            config.embedding.model = "mock-embedding".to_string();
            config.embedding.dimensions = Some(1024);
            config.milvus.text_vector_dim = 1024;

            config.mm_embedding.base_url = url.clone();
            config.mm_embedding.api_key = "mock".to_string();
            config.mm_embedding.model = mm_model.clone();
            config.mm_embedding.api_style =
                Some("dashscope_multimodal_embedding".to_string());
            config.mm_embedding.dimensions = Some(1024);
            config.milvus.multimodal_vector_dim = 1024;
        }

        if self.has_real_search {
            Self::apply_search_from_env(&mut config);
        } else if let Some(ref url) = self.mock_search_base_url {
            config.search.provider = "brave_llm_context".to_string();
            config.search.base_url = url.clone();
            config.search.api_key = "mock".to_string();
        }

        config
    }

    /// Inject worker process env. `NEXT_PUBLIC_DEV_ORG_ID` / `NEXT_PUBLIC_DEV_USER_ID`
    /// are shared with `AppConfig::from_env` (see `app-core` config) — worker has no
    /// separate `AVRAG_ORG_ID` / `AVRAG_USER_ID` keys; E2E must keep API headers,
    /// bootstrap config, and worker env aligned on the same pair.
    pub(crate) fn apply_worker_env(&self, cmd: &mut tokio::process::Command, base_url: &str) {
        cmd.env("E2E_ENABLED", "true")
            .env("NEXT_PUBLIC_DEV_ORG_ID", &self.org_id)
            .env("NEXT_PUBLIC_DEV_USER_ID", &self.user_id)
            .env("DATABASE_URL", &self.database_url)
            .env(
                "AVRAG_RUN_MIGRATIONS",
                if self.auto_migrate { "true" } else { "false" },
            )
            .env("AVRAG_OBJECT_ROOT", &self.object_root)
            .env(
                "AVRAG_ENABLE_RAG",
                if self.enable_rag { "true" } else { "false" },
            )
            .env("REDIS_URL", &self.redis_url)
            .env("AVRAG_PUBLIC_BASE_URL", base_url)
            .env("AVRAG_WORKER_ID", "test-worker")
            .env("AVRAG_WORKER_HEALTH_PORT", "0")
            .env("AVRAG_WORKER_HEALTH_PORT_FILE", &self.worker_health_port_file)
            .env("AVRAG_WORKER_POLL_MILLIS", "200")
            .env(
                "AVRAG_INGESTION_TASK_TIMEOUT_SECS",
                self.worker_timeout_secs.to_string(),
            );

        if let Some(ref url) = self.milvus_url {
            let prefix = self.milvus_collection_prefix.clone().unwrap_or_default();
            cmd.env("MILVUS_URL", url)
                .env("MILVUS_TOKEN", "")
                .env("MILVUS_DATABASE", "default")
                .env("MILVUS_COLLECTION_PREFIX", prefix);
        }

        if self.use_real_llm {
            for key in [
                "AGENT_LLM_BASE_URL",
                "AGENT_LLM_API_KEY",
                "AGENT_LLM_MODEL",
                "MEMORY_LLM_BASE_URL",
                "MEMORY_LLM_API_KEY",
                "MEMORY_LLM_MODEL",
                "INGESTION_LLM_BASE_URL",
                "INGESTION_LLM_API_KEY",
                "INGESTION_LLM_MODEL",
                "EMBEDDING_BASE_URL",
                "EMBEDDING_API_KEY",
                "EMBEDDING_MODEL",
                "EMBEDDING_DIMENSIONS",
                "AVRAG_EMBEDDING_DIM",
                "OFFICE_PARSER_BASE_URL",
                "PADDLE_OCR_BASE_URL",
                "PADDLE_OCR_API_TOKEN",
                "PADDLE_OCR_MODEL",
                "PDF_RENDERER_BASE_URL",
                "PDF_VISUAL_PAGES_PER_CHUNK",
                "INGESTION_PDF_MAX_PAGES",
                "INGESTION_TRIPLET_ENABLED",
                "INGESTION_VLM_TRIPLET_ENABLED",
                "INGESTION_VLM_SUMMARY_ENABLED",
                "INGESTION_TRIPLET_MIN_CONFIDENCE",
                "DASHSCOPE_API_KEY",
                "MM_EMBEDDING_BASE_URL",
                "MM_EMBEDDING_API_KEY",
                "MM_EMBEDDING_MODEL",
                "MM_EMBEDDING_API_STYLE",
                "MM_EMBEDDING_DIMENSIONS",
                "MM_RERANK_BASE_URL",
                "MM_RERANK_API_KEY",
                "MM_RERANK_MODEL",
                "MM_RERANK_API_STYLE",
            ] {
                if let Ok(v) = std::env::var(key) {
                    cmd.env(key, v);
                }
            }
        } else if let Some(ref url) = self.mock_llm_base_url {
            let mm_model = default_e2e_mm_embedding_model();
            cmd.env("AGENT_LLM_BASE_URL", url)
                .env("AGENT_LLM_API_KEY", "mock")
                .env("AGENT_LLM_MODEL", "mock-llm")
                .env("MEMORY_LLM_BASE_URL", url)
                .env("MEMORY_LLM_API_KEY", "mock")
                .env("MEMORY_LLM_MODEL", "mock-llm")
                .env("INGESTION_LLM_BASE_URL", url)
                .env("INGESTION_LLM_API_KEY", "mock")
                .env("INGESTION_LLM_MODEL", "mock-llm");

            if let Some(ref embed_url) = self.mock_embedding_base_url {
                cmd.env("EMBEDDING_BASE_URL", embed_url)
                    .env("EMBEDDING_API_KEY", "mock")
                    .env("EMBEDDING_MODEL", "mock-embedding")
                    .env("EMBEDDING_DIMENSIONS", "1024")
                    .env("AVRAG_EMBEDDING_DIM", "1024")
                    .env("MM_EMBEDDING_BASE_URL", embed_url)
                    .env("MM_EMBEDDING_API_KEY", "mock")
                    .env("MM_EMBEDDING_MODEL", &mm_model)
                    .env("MM_EMBEDDING_API_STYLE", "dashscope_multimodal_embedding")
                    .env("MM_EMBEDDING_DIMENSIONS", "1024")
                    .env("MILVUS_MULTIMODAL_VECTOR_DIM", "1024");
            }
        }

        if self.has_real_search {
            avrag_search::sync_resolved_proxy_env();
            for key in [
                "SEARCH_PROVIDER",
                "SEARCH_BASE_URL",
                "SEARCH_API_KEY",
                "SEARCH_MODE",
                "HTTPS_PROXY",
                "https_proxy",
                "HTTP_PROXY",
                "http_proxy",
            ] {
                if let Ok(v) = std::env::var(key) {
                    cmd.env(key, v);
                }
            }
        } else if let Some(ref url) = self.mock_search_base_url {
            cmd.env("SEARCH_PROVIDER", "brave_llm_context")
                .env("SEARCH_BASE_URL", url)
                .env("SEARCH_API_KEY", "mock");
        }

        if let Some(ref url) = self.mock_paddle_ocr_base_url {
            cmd.env("PADDLE_OCR_BASE_URL", url)
                .env("PADDLE_OCR_API_TOKEN", "mock")
                .env("PADDLE_OCR_MODEL", "mock-paddle")
                .env("PADDLE_OCR_POLL_INTERVAL_SECS", "1")
                .env("PADDLE_OCR_MAX_JOBS_PER_DOCUMENT", "5")
                .env("PADDLE_OCR_RESULT_CACHE_ENABLED", "0");
        } else {
            Self::forward_optional_env(
                cmd,
                &[
                    "PADDLE_OCR_BASE_URL",
                    "PADDLE_OCR_API_TOKEN",
                    "PADDLE_OCR_MODEL",
                    "PADDLE_OCR_MAX_JOBS_PER_DOCUMENT",
                    "PADDLE_OCR_RESULT_CACHE_ENABLED",
                ],
            );
        }

        Self::forward_optional_env(
            cmd,
            &[
                "LITEPARSE_OCR_ENABLED",
                "LITEPARSE_OCR_SERVER_URL",
                "LITEPARSE_OCR_LANGUAGE",
                "LITEPARSE_SCANNED_PAGE_THRESHOLD",
                "LITEPARSE_TABLE_GARBLE_THRESHOLD",
                "LITEPARSE_TABLE_HEAVY_THRESHOLD",
                "LITEPARSE_FIG_RATIO_THRESHOLD",
                "LITEPARSE_FIG_COUNT_THRESHOLD",
                "LITEPARSE_TEXT_QUAL_THRESHOLD",
                "LITEPARSE_DECORATIVE_MAX_AREA",
                "PDF_RENDERER_BASE_URL",
                "INGESTION_PDF_MAX_PAGES",
                "INGESTION_TRIPLET_ENABLED",
                "INGESTION_VLM_TRIPLET_ENABLED",
                "INGESTION_VLM_SUMMARY_ENABLED",
            ],
        );
    }

    fn forward_optional_env(cmd: &mut tokio::process::Command, keys: &[&str]) {
        for key in keys {
            if let Ok(value) = std::env::var(key) {
                cmd.env(key, value);
            }
        }
    }
}
