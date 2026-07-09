use contracts::chat::{RagPlan, RagPlanItem};
use contracts::{
    ExecutePlanRequest, ExecutePlanSummaryMode, RetrievalBundle,
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
fn execute_plan_request_compat_roundtrip_preserves_summary_mode() {
    let request = ExecutePlanRequest {
        plan_version: "rag-execute-v1".to_string(),
        doc_scope: vec!["doc-1".to_string(), "doc-2".to_string()],
        items: vec![
            contracts::ExecutePlanItem {
                priority: 0.7,
                query: Some("semantic lookup".to_string()),
                bm25_terms: None,
            },
            contracts::ExecutePlanItem {
                priority: 0.3,
                query: None,
                bm25_terms: Some(vec!["rollback".to_string(), "atlas".to_string()]),
            },
        ],
        summary_mode: ExecutePlanSummaryMode::All,
        budget: Some(contracts::ExecutePlanBudget {
            total_candidate_budget: Some(32),
            final_chunk_budget: Some(8),
            graph_hop_limit: None,
            graph_fan_out_limit: None,
        }),
        channel_budget: None,
        query_entities: Vec::new(),
        graph_hints: Vec::new(),
        placeholder_triplets: Vec::new(),
        trace: Some(contracts::ExecutePlanTrace {
            request_id: Some("req-123".to_string()),
            trace_id: None,
            origin: Some("unit-test".to_string()),
        }),
    };

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
fn execute_plan_request_accepts_v2_optional_retrieval_fields() {
    let request = serde_json::from_value::<ExecutePlanRequest>(serde_json::json!({
        "plan_version": "rag-execute-v1",
        "doc_scope": ["doc-1"],
        "items": [{ "priority": 1.0, "query": "semantic lookup" }],
        "channel_budget": {
            "text_dense": 12,
            "bm25": 8,
            "multimodal_dense": 4,
            "graph": 6
        },
        "query_entities": [
            { "text": "Atlas", "kind": "project" }
        ],
        "graph_hints": [
            { "subject": "Atlas", "predicate": "uses", "object": "rollback checklist" }
        ],
        "placeholder_triplets": [
            { "subject": "Atlas", "predicate": "uses", "object": "?checklist" }
        ],
        "trace": {
            "request_id": "req-123",
            "trace_id": "trace-456",
            "origin": "unit-test"
        }
    }))
    .unwrap();

    assert_eq!(
        request
            .channel_budget
            .as_ref()
            .and_then(|budget| budget.graph),
        Some(6)
    );
    assert_eq!(request.query_entities[0].text, "Atlas");
    assert_eq!(request.graph_hints[0].predicate.as_deref(), Some("uses"));
    assert_eq!(request.placeholder_triplets[0].object, "?checklist");
    assert_eq!(
        request
            .trace
            .as_ref()
            .and_then(|trace| trace.trace_id.as_deref()),
        Some("trace-456")
    );
}

#[test]
fn retrieval_bundle_exposes_answer_context_in_retrieval_then_summary_order() {
    let bundle = RetrievalBundle {
        chunks: vec![contracts::RetrievedChunk {
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
            parse_run_id: None,
            score_breakdown: Vec::new(),
        }],
        graph_supported_chunks: vec![contracts::RetrievedChunk {
            chunk_id: "graph-chunk-1".to_string(),
            doc_id: "doc-1".to_string(),
            chunk_type: "text".to_string(),
            page: Some(2),
            text: "graph supported".to_string(),
            score: 0.8,
            retrieval_channel: "graph".to_string(),
            asset_id: None,
            caption: None,
            image_url: None,
            parser_backend: None,
            source_locator: None,
            parse_run_id: None,
            score_breakdown: Vec::new(),
        }],
        relation_paths: vec![contracts::RelationPath {
            path_id: "path-1".to_string(),
            entities: vec!["Atlas".to_string()],
            relations: vec!["uses".to_string()],
            supporting_chunk_ids: vec!["graph-chunk-1".to_string()],
            score: 0.8,
        }],
        citations: Vec::new(),
        summary_chunks: vec![contracts::AnswerContextChunk {
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

    assert_eq!(answer_context.len(), 3);
    assert_eq!(answer_context[0].chunk_type, "text");
    assert_eq!(answer_context[1].chunk_id, "graph-chunk-1");
    assert_eq!(answer_context[2].chunk_type, "summary");
}

#[test]
fn retrieval_bundle_relation_paths_roundtrip() {
    let bundle = serde_json::from_value::<RetrievalBundle>(serde_json::json!({
        "chunks": [],
        "graph_supported_chunks": [],
        "relation_paths": [{
            "path_id": "path-1",
            "entities": ["Atlas", "rollback checklist"],
            "relations": ["uses"],
            "supporting_chunk_ids": ["chunk-1"],
            "score": 0.8
        }],
        "citations": [],
        "summary_chunks": []
    }))
    .unwrap();

    assert_eq!(bundle.relation_paths.len(), 1);
    assert_eq!(bundle.relation_paths[0].relations, vec!["uses"]);
}

#[test]
fn retrieval_bundle_citation_chunks_includes_graph_supported_chunks() {
    let bundle = RetrievalBundle {
        chunks: vec![],
        graph_supported_chunks: vec![contracts::RetrievedChunk {
            chunk_id: "graph-chunk-1".to_string(),
            doc_id: "doc-1".to_string(),
            chunk_type: "graph_relation".to_string(),
            page: None,
            text: "Atlas uses checklist".to_string(),
            score: 0.8,
            retrieval_channel: "graph".to_string(),
            asset_id: None,
            caption: None,
            image_url: None,
            parser_backend: None,
            source_locator: None,
            parse_run_id: None,
            score_breakdown: Vec::new(),
        }],
        relation_paths: vec![],
        citations: vec![contracts::chat::Citation {
            citation_id: 1,
            doc_id: "doc-1".to_string(),
            chunk_id: Some("graph-chunk-1".to_string()),
            page: None,
            doc_name: "Doc 1".to_string(),
            preview: Some("Atlas uses checklist".to_string()),
            content: Some("Atlas uses checklist".to_string()),
            score: 0.8,
            layer: Some("graph".to_string()),
            chunk_type: Some("graph_relation".to_string()),
            asset_id: None,
            caption: None,
            image_url: None,
            parser_backend: None,
            source_locator: None,
            parse_run_id: None,
        }],
        summary_chunks: vec![],
    };

    let chunks = bundle.citation_chunks();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].chunk_id, "graph-chunk-1");
    assert!(bundle.has_evidence());
}

#[test]
fn retrieval_bundle_has_evidence_with_summary_chunks_only() {
    let bundle = RetrievalBundle {
        chunks: vec![],
        graph_supported_chunks: vec![],
        relation_paths: vec![],
        citations: vec![],
        summary_chunks: vec![contracts::AnswerContextChunk {
            chunk_id: "summary-doc-1".to_string(),
            doc_id: Some("doc-1".to_string()),
            chunk_type: "summary".to_string(),
            page: None,
            text: "summary".to_string(),
            asset_id: None,
            caption: None,
            image_url: None,
            parser_backend: None,
            source_locator: None,
        }],
    };

    assert!(bundle.has_evidence());
}

#[test]
fn retrieval_bundle_citation_chunks_dedupes_regular_and_graph() {
    let bundle = RetrievalBundle {
        chunks: vec![contracts::RetrievedChunk {
            chunk_id: "chunk-1".to_string(),
            doc_id: "doc-1".to_string(),
            chunk_type: "text".to_string(),
            page: Some(1),
            text: "regular".to_string(),
            score: 0.9,
            retrieval_channel: "dense".to_string(),
            asset_id: None,
            caption: None,
            image_url: None,
            parser_backend: None,
            source_locator: None,
            parse_run_id: None,
            score_breakdown: Vec::new(),
        }],
        graph_supported_chunks: vec![contracts::RetrievedChunk {
            chunk_id: "chunk-1".to_string(),
            doc_id: "doc-1".to_string(),
            chunk_type: "text".to_string(),
            page: Some(1),
            text: "regular".to_string(),
            score: 0.9,
            retrieval_channel: "graph".to_string(),
            asset_id: None,
            caption: None,
            image_url: None,
            parser_backend: None,
            source_locator: None,
            parse_run_id: None,
            score_breakdown: Vec::new(),
        }],
        relation_paths: vec![],
        citations: vec![],
        summary_chunks: vec![],
    };

    let chunks = bundle.citation_chunks();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].retrieval_channel, "dense");
}
