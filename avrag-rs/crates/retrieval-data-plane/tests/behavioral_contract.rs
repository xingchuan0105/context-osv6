use async_trait::async_trait;
use avrag_auth::{AuthContext, OrgId, SubjectKind};
use avrag_retrieval_data_plane::{
    DocumentIndexBatch, FALLBACK_RETRIEVAL_WEIGHT, MultimodalChunkIndexRecord, RetrievalDataPlane,
    TextDenseSearchRequest, multimodal_retrieval_weight,
};
use uuid::Uuid;

struct StubDataPlane;

#[async_trait]
impl RetrievalDataPlane for StubDataPlane {
    async fn search_text_dense(
        &self,
        _request: TextDenseSearchRequest,
    ) -> anyhow::Result<Vec<avrag_retrieval_data_plane::ScoredChunk>> {
        Ok(Vec::new())
    }

    async fn search_bm25(
        &self,
        _request: avrag_retrieval_data_plane::Bm25SearchRequest,
    ) -> anyhow::Result<avrag_retrieval_data_plane::Bm25SearchOutput> {
        Ok(avrag_retrieval_data_plane::Bm25SearchOutput {
            chunks: Vec::new(),
            trace: avrag_retrieval_data_plane::Bm25SearchTrace {
                backend: "stub".to_string(),
                raw_hit_count: 0,
                hydrated_hit_count: 0,
                fallback_reason: None,
            },
        })
    }

    async fn search_multimodal(
        &self,
        _request: avrag_retrieval_data_plane::MultimodalSearchRequest,
    ) -> anyhow::Result<Vec<avrag_retrieval_data_plane::ScoredChunk>> {
        Ok(Vec::new())
    }
}

fn auth_context() -> AuthContext {
    AuthContext::new(OrgId::from(Uuid::from_u128(1)), SubjectKind::System)
}

#[test]
fn multimodal_retrieval_weight_downweights_failed_ocr_page_raster() {
    assert_eq!(
        multimodal_retrieval_weight("page_raster", true),
        Some(FALLBACK_RETRIEVAL_WEIGHT)
    );
    assert_eq!(multimodal_retrieval_weight("text", true), None);
}

#[test]
fn document_index_batch_roundtrip_preserves_retrieval_weight() {
    let batch = DocumentIndexBatch {
        org_id: OrgId::from(Uuid::from_u128(10)),
        workspace_id: None,
        document_id: Uuid::from_u128(11),
        parse_run_id: Uuid::from_u128(12),
        doc_version: 1,
        text_chunks: Vec::new(),
        multimodal_chunks: vec![MultimodalChunkIndexRecord {
            chunk_id: Uuid::from_u128(13),
            asset_id: Uuid::from_u128(14),
            context_text: "figure caption".to_string(),
            caption: Some("chart".to_string()),
            image_path: Some("assets/chart.png".to_string()),
            vector: vec![0.1, 0.2],
            page: Some(1),
            chunk_type: "page_raster".to_string(),
            parser_backend: Some("paddle".to_string()),
            source_locator: None,
            retrieval_weight: Some(FALLBACK_RETRIEVAL_WEIGHT),
        }],
        entities: Vec::new(),
        relations: Vec::new(),
        graph_passages: Vec::new(),
    };

    let encoded = serde_json::to_value(&batch).unwrap();
    let decoded: DocumentIndexBatch = serde_json::from_value(encoded).unwrap();

    assert_eq!(
        decoded.multimodal_chunks[0].retrieval_weight,
        Some(FALLBACK_RETRIEVAL_WEIGHT)
    );
}

#[tokio::test]
async fn default_graph_search_fails_with_explicit_adapter_message() {
    let data_plane = StubDataPlane;
    let error = data_plane
        .search_graph(avrag_retrieval_data_plane::GraphSearchRequest {
            auth: auth_context(),
            doc_ids: None,
            entity_names: Vec::new(),
            relation_hints: Vec::new(),
            relation_limit: 5,
            supporting_chunk_limit: 5,
            query_entities: Vec::new(),
            query_entity_vectors: Vec::new(),
            hop_limit: 1,
            fan_out_limit: 5,
            tenant_org_id: "org-1".to_string(),
        })
        .await
        .unwrap_err();

    let message = error.to_string();
    assert!(message.contains("search_graph"));
    assert!(message.contains("not implemented"));
}

