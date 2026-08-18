use crate::{SearchConfig, SearchExecutor};

#[test]
fn default_provider_is_qwen_web() {
    assert_eq!(SearchConfig::default().provider, "qwen_web");
}

#[tokio::test]
async fn missing_qwen_key_is_explicit_error() {
    // Default provider is qwen_web; with no dashscope key the error is explicit.
    let executor = SearchExecutor::new(SearchConfig::default());
    let request = contracts::chat::ChatRequest {
        query: "test".to_string(),
        workspace_id: None,
        session_id: None,
        agent_type: "search".to_string(),
        capabilities: None,
        client_context: None,
        client_ip: None,
        source_type: None,
        source_token: None,
        doc_scope: Vec::new(),
        messages: Vec::new(),
        stream: false,
        debug: false,
        language: None,
        format_hint: None,
        turnstile_token: None,
    };
    let auth = contracts::auth_runtime::AuthContext::new(
        contracts::auth_runtime::UserId::from(uuid::Uuid::nil()),
        contracts::auth_runtime::SubjectKind::User,
    );
    let error = executor.execute(&request, &auth).await.unwrap_err();
    let msg = error.to_string();
    assert!(
        msg.contains("Qwen web search API key not configured"),
        "unexpected error: {msg}"
    );
}

#[tokio::test]
async fn missing_brave_key_on_brave_only_provider_is_explicit_error() {
    let executor = SearchExecutor::new(SearchConfig {
        provider: "brave_llm_context".to_string(),
        ..SearchConfig::default()
    });
    let request = contracts::chat::ChatRequest {
        query: "test".to_string(),
        workspace_id: None,
        session_id: None,
        agent_type: "search".to_string(),
        capabilities: None,
        client_context: None,
        client_ip: None,
        source_type: None,
        source_token: None,
        doc_scope: Vec::new(),
        messages: Vec::new(),
        stream: false,
        debug: false,
        language: None,
        format_hint: None,
        turnstile_token: None,
    };
    let auth = contracts::auth_runtime::AuthContext::new(
        contracts::auth_runtime::UserId::from(uuid::Uuid::nil()),
        contracts::auth_runtime::SubjectKind::User,
    );
    let error = executor.execute(&request, &auth).await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Brave LLM Context API key not configured")
    );
}

#[tokio::test]
async fn unsupported_provider_is_explicit_error() {
    let executor = SearchExecutor::new(SearchConfig {
        provider: "exa".to_string(),
        ..SearchConfig::default()
    });
    let request = contracts::chat::ChatRequest {
        query: "test".to_string(),
        workspace_id: None,
        session_id: None,
        agent_type: "search".to_string(),
        capabilities: None,
        client_context: None,
        client_ip: None,
        source_type: None,
        source_token: None,
        doc_scope: Vec::new(),
        messages: Vec::new(),
        stream: false,
        debug: false,
        language: None,
        format_hint: None,
        turnstile_token: None,
    };
    let auth = contracts::auth_runtime::AuthContext::new(
        contracts::auth_runtime::UserId::from(uuid::Uuid::nil()),
        contracts::auth_runtime::SubjectKind::User,
    );
    let error = executor.execute(&request, &auth).await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unsupported search provider: exa")
    );
}

#[tokio::test]
#[ignore = "requires external network connectivity to Brave Search API"]
async fn executor_routes_news_vertical_to_news_endpoint() {
    let executor = SearchExecutor::new(SearchConfig {
        provider: "deepseek_web_brave".to_string(),
        api_key: "dummy".to_string(),
        deepseek_api_key: "dummy".to_string(),
        ..SearchConfig::default()
    });
    let error = executor
        .execute_search("test", Some("news"))
        .await
        .unwrap_err();
    let msg = error.to_string();
    assert!(
        msg.contains("brave news api error"),
        "expected news endpoint error, got: {}",
        msg
    );
}

#[tokio::test]
#[ignore = "requires live Brave Search API credentials in SEARCH_API_KEY"]
async fn brave_llm_context_live_smoke_returns_grounding_sources() {
    let Ok(api_key) = std::env::var("SEARCH_API_KEY") else {
        return;
    };
    if api_key.trim().is_empty() {
        return;
    }

    let executor = SearchExecutor::new(SearchConfig {
        provider: "brave_llm_context".to_string(),
        api_key,
        max_results: 3,
        ..SearchConfig::default()
    });
    let request = contracts::chat::ChatRequest {
        query: "What is the Brave Search LLM Context API?".to_string(),
        workspace_id: None,
        session_id: None,
        agent_type: "search".to_string(),
        capabilities: None,
        client_context: None,
        client_ip: None,
        source_type: None,
        source_token: None,
        doc_scope: Vec::new(),
        messages: Vec::new(),
        stream: false,
        debug: false,
        language: None,
        format_hint: None,
        turnstile_token: None,
    };
    let auth = contracts::auth_runtime::AuthContext::new(
        contracts::auth_runtime::UserId::from(uuid::Uuid::nil()),
        contracts::auth_runtime::SubjectKind::User,
    );

    let response = executor.execute(&request, &auth).await.unwrap();

    assert_eq!(response.query_type, "brave_llm_context");
    assert!(!response.results.is_empty());
    assert!(response.results.iter().all(|result| !result.url.is_empty()));
}

#[tokio::test]
#[ignore = "requires live DeepSeek credentials (SEARCH_DEEPSEEK_API_KEY or AGENT_LLM_API_KEY)"]
async fn deepseek_web_live_smoke_returns_sources() {
    let api_key = std::env::var("SEARCH_DEEPSEEK_API_KEY")
        .or_else(|_| std::env::var("AGENT_LLM_API_KEY"))
        .unwrap_or_default();
    if api_key.trim().is_empty() {
        return;
    }
    let base_url = std::env::var("SEARCH_DEEPSEEK_BASE_URL")
        .or_else(|_| std::env::var("AGENT_LLM_BASE_URL"))
        .unwrap_or_else(|_| "https://api.deepseek.com".to_string());
    let model = std::env::var("SEARCH_DEEPSEEK_MODEL")
        .or_else(|_| std::env::var("AGENT_LLM_MODEL"))
        .unwrap_or_else(|_| "deepseek-v4-flash".to_string());

    let executor = SearchExecutor::new(SearchConfig {
        provider: "deepseek_web".to_string(),
        deepseek_base_url: base_url,
        deepseek_api_key: api_key,
        deepseek_model: model,
        max_results: 5,
        timeout_ms: 60_000,
        ..SearchConfig::default()
    });

    let response = executor
        .execute_search("What is the capital of France?", None)
        .await
        .expect("deepseek web search should succeed");

    assert_eq!(response.query_type, "deepseek_web");
    assert!(
        !response.results.is_empty(),
        "expected at least one source; answer={}",
        response.synthesized_answer
    );
    assert!(response.results.iter().all(|r| !r.url.is_empty()));
}
