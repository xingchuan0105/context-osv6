pub struct SearchConfig {
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub max_results: usize,
    pub search_lang: Option<String>,
    pub country: Option<String>,
    pub freshness: Option<String>,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            provider: "brave_llm_context".to_string(),
            base_url: "https://api.search.brave.com".to_string(),
            api_key: String::new(),
            max_results: 10,
            search_lang: None,
            country: None,
            freshness: None,
        }
    }
}
