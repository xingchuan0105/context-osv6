use async_trait::async_trait;
use avrag_auth::{AuthContext, OrgId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Down-weight multiplier for OCR-failed page raster chunks in multimodal search (RET-1).
pub const FALLBACK_RETRIEVAL_WEIGHT: f32 = 0.4;

/// RET-1: down-weight `page_raster` chunks when the source page OCR failed.
pub fn multimodal_retrieval_weight(chunk_type: &str, page_ocr_failed: bool) -> Option<f32> {
    if chunk_type == "page_raster" && page_ocr_failed {
        Some(FALLBACK_RETRIEVAL_WEIGHT)
    } else {
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredChunk {
    pub chunk_id: Uuid,
    pub doc_id: Uuid,
    pub content: String,
    pub score: f32,
    pub source: String,
    pub page: Option<i64>,
    pub chunk_type: String,
    pub asset_id: Option<Uuid>,
    pub caption: Option<String>,
    pub image_path: Option<String>,
    pub parser_backend: Option<String>,
    pub source_locator: Option<Value>,
    pub parse_run_id: Option<Uuid>,
}

impl ScoredChunk {
    pub fn new_text(
        chunk_id: Uuid,
        doc_id: Uuid,
        content: String,
        score: f32,
        source: String,
        page: Option<i64>,
    ) -> Self {
        Self {
            chunk_id,
            doc_id,
            content,
            score,
            source,
            page,
            chunk_type: "text".to_string(),
            asset_id: None,
            caption: None,
            image_path: None,
            parser_backend: None,
            source_locator: None,
            parse_run_id: None,
        }
    }

    pub fn with_metadata(
        mut self,
        chunk_type: String,
        parser_backend: Option<String>,
        source_locator: Option<Value>,
    ) -> Self {
        self.chunk_type = chunk_type;
        self.parser_backend = parser_backend;
        self.source_locator = source_locator;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightedChunkList {
    pub weight: f32,
    pub chunks: Vec<ScoredChunk>,
}

#[derive(Debug, Clone)]
pub struct TextDenseSearchRequest {
    pub auth: AuthContext,
    pub query_vector: Vec<f32>,
    pub doc_ids: Option<Vec<Uuid>>,
    pub limit: usize,
}

#[derive(Debug, Clone)]
pub struct Bm25SearchRequest {
    pub auth: AuthContext,
    pub query: String,
    pub doc_ids: Option<Vec<Uuid>>,
    pub limit: usize,
}

#[derive(Debug, Clone)]
pub struct Bm25SearchTrace {
    pub backend: String,
    pub raw_hit_count: usize,
    pub hydrated_hit_count: usize,
    pub fallback_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Bm25SearchOutput {
    pub chunks: Vec<ScoredChunk>,
    pub trace: Bm25SearchTrace,
}

#[derive(Debug, Clone)]
pub struct MultimodalSearchRequest {
    pub auth: AuthContext,
    pub query_vector: Vec<f32>,
    pub doc_ids: Option<Vec<Uuid>>,
    pub limit: usize,
}

#[derive(Debug, Clone, Default)]
pub struct GraphRelationHint {
    pub subject: Option<String>,
    pub predicate: Option<String>,
    pub object: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GraphSearchRequest {
    pub auth: AuthContext,
    pub doc_ids: Option<Vec<Uuid>>,
    pub entity_names: Vec<String>,
    pub relation_hints: Vec<GraphRelationHint>,
    pub relation_limit: usize,
    pub supporting_chunk_limit: usize,
    /// Query-time entity names extracted from the user query.
    /// Used for vector similarity search against kg_entities when
    /// exact attribute matching is insufficient.
    pub query_entities: Vec<String>,
    /// Pre-computed vectors for query_entities.
    /// If provided, these are used for ANN search against kg_entities.
    /// If empty, query_entities text is used for exact-match fallback.
    pub query_entity_vectors: Vec<Vec<f32>>,
    /// Maximum number of hops for subgraph expansion. Default 1.
    pub hop_limit: usize,
    /// Maximum number of relations to retrieve per hop.
    pub fan_out_limit: usize,
    /// Tenant context for mandatory access control.
    /// All searches are scoped to this tenant's data.
    pub tenant_org_id: String,
}

#[derive(Debug, Clone, Default)]
pub struct RelationPathCandidate {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub score: f32,
    pub supporting_chunk_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Default)]
pub struct GraphSearchOutput {
    pub relation_paths: Vec<RelationPathCandidate>,
    pub supporting_chunks: Vec<ScoredChunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentIndexBatch {
    pub org_id: OrgId,
    pub workspace_id: Option<Uuid>,
    pub document_id: Uuid,
    pub parse_run_id: Uuid,
    pub doc_version: u32,
    pub text_chunks: Vec<TextChunkIndexRecord>,
    pub multimodal_chunks: Vec<MultimodalChunkIndexRecord>,
    pub entities: Vec<EntityIndexRecord>,
    pub relations: Vec<RelationIndexRecord>,
    pub graph_passages: Vec<GraphPassageIndexRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextChunkIndexRecord {
    pub chunk_id: Uuid,
    pub content: String,
    pub vector: Vec<f32>,
    pub page: Option<i64>,
    pub chunk_type: String,
    pub parser_backend: Option<String>,
    pub source_locator: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultimodalChunkIndexRecord {
    pub chunk_id: Uuid,
    pub asset_id: Uuid,
    pub context_text: String,
    pub caption: Option<String>,
    pub image_path: Option<String>,
    pub vector: Vec<f32>,
    pub page: Option<i64>,
    pub chunk_type: String,
    pub parser_backend: Option<String>,
    pub source_locator: Option<Value>,
    /// Score multiplier for retrieval (0.0-1.0). None = 1.0 (default).
    /// Used to down-weight fallback/low-quality chunks (e.g. OCR-fail page_raster = 0.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval_weight: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityIndexRecord {
    pub entity_id: Uuid,
    pub name: String,
    pub normalized_name: String,
    pub entity_type: Option<String>,
    pub vector: Vec<f32>,
    pub supporting_chunk_ids: Vec<Uuid>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationIndexRecord {
    pub relation_id: Uuid,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub relation_text: String,
    pub vector: Vec<f32>,
    pub supporting_chunk_ids: Vec<Uuid>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPassageIndexRecord {
    pub passage_id: Uuid,
    pub chunk_id: Option<Uuid>,
    pub text: String,
    pub vector: Vec<f32>,
    pub relation_ids: Vec<Uuid>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexWriteReport {
    pub text_chunk_count: usize,
    pub multimodal_chunk_count: usize,
    pub entity_count: usize,
    pub relation_count: usize,
    pub graph_passage_count: usize,
}

#[async_trait]
pub trait RetrievalDataPlane: Send + Sync {
    async fn ensure_schema(&self) -> anyhow::Result<()> {
        Err(retrieval_data_plane_method_not_implemented("ensure_schema"))
    }

    async fn replace_document_index(
        &self,
        _batch: DocumentIndexBatch,
    ) -> anyhow::Result<IndexWriteReport> {
        Err(retrieval_data_plane_method_not_implemented(
            "replace_document_index",
        ))
    }

    async fn delete_document_index(
        &self,
        _auth: &AuthContext,
        _document_id: Uuid,
    ) -> anyhow::Result<()> {
        Err(retrieval_data_plane_method_not_implemented(
            "delete_document_index",
        ))
    }

    async fn search_text_dense(
        &self,
        request: TextDenseSearchRequest,
    ) -> anyhow::Result<Vec<ScoredChunk>>;

    async fn search_bm25(&self, request: Bm25SearchRequest) -> anyhow::Result<Bm25SearchOutput>;

    async fn search_multimodal(
        &self,
        request: MultimodalSearchRequest,
    ) -> anyhow::Result<Vec<ScoredChunk>>;

    async fn search_graph(
        &self,
        _request: GraphSearchRequest,
    ) -> anyhow::Result<GraphSearchOutput> {
        Err(retrieval_data_plane_method_not_implemented("search_graph"))
    }
}

fn retrieval_data_plane_method_not_implemented(method: &str) -> anyhow::Error {
    anyhow::anyhow!(
        "RetrievalDataPlane method {method} is not implemented; configure a concrete retrieval data plane adapter instead of relying on trait defaults"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use avrag_auth::SubjectKind;

    struct PartialRetrievalDataPlane;

    #[async_trait]
    impl RetrievalDataPlane for PartialRetrievalDataPlane {
        async fn search_text_dense(
            &self,
            _request: TextDenseSearchRequest,
        ) -> anyhow::Result<Vec<ScoredChunk>> {
            Ok(Vec::new())
        }

        async fn search_bm25(
            &self,
            _request: Bm25SearchRequest,
        ) -> anyhow::Result<Bm25SearchOutput> {
            Ok(Bm25SearchOutput {
                chunks: Vec::new(),
                trace: Bm25SearchTrace {
                    backend: "test".to_string(),
                    raw_hit_count: 0,
                    hydrated_hit_count: 0,
                    fallback_reason: None,
                },
            })
        }

        async fn search_multimodal(
            &self,
            _request: MultimodalSearchRequest,
        ) -> anyhow::Result<Vec<ScoredChunk>> {
            Ok(Vec::new())
        }
    }

    fn auth_context() -> AuthContext {
        AuthContext::new(OrgId::from(Uuid::from_u128(1)), SubjectKind::System)
    }

    fn empty_index_batch() -> DocumentIndexBatch {
        DocumentIndexBatch {
            org_id: OrgId::from(Uuid::from_u128(1)),
            workspace_id: None,
            document_id: Uuid::from_u128(2),
            parse_run_id: Uuid::from_u128(3),
            doc_version: 1,
            text_chunks: Vec::new(),
            multimodal_chunks: Vec::new(),
            entities: Vec::new(),
            relations: Vec::new(),
            graph_passages: Vec::new(),
        }
    }

    fn assert_not_implemented(error: anyhow::Error, method: &str) {
        let message = error.to_string();
        assert!(message.contains(method), "{message}");
        assert!(message.contains("not implemented"), "{message}");
    }

    #[test]
    fn multimodal_retrieval_weight_downweights_ocr_fail_page_raster() {
        assert_eq!(
            multimodal_retrieval_weight("page_raster", true),
            Some(FALLBACK_RETRIEVAL_WEIGHT)
        );
        assert_eq!(multimodal_retrieval_weight("page_raster", false), None);
        assert_eq!(multimodal_retrieval_weight("text", true), None);
    }

    #[tokio::test]
    async fn default_write_and_graph_methods_fail_explicitly() {
        let data_plane = PartialRetrievalDataPlane;
        let auth = auth_context();

        assert_not_implemented(
            data_plane.ensure_schema().await.unwrap_err(),
            "ensure_schema",
        );
        assert_not_implemented(
            data_plane
                .replace_document_index(empty_index_batch())
                .await
                .unwrap_err(),
            "replace_document_index",
        );
        assert_not_implemented(
            data_plane
                .delete_document_index(&auth, Uuid::from_u128(2))
                .await
                .unwrap_err(),
            "delete_document_index",
        );
        assert_not_implemented(
            data_plane
                .search_graph(GraphSearchRequest {
                    auth,
                    doc_ids: None,
                    entity_names: Vec::new(),
                    relation_hints: Vec::new(),
                    relation_limit: 10,
                    supporting_chunk_limit: 10,
                    query_entities: Vec::new(),
                    query_entity_vectors: Vec::new(),
                    hop_limit: 1,
                    fan_out_limit: 10,
                    tenant_org_id: "test-org".to_string(),
                })
                .await
                .unwrap_err(),
            "search_graph",
        );
    }
}
