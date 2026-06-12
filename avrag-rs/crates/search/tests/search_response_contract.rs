use avrag_search::{SearchResponse, SearchResult};

#[test]
fn search_response_roundtrip_preserves_optional_citation_index() {
    let response = SearchResponse {
        query_type: "single".to_string(),
        sub_queries: vec!["rust async runtime".to_string()],
        results: vec![SearchResult {
            title: "Tokio".to_string(),
            url: "https://tokio.rs".to_string(),
            snippet: "An async runtime for Rust".to_string(),
            citation_index: Some(1),
        }],
        synthesized_answer: "Tokio is the async runtime.".to_string(),
        llm_usage: None,
    };

    let encoded = serde_json::to_value(&response).unwrap();
    assert_eq!(encoded["results"][0]["citation_index"], 1);

    let decoded: SearchResponse = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded.results[0].citation_index, Some(1));
    assert_eq!(decoded.query_type, "single");
}

#[test]
fn search_response_omits_citation_index_when_unset() {
    let response = SearchResponse {
        query_type: "single".to_string(),
        sub_queries: Vec::new(),
        results: vec![SearchResult {
            title: "Example".to_string(),
            url: "https://example.com".to_string(),
            snippet: "snippet".to_string(),
            citation_index: None,
        }],
        synthesized_answer: String::new(),
        llm_usage: None,
    };

    let encoded = serde_json::to_value(&response).unwrap();
    assert!(encoded["results"][0].get("citation_index").is_none());
}
