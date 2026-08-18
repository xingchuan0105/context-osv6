use async_trait::async_trait;
use contracts::auth_runtime::{AuthContext, UserId};
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
    /// In-document sequence from ingest `metadata.cursor` (optional; missing → skip adjacent merge).
    #[serde(default)]
    pub cursor: Option<i32>,
    /// Atomic chunk ids in an S+L evidence run (cursor order). Empty ⇒ treat as `[chunk_id]`.
    #[serde(default)]
    pub member_chunk_ids: Vec<Uuid>,
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
            cursor: None,
            member_chunk_ids: Vec::new(),
        }
    }

    /// Member ids for reseen closure; falls back to `[chunk_id]` when empty.
    pub fn members(&self) -> Vec<Uuid> {
        if self.member_chunk_ids.is_empty() {
            vec![self.chunk_id]
        } else {
            self.member_chunk_ids.clone()
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
        if self.cursor.is_none() {
            self.cursor = cursor_from_value(source_locator.as_ref());
        }
        self.source_locator = source_locator;
        self
    }

    pub fn with_cursor(mut self, cursor: Option<i32>) -> Self {
        self.cursor = cursor;
        self
    }
}

/// Parse ingest cursor from `source_locator` or chunk metadata JSON.
pub fn cursor_from_value(v: Option<&Value>) -> Option<i32> {
    let obj = v?.as_object()?;
    obj.get("cursor")
        .and_then(|c| {
            c.as_i64()
                .or_else(|| c.as_u64().map(|u| u as i64))
                .or_else(|| c.as_str().and_then(|s| s.parse().ok()))
        })
        .map(|i| i as i32)
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
    /// Account owner for mandatory access control.
    /// All searches are scoped to this account's data.
    pub owner_user_id: String,
}

#[derive(Debug, Clone, Default)]
pub struct RelationPathCandidate {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub score: f32,
    pub supporting_chunk_ids: Vec<Uuid>,
    /// Source document of this relation edge (for cite-safe evidence fallback).
    pub doc_id: Uuid,
}

#[derive(Debug, Clone, Default)]
pub struct GraphSearchOutput {
    pub relation_paths: Vec<RelationPathCandidate>,
    pub supporting_chunks: Vec<ScoredChunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentIndexBatch {
    pub owner_user_id: UserId,
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

impl DocumentIndexBatch {
    /// Rebind owner / workspace for cloud import. Chunk, entity, and relation ids stay put.
    pub fn rebind_owner(&mut self, owner_user_id: UserId, workspace_id: Uuid) {
        self.owner_user_id = owner_user_id;
        self.workspace_id = Some(workspace_id);
    }
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

// ── Publish / export (ADR-0010 §5.6) ────────────────────────────────────────

/// Bundle format version for WorkspacePublishBundle (manifest schema_version).
pub const EXPORT_SCHEMA_VERSION: &str = "workspace_publish_bundle_v1";

/// Model / schema fingerprint required so cloud import can accept vectors
/// without re-embedding. Not stored on [`DocumentIndexBatch`] today — lives on
/// the export manifest only.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublishFingerprint {
    pub embedding_model_id: String,
    pub vector_dim: usize,
    /// When multimodal embedding dim differs from text; defaults to `vector_dim`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multimodal_vector_dim: Option<usize>,
    pub schema_version: String,
}

impl PublishFingerprint {
    pub fn new(embedding_model_id: impl Into<String>, vector_dim: usize) -> Self {
        Self {
            embedding_model_id: embedding_model_id.into(),
            vector_dim,
            multimodal_vector_dim: None,
            schema_version: EXPORT_SCHEMA_VERSION.to_string(),
        }
    }

    pub fn multimodal_dim(&self) -> usize {
        self.multimodal_vector_dim.unwrap_or(self.vector_dim)
    }

    /// Validate every vector in `batch` against this fingerprint's dims.
    pub fn validate_batch(&self, batch: &DocumentIndexBatch) -> anyhow::Result<()> {
        validate_batch_vector_dims(batch, self.vector_dim, self.multimodal_dim())
    }

    /// Model id + dims + schema must match for vector import without re-embed.
    pub fn incompatible_field(&self, other: &Self) -> Option<&'static str> {
        if self.embedding_model_id != other.embedding_model_id {
            return Some("embedding_model_id");
        }
        if self.vector_dim != other.vector_dim {
            return Some("vector_dim");
        }
        if self.multimodal_dim() != other.multimodal_dim() {
            return Some("multimodal_vector_dim");
        }
        if self.schema_version != other.schema_version {
            return Some("schema_version");
        }
        None
    }
}

/// Counts + identity + fingerprint for one document export (publish manifest slice).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentExportManifest {
    pub fingerprint: PublishFingerprint,
    pub owner_user_id: UserId,
    pub workspace_id: Option<Uuid>,
    pub document_id: Uuid,
    pub parse_run_id: Uuid,
    pub doc_version: u32,
    pub text_chunk_count: usize,
    pub multimodal_chunk_count: usize,
    pub entity_count: usize,
    pub relation_count: usize,
    pub graph_passage_count: usize,
}

