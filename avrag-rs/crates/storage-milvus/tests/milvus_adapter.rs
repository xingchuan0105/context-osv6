use std::time::Duration;

use avrag_auth::{AuthContext, OrgId, SubjectKind};
use avrag_retrieval_data_plane::{
    Bm25SearchRequest, DocumentIndexBatch, EntityIndexRecord, GraphPassageIndexRecord,
    MultimodalChunkIndexRecord, MultimodalSearchRequest, RelationIndexRecord, RetrievalDataPlane,
    TextChunkIndexRecord, TextDenseSearchRequest,
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
