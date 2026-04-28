use std::time::Duration;

use avrag_auth::{AuthContext, OrgId, SubjectKind};
use avrag_retrieval_data_plane::{
    Bm25SearchRequest, DocumentIndexBatch, EntityIndexRecord, GraphPassageIndexRecord,
    GraphRelationHint, GraphSearchRequest, MultimodalChunkIndexRecord, MultimodalSearchRequest,
    RelationIndexRecord, RetrievalDataPlane, TextChunkIndexRecord, TextDenseSearchRequest,
};
use avrag_storage_milvus::{MilvusConfig, MilvusDataPlane};
use tokio::time::sleep;
use uuid::Uuid;

fn integration_config() -> Option<MilvusConfig> {
    if std::env::var("MILVUS_INTEGRATION_TEST").ok().as_deref() != Some("1") {
        return None;
    }

    let suffix = Uuid::new_v4().simple().to_string();
    Some(MilvusConfig {
        url: std::env::var("MILVUS_URL").unwrap_or_else(|_| "http://127.0.0.1:19530".to_string()),
        token: std::env::var("MILVUS_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty()),
        database: std::env::var("MILVUS_DATABASE")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| Some("default".to_string())),
        collection_prefix: std::env::var("MILVUS_TEST_COLLECTION_PREFIX")
            .unwrap_or_else(|_| format!("avrag_test_{suffix}")),
        text_vector_dim: 4,
        multimodal_vector_dim: 4,
        metric_type: "COSINE".to_string(),
    })
}

#[tokio::test]
async fn configured_milvus_adapter_can_index_search_and_delete() -> anyhow::Result<()> {
    let Some(config) = integration_config() else {
        eprintln!("skipping Milvus integration test; set MILVUS_INTEGRATION_TEST=1");
        return Ok(());
    };

    let data_plane = MilvusDataPlane::new(config);
    let org_id = OrgId::new(Uuid::new_v4());
    let auth = AuthContext::new(org_id, SubjectKind::System);
    let document_id = Uuid::new_v4();
    let parse_run_id = Uuid::new_v4();
    let chunk_id = Uuid::new_v4();
    let multimodal_chunk_id = Uuid::new_v4();
    let asset_id = Uuid::new_v4();
    let relation_id = Uuid::new_v4();

    let report = data_plane
        .replace_document_index(DocumentIndexBatch {
            org_id,
            workspace_id: None,
            document_id,
            parse_run_id,
            doc_version: 1,
            text_chunks: vec![TextChunkIndexRecord {
                chunk_id,
                content: "Atlas uses Milvus for graph retrieval evidence.".to_string(),
                vector: vec![0.1, 0.2, 0.3, 0.4],
                page: Some(1),
                chunk_type: "text".to_string(),
                parser_backend: Some("test".to_string()),
                source_locator: None,
            }],
            multimodal_chunks: vec![MultimodalChunkIndexRecord {
                chunk_id: multimodal_chunk_id,
                asset_id,
                context_text: "Diagram showing Atlas relation retrieval.".to_string(),
                caption: Some("Atlas relation diagram".to_string()),
                image_path: Some("s3://bucket/atlas.png".to_string()),
                vector: vec![0.1, 0.2, 0.3, 0.4],
                page: Some(2),
                chunk_type: "image_with_context".to_string(),
                parser_backend: Some("test".to_string()),
                source_locator: None,
            }],
            entities: vec![EntityIndexRecord {
                entity_id: Uuid::new_v4(),
                name: "Atlas".to_string(),
                normalized_name: "atlas".to_string(),
                entity_type: Some("system".to_string()),
                vector: vec![0.1, 0.2, 0.3, 0.4],
                supporting_chunk_ids: vec![chunk_id],
                metadata: None,
            }],
            relations: vec![RelationIndexRecord {
                relation_id,
                subject: "Atlas".to_string(),
                predicate: "uses".to_string(),
                object: "Milvus".to_string(),
                relation_text: "Atlas uses Milvus".to_string(),
                vector: vec![0.1, 0.2, 0.3, 0.4],
                supporting_chunk_ids: vec![chunk_id],
                metadata: None,
            }],
            graph_passages: vec![GraphPassageIndexRecord {
                passage_id: Uuid::new_v4(),
                chunk_id: Some(chunk_id),
                text: "Atlas uses Milvus for graph retrieval evidence.".to_string(),
                vector: vec![0.1, 0.2, 0.3, 0.4],
                relation_ids: vec![relation_id],
                metadata: None,
            }],
        })
        .await?;

    assert_eq!(report.text_chunk_count, 1);
    assert_eq!(report.multimodal_chunk_count, 1);
    assert_eq!(report.entity_count, 1);
    assert_eq!(report.relation_count, 1);
    assert_eq!(report.graph_passage_count, 1);

    let mut dense_hits = Vec::new();
    let mut bm25_hits = Vec::new();
    let mut multimodal_hits = Vec::new();
    for _ in 0..10 {
        dense_hits = data_plane
            .search_text_dense(TextDenseSearchRequest {
                auth: auth.clone(),
                query_vector: vec![0.1, 0.2, 0.3, 0.4],
                doc_ids: Some(vec![document_id]),
                limit: 5,
            })
            .await?;
        bm25_hits = data_plane
            .search_bm25(Bm25SearchRequest {
                auth: auth.clone(),
                query: "Milvus graph retrieval".to_string(),
                doc_ids: Some(vec![document_id]),
                limit: 5,
            })
            .await?
            .chunks;
        multimodal_hits = data_plane
            .search_multimodal(MultimodalSearchRequest {
                auth: auth.clone(),
                query_vector: vec![0.1, 0.2, 0.3, 0.4],
                doc_ids: Some(vec![document_id]),
                limit: 5,
            })
            .await?;

        if !dense_hits.is_empty() && !bm25_hits.is_empty() && !multimodal_hits.is_empty() {
            break;
        }
        sleep(Duration::from_millis(250)).await;
    }

    assert_eq!(dense_hits[0].chunk_id, chunk_id);
    assert_eq!(bm25_hits[0].chunk_id, chunk_id);
    assert_eq!(multimodal_hits[0].chunk_id, multimodal_chunk_id);

    data_plane.delete_document_index(&auth, document_id).await?;

    Ok(())
}

