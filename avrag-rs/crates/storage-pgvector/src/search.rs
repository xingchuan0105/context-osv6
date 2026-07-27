use crate::{PgvectorDataPlane, owner_uuid};
use pgvector::Vector;
use avrag_retrieval_data_plane::{
    Bm25SearchOutput, Bm25SearchRequest, Bm25SearchTrace, MultimodalSearchRequest, ScoredChunk,
    TextDenseSearchRequest,
};
use contracts::auth_runtime::AuthContext;
use serde_json::Value;
use uuid::Uuid;

const LIST_QUERY_LIMIT: i64 = 16_384;

impl PgvectorDataPlane {
    pub(crate) async fn search_text_dense_impl(
        &self,
        request: TextDenseSearchRequest,
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        if request.query_vector.is_empty() || request.doc_ids.as_ref().is_some_and(Vec::is_empty) {
            return Ok(Vec::new());
        }
        let owner = owner_uuid(&request.auth);
        let dense = Vector::from(request.query_vector.clone());
        let limit = request.limit as i64;

        let rows = if let Some(doc_ids) = request.doc_ids.as_ref() {
            sqlx::query_as::<_, TextChunkRow>(
                r#"
                SELECT chunk_id, doc_id, text, page, chunk_type, parser_backend,
                       source_locator, parse_run_id,
                       (1.0 - (text_dense <=> $1))::float4 AS score
                FROM rag_text_chunks
                WHERE owner_user_id = $2 AND doc_id = ANY($3)
                ORDER BY text_dense <=> $1
                LIMIT $4
                "#,
            )
            .bind(&dense)
            .bind(owner)
            .bind(doc_ids)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, TextChunkRow>(
                r#"
                SELECT chunk_id, doc_id, text, page, chunk_type, parser_backend,
                       source_locator, parse_run_id,
                       (1.0 - (text_dense <=> $1))::float4 AS score
                FROM rag_text_chunks
                WHERE owner_user_id = $2
                ORDER BY text_dense <=> $1
                LIMIT $3
                "#,
            )
            .bind(&dense)
            .bind(owner)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|r| r.into_scored("pgvector_text_dense"))
            .collect())
    }

    pub(crate) async fn search_bm25_impl(
        &self,
        request: Bm25SearchRequest,
    ) -> anyhow::Result<Bm25SearchOutput> {
        if request.query.trim().is_empty() || request.doc_ids.as_ref().is_some_and(Vec::is_empty) {
            return Ok(Bm25SearchOutput {
                chunks: Vec::new(),
                trace: Bm25SearchTrace {
                    backend: "pgvector_fts".to_string(),
                    raw_hit_count: 0,
                    hydrated_hit_count: 0,
                    fallback_reason: None,
                },
            });
        }

        let owner = owner_uuid(&request.auth);
        let limit = request.limit as i64;
        let q = request.query.trim();

        let rows = if let Some(doc_ids) = request.doc_ids.as_ref() {
            sqlx::query_as::<_, TextChunkRow>(
                r#"
                SELECT chunk_id, doc_id, text, page, chunk_type, parser_backend,
                       source_locator, parse_run_id,
                       ts_rank(search_vector, plainto_tsquery('simple', $1))::float4 AS score
                FROM rag_text_chunks
                WHERE owner_user_id = $2
                  AND doc_id = ANY($3)
                  AND search_vector @@ plainto_tsquery('simple', $1)
                ORDER BY score DESC
                LIMIT $4
                "#,
            )
            .bind(q)
            .bind(owner)
            .bind(doc_ids)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, TextChunkRow>(
                r#"
                SELECT chunk_id, doc_id, text, page, chunk_type, parser_backend,
                       source_locator, parse_run_id,
                       ts_rank(search_vector, plainto_tsquery('simple', $1))::float4 AS score
                FROM rag_text_chunks
                WHERE owner_user_id = $2
                  AND search_vector @@ plainto_tsquery('simple', $1)
                ORDER BY score DESC
                LIMIT $3
                "#,
            )
            .bind(q)
            .bind(owner)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        let raw_hit_count = rows.len();
        let chunks: Vec<ScoredChunk> = rows
            .into_iter()
            .map(|r| r.into_scored("pgvector_fts"))
            .collect();
        let hydrated_hit_count = chunks.len();

        Ok(Bm25SearchOutput {
            chunks,
            trace: Bm25SearchTrace {
                backend: "pgvector_fts".to_string(),
                raw_hit_count,
                hydrated_hit_count,
                fallback_reason: None,
            },
        })
    }

    pub(crate) async fn search_multimodal_impl(
        &self,
        request: MultimodalSearchRequest,
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        if request.query_vector.is_empty() || request.doc_ids.as_ref().is_some_and(Vec::is_empty) {
            return Ok(Vec::new());
        }
        let owner = owner_uuid(&request.auth);
        let dense = Vector::from(request.query_vector.clone());
        let limit = request.limit as i64;

        let rows = if let Some(doc_ids) = request.doc_ids.as_ref() {
            sqlx::query_as::<_, MultimodalChunkRow>(
                r#"
                SELECT chunk_id, doc_id, context_text, caption, image_path, page, chunk_type,
                       parser_backend, source_locator, parse_run_id, asset_id, retrieval_weight,
                       (1.0 - (multimodal_dense <=> $1))::float4 AS score
                FROM rag_multimodal_chunks
                WHERE owner_user_id = $2 AND doc_id = ANY($3)
                ORDER BY multimodal_dense <=> $1
                LIMIT $4
                "#,
            )
            .bind(&dense)
            .bind(owner)
            .bind(doc_ids)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, MultimodalChunkRow>(
                r#"
                SELECT chunk_id, doc_id, context_text, caption, image_path, page, chunk_type,
                       parser_backend, source_locator, parse_run_id, asset_id, retrieval_weight,
                       (1.0 - (multimodal_dense <=> $1))::float4 AS score
                FROM rag_multimodal_chunks
                WHERE owner_user_id = $2
                ORDER BY multimodal_dense <=> $1
                LIMIT $3
                "#,
            )
            .bind(&dense)
            .bind(owner)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|r| r.into_scored("pgvector_multimodal_dense"))
            .collect())
    }

    pub(crate) async fn count_text_chunks_impl(
        &self,
        auth: &AuthContext,
        doc_ids: &[Uuid],
    ) -> anyhow::Result<usize> {
        if doc_ids.is_empty() {
            return Ok(0);
        }
        let owner = owner_uuid(auth);
        let (count,): (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)::bigint FROM rag_text_chunks
            WHERE owner_user_id = $1 AND doc_id = ANY($2)
            "#,
        )
        .bind(owner)
        .bind(doc_ids)
        .fetch_one(&self.pool)
        .await?;
        Ok(count as usize)
    }

    pub(crate) async fn list_text_chunks_impl(
        &self,
        auth: &AuthContext,
        doc_ids: &[Uuid],
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        if doc_ids.is_empty() {
            return Ok(Vec::new());
        }
        let owner = owner_uuid(auth);
        let rows = sqlx::query_as::<_, TextChunkRow>(
            r#"
            SELECT chunk_id, doc_id, text, page, chunk_type, parser_backend,
                   source_locator, parse_run_id,
                   0.0::float4 AS score
            FROM rag_text_chunks
            WHERE owner_user_id = $1 AND doc_id = ANY($2)
            LIMIT $3
            "#,
        )
        .bind(owner)
        .bind(doc_ids)
        .bind(LIST_QUERY_LIMIT)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| r.into_scored("pgvector_doc_scan"))
            .collect())
    }
}

