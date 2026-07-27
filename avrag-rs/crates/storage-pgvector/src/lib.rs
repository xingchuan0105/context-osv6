//! Postgres + pgvector retrieval data plane.
//!
//! Ports the Milvus multi-collection vector-graph RAG model onto five `rag_*`
//! tables (see migrations/0060_rag_pgvector). Public contracts match
//! `avrag-retrieval-data-plane` so worker / RagRuntime stay backend-agnostic.

mod config;
mod graph;
mod index;
mod search;

use async_trait::async_trait;
use avrag_retrieval_data_plane::{
    Bm25SearchOutput, Bm25SearchRequest, DocumentIndexBatch, GraphSearchOutput, GraphSearchRequest,
    IndexWriteReport, MultimodalSearchRequest, RetrievalDataPlane, RetrievalReadPort, ScoredChunk,
    TextDenseSearchRequest,
};
use contracts::auth_runtime::AuthContext;
use sqlx::PgPool;
use uuid::Uuid;

pub use config::PgvectorConfig;

/// Storage **channel_proxy** for graph-derived rows (same as storage-milvus).
/// Not evidence relevance — lexical graph-augment uses terms + TOP1 score-gap instead.
pub(crate) const GRAPH_CHUNK_SCORE: f32 = 0.85;

#[derive(Clone)]
pub struct PgvectorDataPlane {
    pool: PgPool,
    config: PgvectorConfig,
}

impl PgvectorDataPlane {
    pub fn new(pool: PgPool, config: PgvectorConfig) -> Self {
        Self { pool, config }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn config(&self) -> &PgvectorConfig {
        &self.config
    }
}

#[async_trait]
impl RetrievalReadPort for PgvectorDataPlane {
    async fn search_text_dense(
        &self,
        request: TextDenseSearchRequest,
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        self.search_text_dense_impl(request).await
    }

    async fn search_bm25(&self, request: Bm25SearchRequest) -> anyhow::Result<Bm25SearchOutput> {
        self.search_bm25_impl(request).await
    }

    async fn search_multimodal(
        &self,
        request: MultimodalSearchRequest,
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        self.search_multimodal_impl(request).await
    }

    async fn search_graph(&self, request: GraphSearchRequest) -> anyhow::Result<GraphSearchOutput> {
        self.search_graph_impl(request).await
    }

    async fn count_text_chunks(
        &self,
        auth: &AuthContext,
        doc_ids: &[Uuid],
    ) -> anyhow::Result<usize> {
        self.count_text_chunks_impl(auth, doc_ids).await
    }

    async fn list_text_chunks(
        &self,
        auth: &AuthContext,
        doc_ids: &[Uuid],
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        self.list_text_chunks_impl(auth, doc_ids).await
    }
}

#[async_trait]
impl RetrievalDataPlane for PgvectorDataPlane {
    async fn ensure_schema(&self) -> anyhow::Result<()> {
        // DDL lives in sqlx migrations (0060_rag_pgvector). Verify extension + tables.
        let ext: Option<(String,)> =
            sqlx::query_as("SELECT extname FROM pg_extension WHERE extname = 'vector'")
                .fetch_optional(&self.pool)
                .await?;
        if ext.is_none() {
            anyhow::bail!(
                "pgvector extension not installed; run migrations (CREATE EXTENSION vector) as a superuser"
            );
        }
        for table in [
            "rag_text_chunks",
            "rag_multimodal_chunks",
            "rag_kg_entities",
            "rag_kg_relations",
            "rag_graph_passages",
        ] {
            let exists: (bool,) = sqlx::query_as(
                "SELECT EXISTS (
                    SELECT 1 FROM information_schema.tables
                    WHERE table_schema = 'public' AND table_name = $1
                )",
            )
            .bind(table)
            .fetch_one(&self.pool)
            .await?;
            if !exists.0 {
                anyhow::bail!("missing table {table}; run AVRAG_RUN_MIGRATIONS / migrate");
            }
        }
        if let Some(ef) = self.config.hnsw_ef_search {
            // Best-effort session tune for ANN recall.
            let _ = sqlx::query(&format!("SET hnsw.ef_search = {ef}"))
                .execute(&self.pool)
                .await;
        }
        Ok(())
    }

    async fn replace_document_index(
        &self,
        batch: DocumentIndexBatch,
    ) -> anyhow::Result<IndexWriteReport> {
        self.replace_document_index_impl(batch).await
    }

    async fn delete_document_index(
        &self,
        auth: &AuthContext,
        document_id: Uuid,
    ) -> anyhow::Result<()> {
        self.delete_document_index_impl(auth, document_id).await
    }
}

pub(crate) fn validate_vector_dim(
    path: &str,
    actual: usize,
    expected: usize,
) -> anyhow::Result<()> {
    if actual == expected {
        return Ok(());
    }
    Err(anyhow::anyhow!(
        "vector dimension mismatch for {path}: expected {expected}, got {actual}"
    ))
}

pub(crate) fn owner_uuid(auth: &AuthContext) -> Uuid {
    *auth.user_id().uuid()
}

#[cfg(test)]
mod tests {
    use super::validate_vector_dim;

    #[test]
    fn dim_ok() {
        assert!(validate_vector_dim("v", 1024, 1024).is_ok());
    }

    #[test]
    fn dim_mismatch() {
        assert!(validate_vector_dim("v", 4, 1024).is_err());
    }
}