/// Build a minimal DocumentIndexBatch for a given parse_run with
/// distinguishable content so reindex tests can verify only the latest
/// parse_run is visible.
fn make_index_batch(
    org_id: OrgId,
    document_id: Uuid,
    parse_run_id: Uuid,
    chunk_id: Uuid,
    content: &str,
    relation_subject: &str,
) -> DocumentIndexBatch {
    let relation_id = Uuid::new_v4();
    let entity_id = Uuid::new_v4();
    let multimodal_chunk_id = Uuid::new_v4();
    let asset_id = Uuid::new_v4();
    DocumentIndexBatch {
        org_id,
        workspace_id: None,
        document_id,
        parse_run_id,
        doc_version: 1,
        text_chunks: vec![TextChunkIndexRecord {
            chunk_id,
            content: content.to_string(),
            vector: vec![0.1, 0.2, 0.3, 0.4],
            page: Some(1),
            chunk_type: "text".to_string(),
            parser_backend: Some("test".to_string()),
            source_locator: None,
        }],
        multimodal_chunks: vec![MultimodalChunkIndexRecord {
            chunk_id: multimodal_chunk_id,
            asset_id,
            context_text: format!("Image: {}", content),
            caption: Some("reindex test diagram".to_string()),
            image_path: Some("s3://bucket/reindex.png".to_string()),
            vector: vec![0.1, 0.2, 0.3, 0.4],
            page: Some(2),
            chunk_type: "image_with_context".to_string(),
            parser_backend: Some("test".to_string()),
            source_locator: None,
        }],
        entities: vec![EntityIndexRecord {
            entity_id,
            name: relation_subject.to_string(),
            normalized_name: relation_subject.to_lowercase(),
            entity_type: Some("test".to_string()),
            vector: vec![0.1, 0.2, 0.3, 0.4],
            supporting_chunk_ids: vec![chunk_id],
            metadata: None,
        }],
        relations: vec![RelationIndexRecord {
            relation_id,
            subject: relation_subject.to_string(),
            predicate: "uses".to_string(),
            object: "test subsystem".to_string(),
            relation_text: format!("{} uses test subsystem", relation_subject),
            vector: vec![0.1, 0.2, 0.3, 0.4],
            supporting_chunk_ids: vec![chunk_id],
            metadata: None,
        }],
        graph_passages: vec![GraphPassageIndexRecord {
            passage_id: Uuid::new_v4(),
            chunk_id: Some(chunk_id),
            text: content.to_string(),
            vector: vec![0.1, 0.2, 0.3, 0.4],
            relation_ids: vec![relation_id],
            metadata: None,
        }],
    }
}

