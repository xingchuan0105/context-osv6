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

        // E1 (2026-07-28): the 'simple' tsvector config cannot tokenize CJK —
        // Chinese queries always scored 0. Route CJK queries to the pg_bigm
        // path (LIKE per term + similarity() score); ASCII stays on
        // tsvector/ts_rank. Same return shape and backend labels either way.
        let rows = if has_cjk(q) {
            self.search_bm25_cjk(q, owner, request.doc_ids.as_ref(), limit)
                .await?
        } else if let Some(doc_ids) = request.doc_ids.as_ref() {
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

    /// E1: CJK lexical path over pg_bigm — one `text LIKE '%term%'` per
    /// whitespace-split term (AND chain), scored by the sum of pg_trgm
    /// `similarity(text, term)` (float4, 0..1 per term). Mirrors the tsvector
    /// path's filters (owner / optional doc scope / limit) and row shape.
    /// NOTE: LIKE (not ILIKE) — the gin_bigm_ops opclass only accelerates the
    /// case-sensitive operator; CJK has no case, and embedded ASCII terms are
    /// matched as written (verified 2026-07-28: ILIKE seq-scans).
    async fn search_bm25_cjk(
        &self,
        q: &str,
        owner: Uuid,
        doc_ids: Option<&Vec<Uuid>>,
        limit: i64,
    ) -> anyhow::Result<Vec<TextChunkRow>> {
        let terms = split_terms(q);
        let (sql, patterns) = build_cjk_bm25_query(&terms, doc_ids.is_some());
        let mut query = sqlx::query_as::<_, TextChunkRow>(&sql);
        for pattern in &patterns {
            query = query.bind(pattern);
        }
        query = query.bind(owner);
        if let Some(ids) = doc_ids {
            query = query.bind(ids);
        }
        query = query.bind(limit);
        Ok(query.fetch_all(&self.pool).await?)
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
        // Figure dual-write (2026-08-04) stores asset_id/image_path/caption on source_locator.
        let (asset_id, image_path, caption) = figure_meta_from_locator(self.source_locator.as_ref());
        let cursor = avrag_retrieval_data_plane::cursor_from_value(self.source_locator.as_ref());
        ScoredChunk {
            chunk_id: self.chunk_id,
            doc_id: self.doc_id,
            content: self.text,
            score: self.score,
            source: channel.to_string(),
            page: self.page,
            chunk_type: self.chunk_type,
            asset_id,
            caption,
            image_path,
            parser_backend: self.parser_backend,
            source_locator: self.source_locator,
            parse_run_id: Some(self.parse_run_id),
            cursor,
            member_chunk_ids: vec![],
        }
    }
}

fn figure_meta_from_locator(
    locator: Option<&Value>,
) -> (Option<Uuid>, Option<String>, Option<String>) {
    let Some(obj) = locator.and_then(|v| v.as_object()) else {
        return (None, None, None);
    };
    let asset_id = obj
        .get("asset_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());
    let image_path = obj
        .get("image_path")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .filter(|s| !s.trim().is_empty());
    let caption = obj
        .get("caption")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .filter(|s| !s.trim().is_empty());
    (asset_id, image_path, caption)
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
            cursor: None,
            member_chunk_ids: vec![],
        }
    }
}


// ---------------------------------------------------------------------------
// E1: CJK lexical path helpers (pg_bigm LIKE + similarity)
// ---------------------------------------------------------------------------

/// Cap on LIKE terms per CJK query (keeps the dynamic SQL small; extra
/// whitespace-separated terms are dropped, most-specific first).
const MAX_CJK_TERMS: usize = 8;

/// True when the query contains any CJK ideograph (unified + ext-B +
/// compatibility). The tsvector 'simple' config cannot tokenize these, so
/// such queries must route to the pg_bigm LIKE path.
fn has_cjk(s: &str) -> bool {
    s.chars().any(|c| {
        matches!(c as u32, 0x2E80..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2A6DF)
    })
}

