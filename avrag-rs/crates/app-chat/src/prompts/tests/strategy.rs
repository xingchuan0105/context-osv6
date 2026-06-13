use super::super::*;
use avrag_search::SearchResult;

#[test]
fn build_rag_strategy_evaluation_prompt_contains_all_inputs() {
    let tool_results = vec![
        contracts::ToolResult {
            tool: "dense_retrieval".to_string(),
            version: "1.0".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!([
                {"chunk_id": "c1", "text": "alpha"},
                {"chunk_id": "c2", "text": "beta"},
            ])),
            trace: None,
        },
        contracts::ToolResult {
            tool: "lexical_retrieval".to_string(),
            version: "1.0".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!([])),
            trace: None,
        },
        contracts::ToolResult {
            tool: "graph_retrieval".to_string(),
            version: "1.0".to_string(),
            status: contracts::ToolStatus::Error,
            data: None,
            trace: None,
        },
    ];

    let sub_queries = vec![
        SubQueryItem {
            id: "q1".to_string(),
            text: "rust async runtime".to_string(),
            tool_index: 0,
        },
        SubQueryItem {
            id: "q2".to_string(),
            text: "BM25: async, runtime, tokio".to_string(),
            tool_index: 1,
        },
    ];

    let chunks = vec![
        contracts::RetrievedChunk {
            chunk_id: "c1".to_string(),
            doc_id: "doc1".to_string(),
            chunk_type: "paragraph".to_string(),
            page: None,
            text: "alpha text".to_string(),
            score: 0.95,
            retrieval_channel: "dense".to_string(),
            asset_id: None,
            caption: None,
            image_url: None,
            parser_backend: None,
            source_locator: None,
            parse_run_id: None,
            score_breakdown: vec![],
        },
        contracts::RetrievedChunk {
            chunk_id: "c2".to_string(),
            doc_id: "doc1".to_string(),
            chunk_type: "paragraph".to_string(),
            page: None,
            text: "beta text".to_string(),
            score: 0.85,
            retrieval_channel: "dense".to_string(),
            asset_id: None,
            caption: None,
            image_url: None,
            parser_backend: None,
            source_locator: None,
            parse_run_id: None,
            score_breakdown: vec![],
        },
    ];

    let prompt = build_rag_strategy_evaluation_prompt(
        "How does async runtime work in Rust?",
        &sub_queries,
        &tool_results,
        &chunks,
        1,
        15,
    );

    assert!(prompt.contains("How does async runtime work in Rust?"));
    assert!(prompt.contains("iteration 2"));
    assert!(prompt.contains("- q1: \"rust async runtime\" -> 2 results"));
    assert!(prompt.contains("- q2: \"BM25: async, runtime, tokio\" -> 0 results"));
    assert!(prompt.contains("Additional tool calls:"));
    assert!(prompt.contains("tool=graph_retrieval -> Error"));
    assert!(prompt.contains("Retrieved chunks (2)"));
    assert!(prompt.contains("alpha text"));
}

#[test]
fn build_rag_strategy_evaluation_prompt_maps_multi_query_tool_correctly() {
    // One dense_retrieval call with 2 queries → both map to tool_index 0
    let tool_results = vec![contracts::ToolResult {
        tool: "dense_retrieval".to_string(),
        version: "1.0".to_string(),
        status: contracts::ToolStatus::Ok,
        data: Some(serde_json::json!([
            {"chunk_id": "c1", "text": "alpha"},
            {"chunk_id": "c2", "text": "beta"},
            {"chunk_id": "c3", "text": "gamma"},
        ])),
        trace: None,
    }];

    let sub_queries = vec![
        SubQueryItem {
            id: "q1".to_string(),
            text: "query A".to_string(),
            tool_index: 0,
        },
        SubQueryItem {
            id: "q2".to_string(),
            text: "query B".to_string(),
            tool_index: 0,
        },
    ];

    let chunks: Vec<contracts::RetrievedChunk> = vec![];

    let prompt = build_rag_strategy_evaluation_prompt(
        "test",
        &sub_queries,
        &tool_results,
        &chunks,
        0,
        15,
    );

    // Both q1 and q2 should report 3 results (from the same tool_result at index 0)
    assert!(prompt.contains("- q1: \"query A\" -> 3 results"));
    assert!(prompt.contains("- q2: \"query B\" -> 3 results"));
    assert!(!prompt.contains("Additional tool calls"));
}

