#[test]
fn fallback_dense_args_roundtrips() {
    let args = serde_json::to_value(contracts::DenseRetrievalArgs {
        queries: vec!["rust".to_string()],
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
