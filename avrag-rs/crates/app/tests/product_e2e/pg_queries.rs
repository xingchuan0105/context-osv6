//! PostgreSQL query helpers for product E2E assertions.

use uuid::Uuid;

use super::TestContext;

impl TestContext {
    /// Assert document ingestion reached `Completed` in PG.
    pub async fn assert_ingestion_completed(&self, document_id: &str) -> anyhow::Result<()> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let doc_id = Uuid::parse_str(document_id)?;
        let row: (String,) =
            sqlx::query_as("SELECT status FROM documents WHERE id = $1")
                .bind(doc_id)
                .fetch_one(&pool)
                .await?;
        anyhow::ensure!(
            row.0 == "completed",
            "expected document status completed, got {}",
            row.0
        );
        Ok(())
    }
    /// Query the chunk_count stored in PG for a completed document.
    pub async fn query_document_chunk_count(&self, document_id: &str) -> anyhow::Result<usize> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let doc_id = Uuid::parse_str(document_id)?;
        let row: (i32,) = sqlx::query_as("SELECT chunk_count FROM documents WHERE id = $1")
            .bind(doc_id)
            .fetch_one(&pool)
            .await?;
        Ok(row.0 as usize)
    }

    /// Latest parse run backend_summary JSON for a document.
    pub async fn query_latest_backend_summary(
        &self,
        document_id: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let doc_id = Uuid::parse_str(document_id)?;
        let row: (serde_json::Value,) = sqlx::query_as(
            "SELECT backend_summary FROM document_parse_runs WHERE document_id = $1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(doc_id)
        .fetch_one(&pool)
        .await?;
        Ok(row.0)
    }

    /// Count multimodal chunks whose stored metadata block_type is page_raster.
    pub async fn query_multimodal_page_raster_count(
        &self,
        document_id: &str,
    ) -> anyhow::Result<i64> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let doc_id = Uuid::parse_str(document_id)?;
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM document_multimodal_chunks WHERE document_id = $1 AND metadata->>'block_type' = 'page_raster'",
        )
        .bind(doc_id)
        .fetch_one(&pool)
        .await?;
        Ok(row.0)
    }

    /// MIME type stored on the document row after upload.
    pub async fn query_document_mime_type(&self, document_id: &str) -> anyhow::Result<String> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let doc_id = Uuid::parse_str(document_id)?;
        let row: (String,) =
            sqlx::query_as("SELECT mime_type FROM documents WHERE id = $1")
                .bind(doc_id)
                .fetch_one(&pool)
                .await?;
        Ok(row.0)
    }

    /// Multimodal chunks indexed from Paddle Figure blocks.
    pub async fn query_multimodal_figure_chunk_count(
        &self,
        document_id: &str,
    ) -> anyhow::Result<i64> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let doc_id = Uuid::parse_str(document_id)?;
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM document_multimodal_chunks WHERE document_id = $1 AND metadata->>'block_type' = 'figure'",
        )
        .bind(doc_id)
        .fetch_one(&pool)
        .await?;
        Ok(row.0)
    }

    /// Text body chunks plus multimodal/visual chunks (scan PDFs may have 0 text rows).
    pub async fn query_ingested_chunk_units(&self, document_id: &str) -> anyhow::Result<usize> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let doc_id = Uuid::parse_str(document_id)?;
        let text: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM chunks WHERE document_id = $1")
                .bind(doc_id)
                .fetch_one(&pool)
                .await?;
        let multimodal: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM document_multimodal_chunks WHERE document_id = $1",
        )
        .bind(doc_id)
        .fetch_one(&pool)
        .await?;
        Ok((text.0 + multimodal.0) as usize)
    }

    /// Return one chunk id from PG for mock codegen embedding.
    pub async fn query_first_chunk_id(&self, document_id: &str) -> anyhow::Result<String> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let doc_id = Uuid::parse_str(document_id)?;
        let row: (Uuid,) =
            sqlx::query_as("SELECT id FROM chunks WHERE document_id = $1 ORDER BY created_at LIMIT 1")
                .bind(doc_id)
                .fetch_one(&pool)
                .await?;
        Ok(row.0.to_string())
    }

    /// Return all chunk ids for a document (for bridge smoke assertions).
    pub async fn query_document_chunk_ids(&self, document_id: &str) -> anyhow::Result<Vec<String>> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let doc_id = Uuid::parse_str(document_id)?;
        let rows: Vec<(Uuid,)> =
            sqlx::query_as("SELECT id FROM chunks WHERE document_id = $1 ORDER BY created_at")
                .bind(doc_id)
                .fetch_all(&pool)
                .await?;
        Ok(rows.into_iter().map(|(id,)| id.to_string()).collect())
    }

    /// Read the latest user message content and resolved_query for a session.
    pub async fn query_latest_user_resolved_query(
        &self,
        session_id: &str,
    ) -> anyhow::Result<(String, Option<String>)> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let sid = Uuid::parse_str(session_id)?;
        let row: (String, Option<String>) = sqlx::query_as(
            "SELECT content, resolved_query FROM chat_messages \
             WHERE session_id = $1 AND role = 'user' \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(sid)
        .fetch_one(&pool)
        .await?;
        Ok(row)
    }

    /// Latest ingestion task row for debugging E2E failures.
    pub async fn query_ingestion_task_debug(
        &self,
        document_id: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let doc_id = Uuid::parse_str(document_id)?;
        let row: (String, i32, i32, Option<String>, Option<String>) = sqlx::query_as(
            r#"
            select status, attempt_count, max_attempts, last_error, locked_by
            from ingestion_tasks
            where document_id = $1
            order by enqueued_at desc
            limit 1
            "#,
        )
        .bind(doc_id)
        .fetch_one(&pool)
        .await?;
        Ok(serde_json::json!({
            "status": row.0,
            "attempt_count": row.1,
            "max_attempts": row.2,
            "last_error": row.3,
            "locked_by": row.4,
        }))
    }

    /// Override the ingestion task max_attempts for a document.
    pub async fn set_ingestion_max_attempts(
        &self,
        document_id: &str,
        max_attempts: i32,
    ) -> anyhow::Result<()> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let doc_id = Uuid::parse_str(document_id)?;
        sqlx::query(
            r#"
            update ingestion_tasks
            set max_attempts = $1,
                updated_at = now()
            where document_id = $2
            "#,
        )
        .bind(max_attempts.max(1))
        .bind(doc_id)
        .execute(&pool)
        .await?;
        Ok(())
    }
}
