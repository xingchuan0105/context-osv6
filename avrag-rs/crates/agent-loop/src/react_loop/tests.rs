#[test]
fn react_loop_splits_thinking_retrieve_off_synthesis_on() {
    use std::sync::Arc;

    use agent_tools::capability::CapabilityRegistry;
    use avrag_llm::{LlmClient, ModelProviderConfig};

    use super::ReActLoop;
    use super::config;

    let base = LlmClient::new(ModelProviderConfig {
        base_url: "https://api.deepseek.com".to_string(),
        api_key: "test".to_string(),
        model: "deepseek-v4-flash".to_string(),
        timeout_ms: 1000,
        api_style: None,
        dimensions: None,
        // Inbound default is irrelevant: ReActLoop forces phase policy.
        enable_thinking: Some(true),
        enable_cache: None,
        rpm_limit: None,
        tpm_limit: None,
    });
    let loop_ = ReActLoop::new(Arc::new(base), Arc::new(CapabilityRegistry::standard()));

    assert_eq!(loop_.llm.config.enable_thinking, Some(false));
    assert_eq!(loop_.synthesis_llm.config.enable_thinking, Some(true));

    let mut rag = config::load_mode_config("rag").expect("rag mode");
    rag.loop_exit.forbid_retrieve_direct_answer = true;
    assert_eq!(
        loop_.llm_for_retrieve(&rag).config.enable_thinking,
        Some(false),
        "three-loop retrieve must disable thinking"
    );

    let mut chat = config::load_mode_config("chat").expect("chat mode");
    chat.loop_exit.forbid_retrieve_direct_answer = false;
    assert_eq!(
        loop_.llm_for_retrieve(&chat).config.enable_thinking,
        Some(true),
        "chat DirectAnswer path keeps thinking max"
    );
}

#[test]
fn fallback_dense_args_roundtrips() {
    let args = serde_json::to_value(contracts::DenseRetrievalArgs {
        queries: vec!["rust".to_string()],
        query: None,
        modality: contracts::DenseRetrievalModality::Text,
        top_k: 10,
        doc_scope: vec!["doc1".to_string()],
    })
    .unwrap();
    let round: contracts::DenseRetrievalArgs = serde_json::from_value(args).unwrap();
    assert_eq!(round.queries, vec!["rust"]);
    assert_eq!(round.top_k, 10);
}

#[test]
fn fallback_lexical_args_roundtrips() {
    let args = serde_json::to_value(contracts::LexicalRetrievalArgs {
        terms: vec!["rust".to_string(), "lang".to_string()],
        top_k: 10,
        doc_scope: vec!["doc1".to_string()],
    })
    .unwrap();
    let round: contracts::LexicalRetrievalArgs = serde_json::from_value(args).unwrap();
    assert_eq!(round.terms, vec!["rust", "lang"]);
    assert_eq!(round.top_k, 10);
}

#[test]
fn fallback_graph_args_roundtrips() {
    let args = serde_json::to_value(contracts::GraphRetrievalArgs {
        graph_hints: Vec::new(),
        placeholder_triplets: Vec::new(),
        relation_limit: 20,
        supporting_chunk_limit: 10,
        hop_limit: 1,
        fan_out_limit: 10,
        query: Some("rust".to_string()),
        doc_scope: vec!["doc1".to_string()],
    })
    .unwrap();
    let round: contracts::GraphRetrievalArgs = serde_json::from_value(args).unwrap();
    assert_eq!(round.query.as_deref(), Some("rust"));
    assert_eq!(round.hop_limit, 1);
}
