//! Contract tests for retrieval result DTOs (post ExecutePlan removal).

use contracts::RetrievalBundle;

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

#[test]
fn placeholder_triplet_classify() {
    use contracts::PlaceholderTriplet;
    use contracts::PlaceholderTripletType;

    let resolved = PlaceholderTriplet {
        subject: "Atlas".into(),
        predicate: "uses".into(),
        object: "checklist".into(),
    };
    assert_eq!(resolved.classify(), PlaceholderTripletType::Resolved);

    let traceable = PlaceholderTriplet {
        subject: "Atlas".into(),
        predicate: "uses".into(),
        object: "?x".into(),
    };
    assert_eq!(traceable.classify(), PlaceholderTripletType::Traceable);

    let fuzzy = PlaceholderTriplet {
        subject: "?s".into(),
        predicate: "?p".into(),
        object: "?o".into(),
    };
    assert_eq!(fuzzy.classify(), PlaceholderTripletType::Fuzzy);
}