#[derive(sqlx::FromRow)]
struct TextChunkRow {
    chunk_id: Uuid,
    doc_id: Uuid,
    text: String,
    page: Option<i64>,
    chunk_type: String,
    parser_backend: Option<String>,
    source_locator: Option<Value>,
    parse_run_id: Uuid,
    score: f32,
}

impl TextChunkRow {
    fn into_scored(self, channel: &str) -> ScoredChunk {
        ScoredChunk {
            chunk_id: self.chunk_id,
            doc_id: self.doc_id,
            content: self.text,
            score: self.score,
            source: channel.to_string(),
            page: self.page,
            chunk_type: self.chunk_type,
            asset_id: None,
            caption: None,
            image_path: None,
            parser_backend: self.parser_backend,
            source_locator: self.source_locator,
            parse_run_id: Some(self.parse_run_id),
        }
    }
}

#[derive(sqlx::FromRow)]
struct MultimodalChunkRow {
    chunk_id: Uuid,
    doc_id: Uuid,
    context_text: String,
    caption: Option<String>,
    image_path: Option<String>,
    page: Option<i64>,
    chunk_type: String,
    parser_backend: Option<String>,
    source_locator: Option<Value>,
    parse_run_id: Uuid,
    asset_id: Uuid,
    retrieval_weight: Option<f32>,
    score: f32,
}

impl MultimodalChunkRow {
    fn into_scored(self, channel: &str) -> ScoredChunk {
        let weight = self
            .retrieval_weight
            .filter(|w| *w > 0.0 && *w < 1.0);
        let score = weight.map(|w| self.score * w).unwrap_or(self.score);
        ScoredChunk {
            chunk_id: self.chunk_id,
            doc_id: self.doc_id,
            content: self.context_text,
            score,
            source: channel.to_string(),
            page: self.page,
            chunk_type: self.chunk_type,
            asset_id: Some(self.asset_id),
            caption: self.caption,
            image_path: self.image_path,
            parser_backend: self.parser_backend,
            source_locator: self.source_locator,
            parse_run_id: Some(self.parse_run_id),
        }
    }
}
