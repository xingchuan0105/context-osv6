use crate::{SearchConfig, SearchExecutor};

#[test]
fn default_provider_is_brave_llm_context() {
    assert_eq!(SearchConfig::default().provider, "brave_llm_context");
}

#[tokio::test]
async fn missing_brave_key_is_explicit_error() {
    let executor = SearchExecutor::new(SearchConfig::default());
    let request = contracts::chat::ChatRequest {
        query: "test".to_string(),
        notebook_id: None,
        session_id: None,
        agent_type: "search".to_string(),
        source_type: None,
        source_token: None,
        doc_scope: Vec::new(),
        messages: Vec::new(),
        stream: false,
        debug: false,
        language: None,
        format_hint: None,
    };
    let auth = avrag_auth::AuthContext::new(
        avrag_auth::OrgId::from(uuid::Uuid::nil()),
        avrag_auth::SubjectKind::User,
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
        notebook_id: None,
        session_id: None,
        agent_type: "search".to_string(),
        source_type: None,
        source_token: None,
        doc_scope: Vec::new(),
        messages: Vec::new(),
        stream: false,
        debug: false,
        language: None,
        format_hint: None,
    };
    let auth = avrag_auth::AuthContext::new(
        avrag_auth::OrgId::from(uuid::Uuid::nil()),
        avrag_auth::SubjectKind::User,
    );
    let error = executor.execute(&request, &auth).await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("supported providers: brave_llm_context")
    );
}

#[tokio::test]
#[ignore = "requires external network connectivity to Brave Search API"]
async fn executor_routes_news_vertical_to_news_endpoint() {
    let executor = SearchExecutor::new(SearchConfig {
        api_key: "dummy".to_string(),
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
        api_key,
        max_results: 3,
        ..SearchConfig::default()
    });
    let request = contracts::chat::ChatRequest {
        query: "What is the Brave Search LLM Context API?".to_string(),
        notebook_id: None,
        session_id: None,
        agent_type: "search".to_string(),
        source_type: None,
        source_token: None,
        doc_scope: Vec::new(),
        messages: Vec::new(),
        stream: false,
        debug: false,
        language: None,
        format_hint: None,
    };
    let auth = avrag_auth::AuthContext::new(
        avrag_auth::OrgId::from(uuid::Uuid::nil()),
        avrag_auth::SubjectKind::User,
    );

    let response = executor.execute(&request, &auth).await.unwrap();

    assert_eq!(response.query_type, "brave_llm_context");
    assert!(!response.results.is_empty());
    assert!(response.results.iter().all(|result| !result.url.is_empty()));
}
