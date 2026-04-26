use common::{
    ExecutePlanRequest, ExecutePlanSummaryMode, ExecutePlanValidationError, RagPlan, RagPlanItem,
    RetrievalBundle,
};

#[test]
fn execute_plan_request_drops_legacy_clarify_fields_when_mapped_from_rag_plan() {
    let legacy = RagPlan {
        plan_version: "rag-item-v2".to_string(),
        plan_confidence: 0.4,
        clarify_needed: true,
        clarify_message: "need more detail".to_string(),
        items: vec![
            RagPlanItem {
                priority: 0.8,
                query: Some("incident timeline".to_string()),
                bm25_terms: None,
                summary: None,
            },
            RagPlanItem {
                priority: 0.2,
                query: None,
                bm25_terms: None,
                summary: Some("related".to_string()),
            },
        ],
    };

    let request = ExecutePlanRequest::from_rag_plan(&legacy, &["doc-1".to_string()]);
    let encoded = serde_json::to_value(&request).unwrap();

    assert_eq!(request.plan_version, "rag-item-v2");
    assert_eq!(request.summary_mode, ExecutePlanSummaryMode::Related);
    assert_eq!(request.items.len(), 1);
    assert!(encoded.get("clarify_needed").is_none());
    assert!(encoded.get("clarify_message").is_none());
    assert!(encoded.get("session_id").is_none());
    assert!(encoded.get("history").is_none());
    assert!(encoded.get("messages").is_none());
}

#[test]
fn execute_plan_request_validation_rejects_ambiguous_items() {
    let request = ExecutePlanRequest {
        plan_version: "rag-execute-v1".to_string(),
        doc_scope: vec!["doc-1".to_string()],
        items: vec![common::ExecutePlanItem {
            priority: 0.5,
            query: Some("alpha".to_string()),
            bm25_terms: Some(vec!["beta".to_string()]),
        }],
        summary_mode: ExecutePlanSummaryMode::None,
        budget: None,
        trace: None,
    };

    let error = request.validate().unwrap_err();
    assert_eq!(
        error,
        ExecutePlanValidationError::InvalidPayloadCount { index: 0 }
    );
}

#[test]
fn execute_plan_request_validation_rejects_empty_doc_scope() {
    let request = ExecutePlanRequest {
        plan_version: "rag-execute-v1".to_string(),
        doc_scope: Vec::new(),
        items: vec![common::ExecutePlanItem {
            priority: 1.0,
            query: Some("alpha".to_string()),
            bm25_terms: None,
        }],
        summary_mode: ExecutePlanSummaryMode::None,
        budget: None,
        trace: None,
    };

    assert_eq!(
        request.validate().unwrap_err(),
        ExecutePlanValidationError::EmptyDocScope
    );
}

#[test]
fn execute_plan_request_deserialization_rejects_legacy_session_fields() {
    let error = serde_json::from_value::<ExecutePlanRequest>(serde_json::json!({
        "plan_version": "rag-execute-v1",
        "doc_scope": ["doc-1"],
        "items": [{ "priority": 1.0, "query": "alpha" }],
        "session_id": "session-1",
        "history": [],
        "clarify_needed": false
    }))
    .unwrap_err();

    assert!(error.to_string().contains("unknown field"));
}

#[test]
fn execute_plan_request_validation_rejects_more_than_four_items() {
    let request = ExecutePlanRequest {
        plan_version: "rag-execute-v1".to_string(),
        doc_scope: vec!["doc-1".to_string()],
        items: (0..5)
            .map(|index| common::ExecutePlanItem {
                priority: 0.5,
                query: Some(format!("query {index}")),
                bm25_terms: None,
            })
            .collect(),
        summary_mode: ExecutePlanSummaryMode::None,
        budget: None,
        trace: None,
    };

    assert_eq!(
        request.validate().unwrap_err(),
        ExecutePlanValidationError::TooManyItems { max: 4 }
    );
}

#[test]
fn execute_plan_request_compat_roundtrip_preserves_summary_mode() {
    let request = ExecutePlanRequest {
        plan_version: "rag-execute-v1".to_string(),
        doc_scope: vec!["doc-1".to_string(), "doc-2".to_string()],
        items: vec![
            common::ExecutePlanItem {
                priority: 0.7,
                query: Some("semantic lookup".to_string()),
                bm25_terms: None,
            },
            common::ExecutePlanItem {
                priority: 0.3,
                query: None,
                bm25_terms: Some(vec!["rollback".to_string(), "atlas".to_string()]),
            },
        ],
        summary_mode: ExecutePlanSummaryMode::All,
        budget: Some(common::ExecutePlanBudget {
            total_candidate_budget: Some(32),
            final_chunk_budget: Some(8),
        }),
        trace: Some(common::ExecutePlanTrace {
            request_id: Some("req-123".to_string()),
            origin: Some("unit-test".to_string()),
        }),
    };

    request.validate().unwrap();
    let compat_plan = request.to_rag_plan_compat();

    assert_eq!(compat_plan.items.len(), 3);
    assert_eq!(
        compat_plan
            .items
            .last()
            .and_then(|item| item.summary.as_deref()),
        Some("all")
    );

    let mapped_back = ExecutePlanRequest::from_rag_plan(&compat_plan, &request.doc_scope);
    assert_eq!(mapped_back.summary_mode, ExecutePlanSummaryMode::All);
    assert_eq!(mapped_back.items.len(), 2);
    assert_eq!(mapped_back.doc_scope, request.doc_scope);
}

#[test]
fn retrieval_bundle_exposes_answer_context_in_retrieval_then_summary_order() {
    let bundle = RetrievalBundle {
        chunks: vec![common::RetrievedChunk {
            chunk_id: "chunk-1".to_string(),
            doc_id: "doc-1".to_string(),
            chunk_type: "text".to_string(),
            page: Some(1),
            text: "retrieved".to_string(),
            score: 0.9,
            retrieval_channel: "dense".to_string(),
            asset_id: None,
            caption: None,
            image_url: None,
            parser_backend: None,
            source_locator: None,
        }],
        citations: Vec::new(),
        summary_chunks: vec![common::AnswerContextChunk {
            chunk_id: "summary-doc-1".to_string(),
            doc_id: Some("doc-1".to_string()),
            chunk_type: "summary".to_string(),
            page: None,
            text: "[Document Summary] summary".to_string(),
            asset_id: None,
            caption: None,
            image_url: None,
            parser_backend: None,
            source_locator: None,
        }],
    };

    let answer_context = bundle.answer_context_chunks();

    assert_eq!(answer_context.len(), 2);
    assert_eq!(answer_context[0].chunk_type, "text");
    assert_eq!(answer_context[1].chunk_type, "summary");
}
