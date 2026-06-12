use avrag_auth::{AuthContext, OrgId, SubjectKind};
use avrag_retrieval_data_plane::{
    DocumentIndexBatch, FALLBACK_RETRIEVAL_WEIGHT, MultimodalChunkIndexRecord,
    RetrievalDataPlane, TextDenseSearchRequest, multimodal_retrieval_weight,
};
use async_trait::async_trait;
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
