use super::*;
use common::{RagPlan, RagPlanItem, RagTraceItem};

fn mock_trace_item(
    payload_kind: &str,
    query: Option<&str>,
    bm25_terms: &[&str],
    summary: Option<&str>,
    sources: Vec<&str>,
) -> RagTraceItem {
    RagTraceItem {
        priority: 0.8,
        payload_kind: payload_kind.to_string(),
        query: query.map(ToOwned::to_owned),
        bm25_terms: bm25_terms.iter().map(|v| (*v).to_string()).collect(),
        summary: summary.map(ToOwned::to_owned),
        recall_budget: 10,
        bm25_k: 5,
        dense_k: 5,
        rerank_budget: 5,
        source_count: sources.len(),
        source_ids: sources.into_iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn test_build_retrieval_index_query_item() {
    let plan = RagPlan {
        plan_version: "rag-item-v2".to_string(),
        plan_confidence: 0.8,
        clarify_needed: false,
        clarify_message: String::new(),
        items: vec![RagPlanItem {
            priority: 0.8,
            query: Some("What is Rust?".to_string()),
            bm25_terms: None,
            summary: None,
        }],
    };
    let item = mock_trace_item(
        "query",
        Some("What is Rust?"),
        &[],
        None,
        vec!["chunk-1", "chunk-2"],
    );
    let index = build_retrieval_index("What is Rust?", &Some(plan), &[item], 2);
    let parsed: serde_json::Value = serde_json::from_str(&index).unwrap();
    assert_eq!(parsed["retrieval_paths"][0]["payload_kind"], "query");
    assert_eq!(parsed["retrieval_paths"][0]["query"], "What is Rust?");
    assert_eq!(parsed["grounding"]["recalled_chunk_count"], 2);
}

#[test]
fn test_build_retrieval_index_bm25_item() {
    let plan = RagPlan {
        plan_version: "rag-item-v2".to_string(),
        plan_confidence: 0.9,
        clarify_needed: false,
        clarify_message: String::new(),
        items: vec![RagPlanItem {
            priority: 0.2,
            query: None,
            bm25_terms: Some(vec!["atlas".to_string(), "rollback".to_string()]),
            summary: None,
        }],
    };
    let item = mock_trace_item("bm25_terms", None, &["atlas", "rollback"], None, vec!["c1"]);
    let index = build_retrieval_index("atlas rollback", &Some(plan), &[item], 1);
    let parsed: serde_json::Value = serde_json::from_str(&index).unwrap();
    assert_eq!(parsed["retrieval_paths"][0]["payload_kind"], "bm25_terms");
    assert_eq!(
        parsed["retrieval_paths"][0]["bm25_terms"],
        serde_json::json!(["atlas", "rollback"])
    );
}

#[test]
fn test_build_retrieval_index_summary_item() {
    let plan = RagPlan {
        plan_version: "rag-item-v2".to_string(),
        plan_confidence: 0.7,
        clarify_needed: false,
        clarify_message: String::new(),
        items: vec![RagPlanItem {
            priority: 0.1,
            query: None,
            bm25_terms: None,
            summary: Some("related".to_string()),
        }],
    };
    let item = mock_trace_item("summary", None, &[], Some("related"), vec![]);
    let index = build_retrieval_index("Explain the rollout", &Some(plan), &[item], 0);
    let parsed: serde_json::Value = serde_json::from_str(&index).unwrap();
    assert_eq!(parsed["retrieval_paths"][0]["payload_kind"], "summary");
    assert_eq!(parsed["retrieval_paths"][0]["summary"], "related");
}

#[test]
fn test_build_retrieval_index_empty() {
    let index = build_retrieval_index("Ghost query", &None, &[], 0);
    let parsed: serde_json::Value = serde_json::from_str(&index).unwrap();
    assert_eq!(parsed["grounding"]["zero_recall"], true);
    assert_eq!(parsed["path_count"], 0);
}

#[test]
fn test_build_synthesis_request_includes_sections() {
    let request = build_synthesis_request(
        "What is Rust?",
        r#"{"grounding":{"zero_recall":false}}"#,
        r#"[{"chunk_id":"chunk-1","chunk_type":"text","text":"Rust is a systems language."}]"#,
    );
    assert!(request.contains("User Question:"));
    assert!(request.contains("Retrieval Index (JSON):"));
    assert!(request.contains(
        "Context Chunks (JSON array of objects with fields: chunk_id, doc_id, chunk_type, page, text, caption, image_url):"
    ));
    assert!(request.contains(
        "chunk_id, doc_id, chunk_type, page, text, caption, image_url"
    ));
}

#[test]
fn test_parse_synthesis_output_supports_structured_json() {
    let parsed = parse_synthesis_output(
        r#"{"answer_text":"Rust is a systems language.","cited_chunk_ids":["chunk-1","chunk-2"]}"#,
    );

    assert_eq!(parsed.answer_text, "Rust is a systems language.");
    assert_eq!(
        parsed.cited_chunk_ids,
        vec!["chunk-1".to_string(), "chunk-2".to_string()]
    );
}

#[test]
fn test_parse_synthesis_output_supports_block_schema() {
    let parsed = parse_synthesis_output(
        r#"{
            "answer_blocks": [
                {"type":"text","text":"Rust is a systems language.","citations":["chunk-1"]},
                {"type":"image","chunk_id":"chunk-img"},
                {"type":"text","text":"It emphasizes safety.","citations":["chunk-2","chunk-3"]}
            ],
            "cited_chunk_ids": ["chunk-1","chunk-2","chunk-3","chunk-img"]
        }"#,
    );

    assert_eq!(
        parsed.answer_text,
        "Rust is a systems language. [[1]]\n\n[[image:chunk-img]]\n\nIt emphasizes safety. [[3]] [[4]]"
    );
    assert_eq!(parsed.answer_blocks.len(), 3);
    assert_eq!(
        parsed.cited_chunk_ids,
        vec![
            "chunk-1".to_string(),
            "chunk-img".to_string(),
            "chunk-2".to_string(),
            "chunk-3".to_string()
        ]
    );
}

#[test]
fn test_parse_synthesis_output_falls_back_to_plain_text() {
    let parsed = parse_synthesis_output("plain answer");

    assert_eq!(parsed.answer_text, "plain answer");
    assert!(parsed.cited_chunk_ids.is_empty());
}
