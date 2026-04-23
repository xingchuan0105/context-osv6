use crate::{SearchConfig, SearchExecutor};

#[tokio::test]
async fn missing_provider_is_explicit_error() {
    let executor = SearchExecutor::new(SearchConfig {
        provider: "perplexity".to_string(),
        perplexity_api_key: None,
        ..SearchConfig::default()
    });
    let request = common::ChatRequest {
        query: "test".to_string(),
        notebook_id: None,
        session_id: None,
        agent_type: "search".to_string(),
        source_type: None,
        source_token: None,
        doc_scope: Vec::new(),
        messages: Vec::new(),
        stream: false,
    };
    let auth = avrag_auth::AuthContext::new(
        avrag_auth::OrgId::from(uuid::Uuid::nil()),
        avrag_auth::SubjectKind::User,
    );
    let error = executor.execute(&request, &auth).await.unwrap_err();
    assert!(error.to_string().contains("Perplexity API key not configured"));
}

#[tokio::test]
async fn unsupported_provider_is_explicit_error() {
    let executor = SearchExecutor::new(SearchConfig {
        provider: "exa".to_string(),
        perplexity_api_key: Some("test".to_string()),
    });
    let request = common::ChatRequest {
        query: "test".to_string(),
        notebook_id: None,
        session_id: None,
        agent_type: "search".to_string(),
        source_type: None,
        source_token: None,
        doc_scope: Vec::new(),
        messages: Vec::new(),
        stream: false,
    };
    let auth = avrag_auth::AuthContext::new(
        avrag_auth::OrgId::from(uuid::Uuid::nil()),
        avrag_auth::SubjectKind::User,
    );
    let error = executor.execute(&request, &auth).await.unwrap_err();
    assert!(error.to_string().contains("only perplexity agent is supported"));
}