#[test]
fn scored_chunk_new_text_defaults_to_text_chunk_type() {
    let chunk_id = Uuid::from_u128(20);
    let doc_id = Uuid::from_u128(21);
    let chunk = avrag_retrieval_data_plane::ScoredChunk::new_text(
        chunk_id,
        doc_id,
        "dense hit".to_string(),
        0.91,
        "text_dense".to_string(),
        Some(3),
    );

    assert_eq!(chunk.chunk_id, chunk_id);
    assert_eq!(chunk.doc_id, doc_id);
    assert_eq!(chunk.chunk_type, "text");
    assert_eq!(chunk.source, "text_dense");
    assert_eq!(chunk.page, Some(3));
}

#[test]
fn scored_chunk_with_metadata_preserves_parser_backend_and_locator() {
    let chunk = avrag_retrieval_data_plane::ScoredChunk::new_text(
        Uuid::from_u128(30),
        Uuid::from_u128(31),
        "figure caption".to_string(),
        0.75,
        "multimodal_dense".to_string(),
        Some(2),
    )
    .with_metadata(
        "page_raster".to_string(),
        Some("paddle".to_string()),
        Some(serde_json::json!({"page": 2, "bbox": [0, 0, 100, 100]})),
    );

    assert_eq!(chunk.chunk_type, "page_raster");
    assert_eq!(chunk.parser_backend.as_deref(), Some("paddle"));
    assert_eq!(
        chunk.source_locator.as_ref().and_then(|v| v.get("page")),
        Some(&serde_json::json!(2))
    );
}

#[test]
fn weighted_chunk_list_roundtrip_preserves_weight_and_chunks() {
    let list = avrag_retrieval_data_plane::WeightedChunkList {
        weight: FALLBACK_RETRIEVAL_WEIGHT,
        chunks: vec![avrag_retrieval_data_plane::ScoredChunk::new_text(
            Uuid::from_u128(40),
            Uuid::from_u128(41),
            "fallback chunk".to_string(),
            0.4,
            "multimodal_dense".to_string(),
            Some(1),
        )],
    };

    let encoded = serde_json::to_value(&list).unwrap();
    let decoded: avrag_retrieval_data_plane::WeightedChunkList =
        serde_json::from_value(encoded).unwrap();

    assert_eq!(decoded.weight, FALLBACK_RETRIEVAL_WEIGHT);
    assert_eq!(decoded.chunks.len(), 1);
    assert_eq!(decoded.chunks[0].content, "fallback chunk");
}

#[tokio::test]
async fn default_index_write_methods_fail_with_explicit_adapter_message() {
    let data_plane = StubDataPlane;
    let auth = auth_context();

    for (method, error) in [
        (
            "ensure_schema",
            data_plane.ensure_schema().await.unwrap_err(),
        ),
        (
            "replace_document_index",
            data_plane
                .replace_document_index(DocumentIndexBatch {
                    org_id: auth.org_id(),
                    workspace_id: None,
                    document_id: Uuid::from_u128(50),
                    parse_run_id: Uuid::from_u128(51),
                    doc_version: 1,
                    text_chunks: Vec::new(),
                    multimodal_chunks: Vec::new(),
                    entities: Vec::new(),
                    relations: Vec::new(),
                    graph_passages: Vec::new(),
                })
                .await
                .unwrap_err(),
        ),
        (
            "delete_document_index",
            data_plane
                .delete_document_index(&auth, Uuid::from_u128(50))
                .await
                .unwrap_err(),
        ),
    ] {
        let message = error.to_string();
        assert!(message.contains(method), "{message}");
        assert!(message.contains("not implemented"), "{message}");
    }
}

#[test]
fn index_write_report_default_is_zero_counts() {
    let report = avrag_retrieval_data_plane::IndexWriteReport::default();

    assert_eq!(report.text_chunk_count, 0);
    assert_eq!(report.multimodal_chunk_count, 0);
    assert_eq!(report.entity_count, 0);
    assert_eq!(report.relation_count, 0);
    assert_eq!(report.graph_passage_count, 0);
}