/// Full export payload: manifest (fingerprint + counts) + index batch with vectors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentIndexExport {
    pub manifest: DocumentExportManifest,
    pub batch: DocumentIndexBatch,
}

/// Request to export one document's retrieval index including embedding vectors.
#[derive(Debug, Clone)]
pub struct ExportDocumentRequest {
    pub auth: AuthContext,
    pub document_id: Uuid,
    /// When set, only rows with this `workspace_id` are exported.
    pub workspace_id: Option<Uuid>,
    /// Caller-supplied model fingerprint (not persisted in rag_* tables today).
    pub fingerprint: PublishFingerprint,
}

/// Optional document-level meta for publish packaging (summary / TOC / profile).
/// Implementors may return empty until product document tables are wired.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocumentExportMeta {
    pub summary: Option<String>,
    pub toc: Option<Value>,
    pub profile: Option<Value>,
}

/// Validate a single vector length (shared write + export path).
pub fn validate_vector_dim(path: &str, actual: usize, expected: usize) -> anyhow::Result<()> {
    if actual == expected {
        return Ok(());
    }
    Err(anyhow::anyhow!(
        "vector dimension mismatch for {path}: expected {expected}, got {actual}"
    ))
}

/// Validate all dense vectors on a [`DocumentIndexBatch`].
pub fn validate_batch_vector_dims(
    batch: &DocumentIndexBatch,
    text_dim: usize,
    multimodal_dim: usize,
) -> anyhow::Result<()> {
    for (idx, chunk) in batch.text_chunks.iter().enumerate() {
        validate_vector_dim(
            &format!("text_chunks[{idx}].text_dense"),
            chunk.vector.len(),
            text_dim,
        )?;
    }
    for (idx, chunk) in batch.multimodal_chunks.iter().enumerate() {
        validate_vector_dim(
            &format!("multimodal_chunks[{idx}].multimodal_dense"),
            chunk.vector.len(),
            multimodal_dim,
        )?;
    }
    for (idx, entity) in batch.entities.iter().enumerate() {
        validate_vector_dim(
            &format!("entities[{idx}].entity_dense"),
            entity.vector.len(),
            text_dim,
        )?;
    }
    for (idx, relation) in batch.relations.iter().enumerate() {
        validate_vector_dim(
            &format!("relations[{idx}].relation_dense"),
            relation.vector.len(),
            text_dim,
        )?;
    }
    for (idx, passage) in batch.graph_passages.iter().enumerate() {
        validate_vector_dim(
            &format!("graph_passages[{idx}].passage_dense"),
            passage.vector.len(),
            text_dim,
        )?;
    }
    Ok(())
}

/// Export port: read back full index records **including vectors** for publish
/// (ADR-0010 §5.6). Separate from [`RetrievalReadPort`] (search has no vectors).
#[async_trait]
pub trait RetrievalExportPort: Send + Sync {
    /// Export one document's index rows with embedding vectors.
    async fn export_document_index(
        &self,
        request: ExportDocumentRequest,
    ) -> anyhow::Result<DocumentIndexExport>;

