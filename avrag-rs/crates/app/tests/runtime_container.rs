use app::runtime::Runtime;

#[tokio::test]
async fn runtime_new_memory_requires_configured_chat_agent() {
    let runtime = Runtime::new_memory().await.unwrap();

    assert_eq!(runtime.runtime_mode(), "memory");
    let error = runtime
        .services
        .chat
        .execute(contracts::chat::ChatRequest {
            query: "say hello".to_string(),
            notebook_id: None,
            session_id: None,
            agent_type: "general".to_string(),
            source_type: None,
            source_token: None,
            doc_scope: Vec::new(),
            messages: Vec::new(),
            stream: false,
        })
        .await
        .unwrap_err();

    assert_eq!(error.code(), "internal_error");
    assert!(error.message().contains("LLM client is not configured"));
}
