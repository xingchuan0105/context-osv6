use async_trait::async_trait;
use avrag_auth::{AuthContext, OrgId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

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
        Ok(())
    }

    async fn replace_document_index(
        &self,
        batch: DocumentIndexBatch,
    ) -> anyhow::Result<IndexWriteReport> {
        Ok(IndexWriteReport {
            text_chunk_count: batch.text_chunks.len(),
            multimodal_chunk_count: batch.multimodal_chunks.len(),
            entity_count: batch.entities.len(),
            relation_count: batch.relations.len(),
            graph_passage_count: batch.graph_passages.len(),
        })
    }

    async fn delete_document_index(
        &self,
        _auth: &AuthContext,
        _document_id: Uuid,
    ) -> anyhow::Result<()> {
        Ok(())
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
        Ok(GraphSearchOutput::default())
    }
}
