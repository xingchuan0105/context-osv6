use super::*;
use crate::config::MilvusConfig;
use crate::executor::tests::{Call, FakeExecutor};
use crate::lib_impl::MilvusDataPlane;
use avrag_retrieval_data_plane::{
    DocumentIndexBatch, MultimodalChunkIndexRecord, TextChunkIndexRecord,
};
use serde_json::json;
use uuid::Uuid;

fn test_config() -> MilvusConfig {
    MilvusConfig {
        url: "http://localhost:19530".to_string(),
        token: None,
        database: Some("test".to_string()),
        collection_prefix: "test".to_string(),
        text_vector_dim: 4,
        multimodal_vector_dim: 4,
        metric_type: "L2".to_string(),
    }
}

fn make_test_batch(doc_id: Uuid, parse_run_id: Uuid) -> DocumentIndexBatch {
    DocumentIndexBatch {
        org_id: avrag_auth::OrgId::new(Uuid::from_u128(1)),
        workspace_id: None,
        document_id: doc_id,
        parse_run_id,
        doc_version: 1,
        text_chunks: vec![TextChunkIndexRecord {
            chunk_id: Uuid::from_u128(100),
            content: "text chunk".to_string(),
            vector: vec![0.1, 0.2, 0.3, 0.4],
            page: Some(1),
            chunk_type: "text".to_string(),
            parser_backend: Some("test".to_string()),
            source_locator: None,
        }],
        multimodal_chunks: vec![MultimodalChunkIndexRecord {
            chunk_id: Uuid::from_u128(101),
            asset_id: Uuid::from_u128(102),
            context_text: "image chunk".to_string(),
            caption: Some("caption".to_string()),
            image_path: Some("s3://bucket/img.png".to_string()),
            vector: vec![0.1, 0.2, 0.3, 0.4],
            page: Some(2),
            chunk_type: "image_with_context".to_string(),
            parser_backend: Some("test".to_string()),
            source_locator: None,
            retrieval_weight: None,
        }],
        entities: vec![],
        relations: vec![],
        graph_passages: vec![],
    }
}

#[tokio::test]
async fn all_inserts_succeed_after_pre_purge() {
    let plane = MilvusDataPlane::new(test_config());
    let doc_id = Uuid::from_u128(42);
    let parse_run_id = Uuid::from_u128(99);

    let executor = FakeExecutor::new();
    let batch = make_test_batch(doc_id, parse_run_id);

    let result = plane.replace_document_index_impl(batch, &executor).await;
    assert!(result.is_ok(), "expected success: {:?}", result);

    // Pre-purge happened.
    let deletes = executor.delete_calls();
    assert_eq!(deletes.len(), 5);

    // Verify call order: purges BEFORE inserts.
    let calls = executor.calls();
    assert!(matches!(calls[0], Call::Delete { .. }));
}