#[test]
fn parse_rag_strategy_evaluation_parses_valid_json() {
    let raw = r#"{"dimensions": [{"name": "async runtime", "attempted": true, "covered": true, "retrieved_count": 3, "query_ids": ["q1"], "status": "covered_strong"}], "missing_dimensions": ["memory model"], "weak_dimensions": [], "recommendation": "replan", "reason": "missing memory model dimension", "suggested_followup_queries": ["memory model async rust"], "decision": "insufficient"}"#;
    let eval = parse_rag_strategy_evaluation(raw).unwrap();
    assert_eq!(eval.dimensions.len(), 1);
    assert_eq!(eval.dimensions[0].name, "async runtime");
    assert!(eval.dimensions[0].attempted);
    assert!(eval.dimensions[0].covered);
    assert_eq!(eval.dimensions[0].retrieved_count, 3);
    assert_eq!(eval.dimensions[0].query_ids, vec!["q1"]);
    assert!(matches!(
        eval.dimensions[0].status,
        DimensionStatus::CoveredStrong
    ));
    assert_eq!(eval.missing_dimensions, vec!["memory model"]);
    assert!(eval.weak_dimensions.is_empty());
    assert!(matches!(
        eval.recommendation,
        Some(StrategyRecommendation::Replan)
    ));
    assert_eq!(
        eval.reason.as_deref(),
        Some("missing memory model dimension")
    );
    assert_eq!(
        eval.suggested_followup_queries,
        vec!["memory model async rust"]
    );
}

#[test]
fn parse_rag_strategy_evaluation_parses_snake_case_recommendations() {
    let synthesize = r#"{"recommendation": "synthesize", "reason": "done", "status": "covered_strong", "decision": "sufficient"}"#;
    let replan = r#"{"recommendation": "replan", "reason": "missing", "status": "missing", "decision": "insufficient"}"#;
    let broaden = r#"{"recommendation": "broaden", "reason": "too few", "status": "covered_weak", "decision": "insufficient"}"#;

    assert!(matches!(
        parse_rag_strategy_evaluation(synthesize)
            .unwrap()
            .recommendation,
        Some(StrategyRecommendation::Synthesize)
    ));
    assert!(matches!(
        parse_rag_strategy_evaluation(replan)
            .unwrap()
            .recommendation,
        Some(StrategyRecommendation::Replan)
    ));
    assert!(matches!(
        parse_rag_strategy_evaluation(broaden)
            .unwrap()
            .recommendation,
        Some(StrategyRecommendation::Broaden)
    ));
}

#[test]
fn parse_rag_strategy_evaluation_parses_all_dimension_statuses() {
    let raw = r#"{"dimensions": [
        {"name": "a", "status": "covered_strong"},
        {"name": "b", "status": "covered_weak"},
        {"name": "c", "status": "missing"}
    ], "recommendation": "synthesize", "reason": "ok", "decision": "sufficient"}"#;
    let eval = parse_rag_strategy_evaluation(raw).unwrap();
    assert!(matches!(
        eval.dimensions[0].status,
        DimensionStatus::CoveredStrong
    ));
    assert!(matches!(
        eval.dimensions[1].status,
        DimensionStatus::CoveredWeak
    ));
    assert!(matches!(
        eval.dimensions[2].status,
        DimensionStatus::Missing
    ));
}

