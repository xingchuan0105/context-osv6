/// Web search backend configuration.
///
/// - `provider`: `qwen_web` | `deepseek_web` | `deepseek_web_brave` | `brave_llm_context`
/// - Qwen uses dashscope Responses API native `web_search` (`SEARCH_QWEN_*`, fallback `RETRIEVE_LLM_*`)
/// - Brave uses `base_url` + `api_key` (SEARCH_BASE_URL / SEARCH_API_KEY)
/// - DeepSeek Anthropic server web_search uses `deepseek_*` (defaults from AGENT_LLM_*)
/// - Optional CRW (`CRW_BASE_URL`) auto-scrapes top-K thin-snippet URLs after search
pub struct SearchConfig {
    pub provider: String,
    /// Brave Search API base (or full path prefix).
    pub base_url: String,
    /// Brave API key.
    pub api_key: String,
    /// DeepSeek API root (OpenAI-style base; Anthropic path is `{root}/anthropic`).
    pub deepseek_base_url: String,
    pub deepseek_api_key: String,
    pub deepseek_model: String,
    /// dashscope OpenAI-compatible root (`{root}/responses` is invoked).
    pub qwen_base_url: String,
    pub qwen_api_key: String,
    pub qwen_model: String,
    pub max_results: usize,
    pub timeout_ms: u64,
    pub search_lang: Option<String>,
    pub country: Option<String>,
    pub freshness: Option<String>,
    /// Host auto-scrape after web search (CRW `/v1/scrape`). Default on when base set.
    pub auto_scrape_enabled: bool,
    /// CRW HTTP root, e.g. `http://127.0.0.1:3000`. Empty disables even if enabled.
    pub crw_base_url: String,
    /// Optional Bearer for managed CRW; local docker usually empty.
    pub crw_api_key: String,
    /// Global unique URLs to scrape per search response (default 4).
    pub auto_scrape_top_k: usize,
    /// Max characters stored into each result snippet after scrape.
    pub auto_scrape_max_chars: usize,
    /// Skip scrape when existing snippet already has at least this many chars.
    pub auto_scrape_min_snippet: usize,
    pub auto_scrape_timeout_ms: u64,
    pub auto_scrape_concurrency: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            // Qwen (dashscope) Responses API native web_search; LLM-native, no Brave key needed.
            provider: "qwen_web".to_string(),
            base_url: "https://api.search.brave.com".to_string(),
            api_key: String::new(),
            deepseek_base_url: "https://api.deepseek.com".to_string(),
            deepseek_api_key: String::new(),
            deepseek_model: "deepseek-v4-flash".to_string(),
            qwen_base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
            qwen_api_key: String::new(),
            qwen_model: "qwen3.7-flash".to_string(),
            max_results: 10,
            timeout_ms: 30_000,
            search_lang: None,
            country: None,
            freshness: None,
            // Prefer enrich when a local/default CRW is configured via env in AppConfig.
            auto_scrape_enabled: true,
            crw_base_url: String::new(),
            crw_api_key: String::new(),
            auto_scrape_top_k: 4,
            auto_scrape_max_chars: 4_000,
            auto_scrape_min_snippet: 80,
            auto_scrape_timeout_ms: 12_000,
            auto_scrape_concurrency: 4,
        }
    }
}
