//! Optional live smoke against DATABASE_URL.
//!
//! ```bash
//! DATABASE_URL=postgres://avrag:avrag@127.0.0.1:5432/avrag_rs \
//!   cargo test -p avrag-storage-pgvector --test smoke_pgvector -- --ignored --nocapture
//! ```

use avrag_retrieval_data_plane::{
    DocumentIndexBatch, EntityIndexRecord, GraphSearchRequest, RelationIndexRecord,
    RetrievalDataPlane, RetrievalReadPort, TextChunkIndexRecord, TextDenseSearchRequest,
};
use avrag_storage_pgvector::{PgvectorConfig, PgvectorDataPlane};
use contracts::auth_runtime::{AuthContext, SubjectKind, UserId};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

fn unit_vec(dim: usize, hotspot: usize) -> Vec<f32> {
    let mut v = vec![0.0; dim];
    if hotspot < dim {
        v[hotspot] = 1.0;
    }
    v
}

#[tokio::test]
#[ignore = "requires DATABASE_URL with pgvector + migration 0060"]
async fn replace_search_and_graph_roundtrip() {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .expect("connect");

    let dim = 1024;
    let plane = PgvectorDataPlane::new(
        pool,
        PgvectorConfig {
            text_vector_dim: dim,
            multimodal_vector_dim: dim,
            hnsw_ef_search: Some(40),
        },
    );
    plane.ensure_schema().await.expect("ensure_schema");

    let owner = Uuid::new_v4();
    let doc_id = Uuid::new_v4();
    let chunk_id = Uuid::new_v4();
    let entity_a = Uuid::new_v4();
    let entity_b = Uuid::new_v4();
    let relation_id = Uuid::new_v4();
    let parse_run_id = Uuid::new_v4();

    let batch = DocumentIndexBatch {
        owner_user_id: UserId::from(owner),
        workspace_id: None,
        document_id: doc_id,
        parse_run_id,
        doc_version: 1,
        text_chunks: vec![TextChunkIndexRecord {
            chunk_id,
            content: "alpha depends on beta in the graph smoke test".to_string(),
            vector: unit_vec(dim, 3),
            page: Some(1),
            chunk_type: "text".to_string(),
            parser_backend: Some("smoke".to_string()),
            source_locator: None,
        }],
        multimodal_chunks: vec![],
        entities: vec![
            EntityIndexRecord {
                entity_id: entity_a,
                name: "Alpha".to_string(),
                normalized_name: "alpha".to_string(),
                entity_type: None,
                vector: unit_vec(dim, 1),
                supporting_chunk_ids: vec![chunk_id],
                metadata: None,
            },
            EntityIndexRecord {
                entity_id: entity_b,
                name: "Beta".to_string(),
                normalized_name: "beta".to_string(),
                entity_type: None,
                vector: unit_vec(dim, 2),
                supporting_chunk_ids: vec![chunk_id],
                metadata: None,
            },
        ],
        relations: vec![RelationIndexRecord {
            relation_id,
            subject: "Alpha".to_string(),
            predicate: "depends_on".to_string(),
            object: "Beta".to_string(),
            relation_text: "Alpha depends_on Beta".to_string(),
            vector: unit_vec(dim, 4),
            supporting_chunk_ids: vec![chunk_id],
            metadata: None,
        }],
        graph_passages: vec![],
    };

    let report = plane
        .replace_document_index(batch)
        .await
        .expect("replace_document_index");
    assert_eq!(report.text_chunk_count, 1);
    assert_eq!(report.entity_count, 2);
    assert_eq!(report.relation_count, 1);

    let auth = AuthContext::new(UserId::from(owner), SubjectKind::System);
    let dense_hits = plane
        .search_text_dense(TextDenseSearchRequest {
            auth: auth.clone(),
            query_vector: unit_vec(dim, 3),
            doc_ids: Some(vec![doc_id]),
            limit: 5,
        })
        .await
        .expect("search_text_dense");
    assert!(
        !dense_hits.is_empty(),
        "expected dense hit, got {dense_hits:?}"
    );
    assert_eq!(dense_hits[0].chunk_id, chunk_id);

    let graph = plane
        .search_graph(GraphSearchRequest {
            auth: auth.clone(),
            doc_ids: Some(vec![doc_id]),
            entity_names: vec!["Alpha".to_string()],
            relation_hints: vec![],
            relation_limit: 10,
            supporting_chunk_limit: 10,
            query_entities: vec![],
            query_entity_vectors: vec![],
            hop_limit: 1,
            fan_out_limit: 20,
            owner_user_id: owner.to_string(),
        })
        .await
        .expect("search_graph");
    assert_eq!(graph.relation_paths.len(), 1);
    assert_eq!(graph.relation_paths[0].subject, "Alpha");
    assert_eq!(graph.relation_paths[0].object, "Beta");
    assert!(!graph.supporting_chunks.is_empty());

    plane
        .delete_document_index(&auth, doc_id)
        .await
        .expect("delete_document_index");
}
