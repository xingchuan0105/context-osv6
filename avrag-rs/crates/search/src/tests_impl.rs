use crate::{SearchConfig, SearchExecutor, planner};

#[test]
fn heuristic_plan_adds_freshness_query() {
    let plan = planner::plan_query_heuristically(
        "OpenAI latest model",
        &SearchConfig {
            mode: "provider".to_string(),
            provider: "exa".to_string(),
            api_key: "test".to_string(),
            planner_enabled: false,
            ..SearchConfig::default()
        },
    )
    .sub_queries;
    assert!(plan.iter().any(|item| item.contains("latest")));
}

#[tokio::test]
async fn missing_provider_is_explicit_error() {
    let executor = SearchExecutor::new(SearchConfig {
        mode: "provider".to_string(),
        provider: "exa".to_string(),
        api_key: String::new(),
        base_url: String::new(),
        perplexity_api_key: None,
        planner_enabled: false,
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
    assert!(error.to_string().contains("not configured"));
}