#[tokio::test]
async fn reindex_replaces_old_parse_run_and_only_latest_is_visible() -> anyhow::Result<()> {
    let Some(config) = integration_config() else {
        eprintln!("skipping Milvus reindex integration test; set MILVUS_INTEGRATION_TEST=1");
        return Ok(());
    };

    let data_plane = MilvusDataPlane::new(config);
    let org_id = OrgId::new(Uuid::new_v4());
    let auth = AuthContext::new(org_id, SubjectKind::System);
    let document_id = Uuid::new_v4();

    // === Parse run A (old) ===
    let parse_run_a = Uuid::new_v4();
    let chunk_a = Uuid::new_v4();
    let batch_a = make_index_batch(
        org_id,
        document_id,
        parse_run_a,
        chunk_a,
        "chunk A — parse run A",
        "ServiceA",
    );
    data_plane.replace_document_index(batch_a).await?;

    // === Parse run B (new, replaces A) ===
    let parse_run_b = Uuid::new_v4();
    let chunk_b = Uuid::new_v4();
    let batch_b = make_index_batch(
        org_id,
        document_id,
        parse_run_b,
        chunk_b,
        "chunk B — parse run B",
        "ServiceB",
    );
    data_plane.replace_document_index(batch_b).await?;

    // Wait for eventual consistency, then query.
    let mut dense_hits = Vec::new();
    let mut bm25_hits = Vec::new();
    for _ in 0..10 {
        sleep(Duration::from_millis(250)).await;
        dense_hits = data_plane
            .search_text_dense(TextDenseSearchRequest {
                auth: auth.clone(),
                query_vector: vec![0.1, 0.2, 0.3, 0.4],
                doc_ids: Some(vec![document_id]),
                limit: 10,
            })
            .await?;
        bm25_hits = data_plane
            .search_bm25(Bm25SearchRequest {
                auth: auth.clone(),
                query: "parse run B".to_string(),
                doc_ids: Some(vec![document_id]),
                limit: 10,
            })
            .await?
            .chunks;

        if !dense_hits.is_empty() && !bm25_hits.is_empty() {
            break;
        }
    }

    assert!(!dense_hits.is_empty(), "dense search must find parse_run_B");
    assert!(!bm25_hits.is_empty(), "BM25 search must find parse_run_B");

    // Verify only parse_run_B chunk is visible, not parse_run_A.
    for hit in &dense_hits {
        assert_ne!(
            hit.chunk_id, chunk_a,
            "parse_run_A chunk must NOT appear after reindex"
        );
        assert_ne!(
            hit.parse_run_id,
            Some(parse_run_a),
            "parse_run_A must NOT appear after reindex"
        );
    }
    for hit in &bm25_hits {
        assert_ne!(
            hit.chunk_id, chunk_a,
            "parse_run_A chunk must NOT appear in BM25 after reindex"
        );
    }

    // Also verify graph search sees only ServiceB (not ServiceA).
    let graph_output = data_plane
        .search_graph(GraphSearchRequest {
            auth: auth.clone(),
            doc_ids: Some(vec![document_id]),
            entity_names: vec!["ServiceB".to_string()],
            relation_hints: vec![GraphRelationHint {
                subject: Some("ServiceB".to_string()),
                predicate: Some("uses".to_string()),
                object: None,
            }],
            relation_limit: 10,
            supporting_chunk_limit: 5,
        })
        .await?;

    // Graph search should find ServiceB relation.
    assert!(
        !graph_output.relation_paths.is_empty(),
        "graph search must find ServiceB relations after reindex"
    );
    for path in &graph_output.relation_paths {
        assert!(
            !path.subject.contains("ServiceA"),
            "ServiceA must NOT appear in graph search after reindex"
        );
    }

    // Clean up.
    data_plane.delete_document_index(&auth, document_id).await?;

    Ok(())
}