#[test]
fn parse_rag_strategy_evaluation_handles_json_wrapped_in_markdown() {
    let raw = r#"Here is my evaluation:
```json
{"dimensions": [], "missing_dimensions": [], "weak_dimensions": [], "recommendation": "synthesize", "reason": "complete", "suggested_followup_queries": [], "decision": "sufficient"}
```"#;
    let eval = parse_rag_strategy_evaluation(raw).unwrap();
    assert!(matches!(
        eval.recommendation,
        Some(StrategyRecommendation::Synthesize)
    ));
    assert_eq!(eval.reason.as_deref(), Some("complete"));
}

#[test]
fn parse_rag_strategy_evaluation_returns_none_for_invalid_json() {
    let raw = "this is not json at all";
    assert!(parse_rag_strategy_evaluation(raw).is_none());
}

#[test]
fn parse_rag_strategy_evaluation_uses_defaults_for_optional_fields() {
    let raw = r#"{"decision": "sufficient", "recommendation": "synthesize", "reason": "ok"}"#;
    let eval = parse_rag_strategy_evaluation(raw).unwrap();
    assert!(eval.dimensions.is_empty());
    assert!(eval.missing_dimensions.is_empty());
    assert!(eval.weak_dimensions.is_empty());
    assert!(eval.suggested_followup_queries.is_empty());
}

// ---------------- search strategy evaluation prompt / parser ----------------

#[test]
fn build_search_strategy_evaluation_prompt_contains_all_inputs() {
    let results = vec![
        SearchResult {
            title: "Result 1".to_string(),
            url: "https://example.com/1".to_string(),
            snippet: "Description 1".to_string(),
            citation_index: None,
        },
        SearchResult {
            title: "Result 2".to_string(),
            url: "https://example.com/2".to_string(),
            snippet: "Description 2".to_string(),
            citation_index: None,
        },
    ];

    let prompt = build_search_strategy_evaluation_prompt(
        "What is Rust?",
        Some("web"),
        &["q1".to_string(), "q2".to_string()],
        &results,
        5,
        0,
        15,
    );

    assert!(prompt.contains("What is Rust?"));
    assert!(prompt.contains("q1"));
    assert!(prompt.contains("Result 1"));
    assert!(prompt.contains("https://example.com/1"));
    assert!(prompt.contains("Actual results (2)"));
}

#[test]
fn build_search_strategy_evaluation_prompt_omits_vertical_when_none() {
    let results = vec![SearchResult {
        title: "Test".to_string(),
        url: "https://test.com".to_string(),
        snippet: "Test snippet".to_string(),
        citation_index: None,
    }];

    let prompt = build_search_strategy_evaluation_prompt(
        "test",
        None,
        &["query".to_string()],
        &results,
        0,
        0,
        15,
    );

    assert!(prompt.contains("test"));
    assert!(!prompt.contains("Vertical used:"));
}

#[test]
fn parse_search_strategy_evaluation_parses_valid_json() {
    let raw = r#"{"dimensions": [{"name": "latest news", "attempted": true, "covered": true, "retrieved_count": 5, "query_ids": ["q1"], "status": "covered_strong"}], "missing_dimensions": ["opinions"], "weak_dimensions": [], "recommendation": "escalate_vertical", "reason": "need discussions vertical", "suggested_followup_queries": ["rust async opinions reddit"], "decision": "insufficient"}"#;
    let eval = parse_search_strategy_evaluation(raw).unwrap();
    assert_eq!(eval.dimensions.len(), 1);
    assert_eq!(eval.dimensions[0].name, "latest news");
    assert!(eval.dimensions[0].attempted);
    assert!(matches!(
        eval.dimensions[0].status,
        DimensionStatus::CoveredStrong
    ));
    assert_eq!(eval.missing_dimensions, vec!["opinions"]);
    assert!(eval.weak_dimensions.is_empty());
    assert!(matches!(
        eval.recommendation,
        Some(SearchStrategyRecommendation::EscalateVertical)
    ));
    assert_eq!(eval.reason.as_deref(), Some("need discussions vertical"));
    assert_eq!(
        eval.suggested_followup_queries,
        vec!["rust async opinions reddit"]
    );
}