    /// Optional document meta (summary/TOC/profile). Default: empty.
    async fn export_document_meta(
        &self,
        _auth: &AuthContext,
        _document_id: Uuid,
    ) -> anyhow::Result<DocumentExportMeta> {
        Ok(DocumentExportMeta::default())
    }

    /// List document ids that have at least one index row for the owner,
    /// optionally scoped to a workspace.
    async fn list_indexed_document_ids(
        &self,
        auth: &AuthContext,
        workspace_id: Option<Uuid>,
    ) -> anyhow::Result<Vec<Uuid>>;
}

/// Read/query contract for the retrieval data plane.
///
/// Consumers that only run queries (e.g. `RagRuntime` during a chat turn) depend
/// on this narrow trait instead of the full [`RetrievalDataPlane`], so they are
/// not coupled to write/schema methods they never call.
#[async_trait]
pub trait RetrievalReadPort: Send + Sync {
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
        Err(anyhow::anyhow!(
            "search_graph is not implemented on this retrieval adapter"
        ))
    }

    /// Count indexed text (body) chunks for the given doc scope.
    ///
    /// Used by the retrieval runtime to size the dynamic rough-recall budget
    /// (docscope chunk total × fraction). Returns 0 by default so stubs and
    /// adapters without a count capability fall back to the configured floor.
    async fn count_text_chunks(
        &self,
        _auth: &AuthContext,
        _doc_ids: &[Uuid],
    ) -> anyhow::Result<usize> {
        Ok(0)
    }

    /// List ALL text (body) chunks for the given doc scope with full content.
    ///
    /// Backs host `doc_scan` / sandbox `client.doc_scan`: code-side scan over
    /// traversal/aggregate operators (re, collections, set, ...) over a doc's
    /// entire chunk set — for "how many / count / distribution" queries that
    /// dense/lexical top-K cannot answer. Returns empty by default so stubs and
    /// adapters without a scan capability degrade gracefully.
    async fn list_text_chunks(
        &self,
        _auth: &AuthContext,
        _doc_ids: &[Uuid],
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        Ok(Vec::new())
    }
}

/// Full data plane: extends [`RetrievalReadPort`] with **required** index
/// write/schema methods used by the ingestion worker and bootstrap.
///
/// Read-only consumers should depend on [`RetrievalReadPort`] only — do not
/// implement this trait with stub writes.
#[async_trait]
pub trait RetrievalDataPlane: RetrievalReadPort {
    async fn ensure_schema(&self) -> anyhow::Result<()>;

    async fn replace_document_index(
        &self,
        batch: DocumentIndexBatch,
    ) -> anyhow::Result<IndexWriteReport>;

    async fn delete_document_index(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
    ) -> anyhow::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::auth_runtime::SubjectKind;

    struct PartialRetrievalDataPlane;

    #[async_trait]
    impl RetrievalReadPort for PartialRetrievalDataPlane {
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

    #[async_trait]
    impl RetrievalDataPlane for PartialRetrievalDataPlane {
        async fn ensure_schema(&self) -> anyhow::Result<()> {
            Ok(())
        }

        async fn replace_document_index(
            &self,
            _batch: DocumentIndexBatch,
        ) -> anyhow::Result<IndexWriteReport> {
            Ok(IndexWriteReport {
                text_chunk_count: 0,
                multimodal_chunk_count: 0,
                entity_count: 0,
                relation_count: 0,
                graph_passage_count: 0,
            })
        }

        async fn delete_document_index(
            &self,
            _auth: &AuthContext,
            _document_id: Uuid,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    fn auth_context() -> AuthContext {
        AuthContext::new(UserId::from(Uuid::from_u128(1)), SubjectKind::System)
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
    async fn write_methods_are_required_on_full_data_plane() {
        let data_plane = PartialRetrievalDataPlane;
        let auth = auth_context();
        data_plane.ensure_schema().await.unwrap();
        data_plane
            .delete_document_index(&auth, Uuid::from_u128(2))
            .await
            .unwrap();
        let err = data_plane
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
                owner_user_id: "test-org".to_string(),
            })
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("search_graph"),
            "{err}"
        );
    }

    #[test]
    fn validate_vector_dim_ok_and_mismatch() {
        assert!(validate_vector_dim("v", 1024, 1024).is_ok());
        let err = validate_vector_dim("v", 4, 1024).unwrap_err();
        assert!(err.to_string().contains("dimension mismatch"), "{err}");
    }

