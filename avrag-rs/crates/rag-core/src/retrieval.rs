use avrag_auth::AuthContext;
use avrag_storage_pg::PgAppRepository;
use avrag_storage_qdrant::HttpQdrantBackend;
use serde::{Deserialize, Serialize};
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
        }
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
            scored_chunks.push(ScoredChunk::new_text(
                hit.chunk_id,
                hit.doc_id,
                chunk.content.clone(),
                hit.score,
                "dense".to_string(),
                hit.page.map(|p| p as i64),
            ));
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
            chunk_type: "text".to_string(),
            asset_id: None,
            caption: None,
            image_path: None,
            parser_backend: None,
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
    pub parser_backend: String,
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
            parser_backend: chunk.parser_backend.clone(),
            source: "multimodal_dense".to_string(),
        });
    }

    Ok(scored_chunks)
}