#[test]
fn parse_search_strategy_evaluation_parses_all_recommendations() {
    let synthesize =
        r#"{"recommendation": "synthesize", "reason": "done", "decision": "sufficient"}"#;
    let broaden =
        r#"{"recommendation": "broaden", "reason": "too few", "decision": "insufficient"}"#;
    let escalate = r#"{"recommendation": "escalate_vertical", "reason": "need news", "decision": "insufficient"}"#;

    assert!(matches!(
        parse_search_strategy_evaluation(synthesize)
            .unwrap()
            .recommendation,
        Some(SearchStrategyRecommendation::Synthesize)
    ));
    assert!(matches!(
        parse_search_strategy_evaluation(broaden)
            .unwrap()
            .recommendation,
        Some(SearchStrategyRecommendation::Broaden)
    ));
    assert!(matches!(
        parse_search_strategy_evaluation(escalate)
            .unwrap()
            .recommendation,
        Some(SearchStrategyRecommendation::EscalateVertical)
    ));
}

#[test]
fn parse_search_strategy_evaluation_returns_none_for_invalid_json() {
    assert!(parse_search_strategy_evaluation("not json").is_none());
}

// ---------------- EvalDecision / NextAction serialization ----------------

#[test]
fn eval_decision_serializes_snake_case() {
    let d = EvalDecision::Sufficient;
    assert_eq!(serde_json::to_string(&d).unwrap(), "\"sufficient\"");
    let d: EvalDecision = serde_json::from_str("\"give_up\"").unwrap();
    assert!(matches!(d, EvalDecision::GiveUp));
}

#[test]
fn next_action_sub_query_serializes() {
    let a = NextAction::SubQuery {
        query: "test query".to_string(),
    };
    let json = serde_json::to_value(&a).unwrap();
    assert_eq!(json["type"], "sub_query");
    assert_eq!(json["query"], "test query");
}

#[test]
fn next_action_tool_call_serializes() {
    let a = NextAction::ToolCall {
        tool: "graph_retrieval".to_string(),
        args: serde_json::json!({"query": "test"}),
        reason: "dense failed".to_string(),
    };
    let json = serde_json::to_value(&a).unwrap();
    assert_eq!(json["type"], "tool_call");
    assert_eq!(json["tool"], "graph_retrieval");
    assert_eq!(json["reason"], "dense failed");
}

#[test]
fn rag_strategy_evaluation_has_decision_and_next_actions() {
    let eval: RagStrategyEvaluation = serde_json::from_str(
        r#"{
        "decision": "insufficient",
        "next_actions": [
            {"type": "sub_query", "query": "new query"}
        ],
        "reasoning": "missing dimension",
        "dimensions": [],
        "missing_dimensions": [],
        "weak_dimensions": []
    }"#,
    )
    .unwrap();
    assert!(matches!(eval.decision, EvalDecision::Insufficient));
    assert_eq!(eval.next_actions.len(), 1);
    assert_eq!(eval.reasoning, "missing dimension");
}

#[test]
fn rag_strategy_evaluation_backwards_compat_with_recommendation() {
    let eval: RagStrategyEvaluation = serde_json::from_str(
        r#"{
        "decision": "sufficient",
        "next_actions": [],
        "reasoning": "all covered",
        "recommendation": "synthesize",
        "dimensions": [],
        "missing_dimensions": [],
        "weak_dimensions": []
    }"#,
    )
    .unwrap();
    assert!(matches!(
        eval.recommendation,
        Some(StrategyRecommendation::Synthesize)
    ));
}

#[test]
fn search_strategy_evaluation_has_decision_and_next_actions() {
    let eval: SearchStrategyEvaluation = serde_json::from_str(
        r#"{
        "decision": "insufficient",
        "next_actions": [
            {"type": "tool_call", "tool": "web_search", "args": {}, "reason": "try news"}
        ],
        "reasoning": "need vertical escalation"
    }"#,
    )
    .unwrap();
    assert!(matches!(eval.decision, EvalDecision::Insufficient));
    assert_eq!(eval.next_actions.len(), 1);
}