/// Escape LIKE metacharacters for `ESCAPE '\'` patterns.
fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Whitespace-split the query into terms (empties dropped).
fn split_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// Build the CJK bm25 query: `$1..$n` are the `%term%` LIKE patterns
/// (escaped), then owner, then optional doc_ids, then limit — matching the
/// bind order in `search_bm25_cjk`. Returns (sql, patterns).
fn build_cjk_bm25_query(terms: &[String], with_doc_ids: bool) -> (String, Vec<String>) {
    let n = terms.len().min(MAX_CJK_TERMS);
    let score = (1..=n)
        .map(|i| format!("similarity(text, ${i})"))
        .collect::<Vec<_>>()
        .join(" + ");
    let conds = (1..=n)
        .map(|i| format!("text LIKE ${i} ESCAPE '\\'"))
        .collect::<Vec<_>>()
        .join(" AND ");
    let owner_idx = n + 1;
    let limit_idx = if with_doc_ids { n + 3 } else { n + 2 };
    let mut sql = format!(
        "SELECT chunk_id, doc_id, text, page, chunk_type, parser_backend,\n\
         \x20      source_locator, parse_run_id,\n\
         \x20      ({score})::float4 AS score\n\
         FROM rag_text_chunks\n\
         WHERE owner_user_id = ${owner_idx}"
    );
    if with_doc_ids {
        sql.push_str(&format!("\n  AND doc_id = ANY(${})", n + 2));
    }
    sql.push_str(&format!(
        "\n  AND {conds}\nORDER BY score DESC\nLIMIT ${limit_idx}"
    ));
    let patterns = terms
        .iter()
        .take(n)
        .map(|t| format!("%{}%", escape_like(t)))
        .collect();
    (sql, patterns)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cjk_detection() {
        assert!(has_cjk("速冻机"));
        assert!(has_cjk("4R营销策略"));
        assert!(has_cjk("营销 strategy"));
        assert!(!has_cjk("salesforce FY25"));
        assert!(!has_cjk("4R"));
        assert!(!has_cjk(""));
    }

    #[test]
    fn like_escaping() {
        assert_eq!(escape_like("100%"), "100\\%");
        assert_eq!(escape_like("a_b"), "a\\_b");
        assert_eq!(escape_like("c\\d"), "c\\\\d");
        assert_eq!(escape_like("速冻机"), "速冻机");
    }

    #[test]
    fn term_splitting() {
        assert_eq!(split_terms("速冻机  年产 "), vec!["速冻机", "年产"]);
        assert_eq!(split_terms("营销"), vec!["营销"]);
        assert!(split_terms("   ").is_empty());
    }

    #[test]
    fn cjk_query_sql_and_binds() {
        let (sql, patterns) = build_cjk_bm25_query(
            &["速冻机".to_string(), "年产".to_string()],
            true,
        );
        assert_eq!(patterns, vec!["%速冻机%", "%年产%"]);
        // score sums both similarities; conditions AND-chained with ESCAPE.
        assert!(sql.contains("(similarity(text, $1) + similarity(text, $2))::float4 AS score"), "{sql}");
        assert!(sql.contains("text LIKE $1 ESCAPE '\\' AND text LIKE $2 ESCAPE '\\'"), "{sql}");
        // bind order: terms, owner($3), doc_ids($4), limit($5).
        assert!(sql.contains("owner_user_id = $3"), "{sql}");
        assert!(sql.contains("doc_id = ANY($4)"), "{sql}");
        assert!(sql.contains("LIMIT $5"), "{sql}");

        let (sql, _) = build_cjk_bm25_query(&["营销".to_string()], false);
        assert!(sql.contains("owner_user_id = $2"), "{sql}");
        assert!(sql.contains("LIMIT $3"), "{sql}");
        assert!(!sql.contains("doc_id = ANY"), "{sql}");
    }

    #[test]
    fn cjk_query_escapes_patterns_and_caps_terms() {
        let (sql, patterns) = build_cjk_bm25_query(&["100%_\\".to_string()], false);
        assert_eq!(patterns, vec!["%100\\%\\_\\\\%"]);
        assert!(sql.contains("text LIKE $1 ESCAPE '\\'"), "{sql}");

        let many: Vec<String> = (0..12).map(|i| format!("词{i}")).collect();
        let (sql, patterns) = build_cjk_bm25_query(&many, false);
        assert_eq!(patterns.len(), MAX_CJK_TERMS);
        assert!(sql.contains("owner_user_id = $9"), "{sql}");
        assert!(sql.contains("LIMIT $10"), "{sql}");
    }
}