    #[test]
    fn fingerprint_validate_batch_checks_all_record_kinds() {
        let fp = PublishFingerprint::new("test-embed", 2);
        let batch = DocumentIndexBatch {
            owner_user_id: UserId::from(Uuid::from_u128(1)),
            workspace_id: None,
            document_id: Uuid::from_u128(2),
            parse_run_id: Uuid::from_u128(3),
            doc_version: 1,
            text_chunks: vec![TextChunkIndexRecord {
                chunk_id: Uuid::from_u128(4),
                content: "c".into(),
                vector: vec![0.0, 1.0],
                page: None,
                chunk_type: "text".into(),
                parser_backend: None,
                source_locator: None,
            }],
            multimodal_chunks: vec![MultimodalChunkIndexRecord {
                chunk_id: Uuid::from_u128(5),
                asset_id: Uuid::from_u128(6),
                context_text: "m".into(),
                caption: None,
                image_path: None,
                vector: vec![0.5, 0.5],
                page: None,
                chunk_type: "multimodal".into(),
                parser_backend: None,
                source_locator: None,
                retrieval_weight: None,
            }],
            entities: vec![EntityIndexRecord {
                entity_id: Uuid::from_u128(7),
                name: "A".into(),
                normalized_name: "a".into(),
                entity_type: None,
                vector: vec![1.0, 0.0],
                supporting_chunk_ids: vec![],
                metadata: None,
            }],
            relations: vec![RelationIndexRecord {
                relation_id: Uuid::from_u128(8),
                subject: "A".into(),
                predicate: "r".into(),
                object: "B".into(),
                relation_text: "A r B".into(),
                vector: vec![0.1, 0.9],
                supporting_chunk_ids: vec![],
                metadata: None,
            }],
            graph_passages: vec![GraphPassageIndexRecord {
                passage_id: Uuid::from_u128(9),
                chunk_id: None,
                text: "p".into(),
                vector: vec![0.2, 0.8],
                relation_ids: vec![],
                metadata: None,
            }],
        };
        fp.validate_batch(&batch).unwrap();

        let mut bad = batch.clone();
        bad.text_chunks[0].vector = vec![1.0]; // dim 1 != 2
        let err = fp.validate_batch(&bad).unwrap_err();
        assert!(err.to_string().contains("text_chunks[0]"), "{err}");

        let other = PublishFingerprint::new("other-embed", 2);
        assert_eq!(fp.incompatible_field(&other), Some("embedding_model_id"));
        let wrong_dim = PublishFingerprint::new("test-embed", 8);
        assert_eq!(fp.incompatible_field(&wrong_dim), Some("vector_dim"));

        let new_owner = UserId::from(Uuid::from_u128(99));
        let new_ws = Uuid::from_u128(77);
        let mut rebound = batch;
        rebound.rebind_owner(new_owner, new_ws);
        assert_eq!(rebound.owner_user_id, new_owner);
        assert_eq!(rebound.workspace_id, Some(new_ws));
        assert_eq!(rebound.document_id, Uuid::from_u128(2));
        assert_eq!(rebound.text_chunks[0].chunk_id, Uuid::from_u128(4));
    }

    #[test]
    fn export_manifest_roundtrip_preserves_fingerprint() {
        let manifest = DocumentExportManifest {
            fingerprint: PublishFingerprint::new("bge-m3", 1024),
            owner_user_id: UserId::from(Uuid::from_u128(1)),
            workspace_id: Some(Uuid::from_u128(2)),
            document_id: Uuid::from_u128(3),
            parse_run_id: Uuid::from_u128(4),
            doc_version: 7,
            text_chunk_count: 1,
            multimodal_chunk_count: 0,
            entity_count: 0,
            relation_count: 0,
            graph_passage_count: 0,
        };
        let encoded = serde_json::to_value(&manifest).unwrap();
        let decoded: DocumentExportManifest = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded, manifest);
        assert_eq!(decoded.fingerprint.schema_version, EXPORT_SCHEMA_VERSION);
    }
}
