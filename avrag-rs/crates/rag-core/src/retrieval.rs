use avrag_auth::AuthContext;
use avrag_search::TantivyLexicalIndex;
use avrag_storage_pg::PgAppRepository;
use avrag_storage_qdrant::HttpQdrantBackend;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DenseSearchHit {
    pub chunk_id: Uuid,
    pub doc_id: Uuid,
    pub score: f32,
    pub page: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct SparseSearchHit {
    pub chunk_id: Uuid,
    pub doc_id: Uuid,
    pub content: String,
    pub score: f32,
    pub page: Option<i64>,
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
        }
    }

    fn with_metadata(
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

#[derive(Debug, Clone)]
pub struct RetrievalResult {
    pub dense_hits: Vec<DenseSearchHit>,
    pub sparse_hits: Vec<SparseSearchHit>,
    pub merged: Vec<ScoredChunk>,
}

#[derive(Debug, Clone)]
pub struct ChunkRow {
    pub chunk_id: Uuid,
    pub doc_id: Uuid,
    pub content: String,
    pub bm25_score: f32,
    pub page: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct DenseRetrievalResult {
    pub hits: Vec<DenseSearchHit>,
}

#[derive(Debug, Clone)]
pub struct SparseRetrievalResult {
    pub hits: Vec<SparseSearchHit>,
}

#[derive(Debug, Clone)]
pub struct SparseRetrievalTrace {
    pub backend: String,
    pub raw_hit_count: usize,
    pub hydrated_hit_count: usize,
    pub fallback_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SparseRetrievalOutput {
    pub chunks: Vec<ScoredChunk>,
    pub trace: SparseRetrievalTrace,
}

pub async fn run_dense_retrieval(
    qdrant: &HttpQdrantBackend,
    repo: &PgAppRepository,
    auth: &AuthContext,
    collection: &str,
    query_vector: Vec<f32>,
    doc_ids: Option<&[Uuid]>,
    limit: usize,
) -> anyhow::Result<Vec<ScoredChunk>> {
    let hits = qdrant
        .search_dense(collection, query_vector, auth.org_id(), doc_ids, limit)
        .await?;

    if hits.is_empty() {
        return Ok(Vec::new());
    }

    let chunk_ids: Vec<Uuid> = hits.iter().map(|h| h.chunk_id).collect();
    let chunk_map = repo.get_chunks_by_ids(auth, &chunk_ids).await?;

    let mut scored_chunks = Vec::with_capacity(hits.len());
    for hit in hits {
        if let Some(chunk) = chunk_map.get(&hit.chunk_id) {
            scored_chunks.push(
                ScoredChunk::new_text(
                    hit.chunk_id,
                    hit.doc_id,
                    chunk.content.clone(),
                    hit.score,
                    "dense".to_string(),
                    hit.page.map(|p| p as i64),
                )
                .with_metadata(
                    chunk_metadata_string(&chunk.metadata, "block_type")
                        .unwrap_or_else(|| "text".to_string()),
                    chunk_metadata_string(&chunk.metadata, "parser_backend"),
                    chunk_metadata_value(&chunk.metadata, "source_locator"),
                ),
            );
        }
    }

    Ok(scored_chunks)
}

pub async fn run_sparse_retrieval(
    repo: &PgAppRepository,
    auth: &AuthContext,
    query: &str,
    doc_ids: Option<&[Uuid]>,
    limit: usize,
) -> anyhow::Result<Vec<ScoredChunk>> {
    Ok(
        run_sparse_retrieval_with_lexical(None, repo, auth, query, doc_ids, limit)
            .await?
            .chunks,
    )
}

pub async fn run_sparse_retrieval_with_lexical(
    lexical_index: Option<&TantivyLexicalIndex>,
    repo: &PgAppRepository,
    auth: &AuthContext,
    query: &str,
    doc_ids: Option<&[Uuid]>,
    limit: usize,
) -> anyhow::Result<SparseRetrievalOutput> {
    if let Some(index) = lexical_index {
        match index.search(auth.org_id().into_uuid(), query, doc_ids, limit) {
            Ok(hits) => {
                let raw_hit_count = hits.len();
                let chunk_ids: Vec<Uuid> = hits.iter().map(|hit| hit.chunk_id).collect();
                let chunk_map = repo.get_chunks_by_ids(auth, &chunk_ids).await?;
                let mut chunks = Vec::with_capacity(hits.len());
                for hit in hits {
                    if let Some(chunk) = chunk_map.get(&hit.chunk_id) {
                        chunks.push(
                            ScoredChunk::new_text(
                                hit.chunk_id,
                                hit.doc_id,
                                chunk.content.clone(),
                                hit.score,
                                "lexical".to_string(),
                                chunk.page.or(hit.page),
                            )
                            .with_metadata(
                                chunk_metadata_string(&chunk.metadata, "block_type")
                                    .unwrap_or_else(|| "text".to_string()),
                                chunk_metadata_string(&chunk.metadata, "parser_backend"),
                                chunk_metadata_value(&chunk.metadata, "source_locator"),
                            ),
                        );
                    }
                }
                let hydrated_hit_count = chunks.len();
                tracing::info!(
                    lexical_backend = "tantivy",
                    raw_hit_count,
                    hydrated_hit_count,
                    "sparse retrieval completed"
                );
                return Ok(SparseRetrievalOutput {
                    chunks,
                    trace: SparseRetrievalTrace {
                        backend: "tantivy".to_string(),
                        raw_hit_count,
                        hydrated_hit_count,
                        fallback_reason: None,
                    },
                });
            }
            Err(error) => {
                let fallback_reason = error.to_string();
                tracing::info!(
                    lexical_backend = "tantivy",
                    error = %fallback_reason,
                    "sparse retrieval falling back to postgres bm25"
                );
                let chunks =
                    run_postgres_sparse_retrieval(repo, auth, query, doc_ids, limit).await?;
                let hydrated_hit_count = chunks.len();
                return Ok(SparseRetrievalOutput {
                    chunks,
                    trace: SparseRetrievalTrace {
                        backend: "postgres_bm25".to_string(),
                        raw_hit_count: hydrated_hit_count,
                        hydrated_hit_count,
                        fallback_reason: Some(fallback_reason),
                    },
                });
            }
        }
    }

    let chunks = run_postgres_sparse_retrieval(repo, auth, query, doc_ids, limit).await?;
    let hydrated_hit_count = chunks.len();
    Ok(SparseRetrievalOutput {
        chunks,
        trace: SparseRetrievalTrace {
            backend: "postgres_bm25".to_string(),
            raw_hit_count: hydrated_hit_count,
            hydrated_hit_count,
            fallback_reason: None,
        },
    })
}

async fn run_postgres_sparse_retrieval(
    repo: &PgAppRepository,
    auth: &AuthContext,
    query: &str,
    doc_ids: Option<&[Uuid]>,
    limit: usize,
) -> anyhow::Result<Vec<ScoredChunk>> {
    let chunks = repo.search_chunks_bm25(auth, query, doc_ids, limit).await?;

    Ok(chunks
        .into_iter()
        .map(|chunk| ScoredChunk {
            chunk_id: Uuid::parse_str(&chunk.chunk_id).unwrap_or_else(|_| Uuid::nil()),
            doc_id: Uuid::parse_str(&chunk.doc_id).unwrap_or_else(|_| Uuid::nil()),
            content: chunk.content,
            score: chunk.score.unwrap_or(0.0),
            source: "sparse".to_string(),
            page: chunk.page,
            chunk_type: chunk_metadata_string(&chunk.metadata, "block_type")
                .unwrap_or_else(|| "text".to_string()),
            asset_id: None,
            caption: None,
            image_path: None,
            parser_backend: chunk_metadata_string(&chunk.metadata, "parser_backend"),
            source_locator: chunk_metadata_value(&chunk.metadata, "source_locator"),
        })
        .collect())
}

#[derive(Debug, Clone)]
pub struct MultimodalSearchHit {
    pub chunk_id: Uuid,
    pub doc_id: Uuid,
    pub asset_id: Uuid,
    pub score: f32,
    pub page: Option<i64>,
    pub caption: Option<String>,
    pub context_text: String,
    pub image_path: Option<String>,
    pub parser_backend: String,
}

#[derive(Debug, Clone)]
pub struct MultimodalScoredChunk {
    pub chunk_id: Uuid,
    pub doc_id: Uuid,
    pub asset_id: Uuid,
    pub context_text: String,
    pub caption: Option<String>,
    pub score: f32,
    pub page: Option<i64>,
    pub image_path: Option<String>,
    pub chunk_type: String,
    pub parser_backend: String,
    pub source_locator: Option<Value>,
    pub source: String,
}

pub async fn run_multimodal_retrieval(
    qdrant: &HttpQdrantBackend,
    repo: &PgAppRepository,
    auth: &AuthContext,
    collection: &str,
    query_vector: Vec<f32>,
    doc_ids: Option<&[Uuid]>,
    limit: usize,
) -> anyhow::Result<Vec<MultimodalScoredChunk>> {
    let hits = qdrant
        .search_dense(collection, query_vector, auth.org_id(), doc_ids, limit)
        .await?;

    if hits.is_empty() {
        return Ok(Vec::new());
    }

    let chunk_ids: Vec<Uuid> = hits.iter().map(|h| h.chunk_id).collect();
    let chunk_map = repo.get_multimodal_chunks_by_ids(auth, &chunk_ids).await?;

    let asset_ids: Vec<Uuid> = chunk_map
        .values()
        .filter_map(|c| c.asset_id)
        .chain(hits.iter().filter_map(|h| {
            let chunk = chunk_map.get(&h.chunk_id);
            chunk.and_then(|c| c.asset_id).or(Some(h.chunk_id))
        }))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let asset_map = repo.get_document_assets_by_ids(auth, &asset_ids).await?;

    let mut scored_chunks = Vec::with_capacity(hits.len());
    for hit in hits {
        let Some(chunk) = chunk_map.get(&hit.chunk_id) else {
            tracing::warn!(chunk_id = %hit.chunk_id, "Multimodal Qdrant hit missing PG chunk row");
            continue;
        };

        let asset_id = chunk.asset_id.unwrap_or(hit.chunk_id);
        let Some(asset) = asset_map.get(&asset_id) else {
            tracing::warn!(chunk_id = %hit.chunk_id, asset_id = %asset_id, "Multimodal chunk missing asset row");
            continue;
        };

        scored_chunks.push(MultimodalScoredChunk {
            chunk_id: hit.chunk_id,
            doc_id: hit.doc_id,
            asset_id,
            context_text: chunk.normalized_text.clone(),
            caption: chunk.caption.clone().or(asset.caption.clone()),
            score: hit.score,
            page: chunk.page.map(i64::from).or(hit.page.map(|p| p as i64)),
            image_path: asset.storage_path.clone(),
            chunk_type: chunk_metadata_string(&chunk.metadata, "block_type")
                .unwrap_or_else(|| "image_with_context".to_string()),
            parser_backend: chunk.parser_backend.clone(),
            source_locator: chunk_metadata_value(&chunk.metadata, "source_locator"),
            source: "multimodal_dense".to_string(),
        });
    }

    Ok(scored_chunks)
}

fn chunk_metadata_string(metadata: &Value, key: &str) -> Option<String> {
    metadata
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn chunk_metadata_value(metadata: &Value, key: &str) -> Option<Value> {
    metadata.get(key).cloned().filter(|value| !value.is_null())
}
