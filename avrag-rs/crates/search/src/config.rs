pub struct SearchConfig {
    pub provider: String,
    pub perplexity_api_key: Option<String>,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            provider: "perplexity".to_string(),
            perplexity_api_key: None,
        }
    }
}
