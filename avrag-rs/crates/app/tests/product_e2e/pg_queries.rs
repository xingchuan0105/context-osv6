//! PostgreSQL query helpers for product E2E assertions.

use uuid::Uuid;

use super::TestContext;

impl TestContext {
    /// Count `llm_usage_events` for a user with the given `usage_kind`.
    pub async fn count_llm_usage_events(
        &self,
        user_id: Uuid,
        owner_user_id: Uuid,
        usage_kind: &str,
    ) -> anyhow::Result<i64> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM llm_usage_events
            WHERE user_id = $1 AND owner_user_id = $2 AND usage_kind = $3
            "#,
        )
        .bind(user_id)
        .bind(owner_user_id)
        .bind(usage_kind)
        .fetch_one(&pool)
        .await?;
        Ok(row.0)
    }

    /// Assert document ingestion reached `Completed` in PG.
    pub async fn assert_ingestion_completed(&self, document_id: &str) -> anyhow::Result<()> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let doc_id = Uuid::parse_str(document_id)?;
        let row: (String,) = sqlx::query_as("SELECT status FROM documents WHERE id = $1")
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

    /// Wall-clock ingestion duration (documents.updated_at - created_at) in seconds.
    pub async fn query_document_ingest_duration_secs(
        &self,
        document_id: &str,
    ) -> anyhow::Result<f64> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let doc_id = Uuid::parse_str(document_id)?;
        let row: (f64,) = sqlx::query_as(
            "SELECT EXTRACT(EPOCH FROM (updated_at - created_at))::float8 FROM documents WHERE id = $1",
        )
        .bind(doc_id)
        .fetch_one(&pool)
        .await?;
        Ok(row.0)
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
        let row: (String,) = sqlx::query_as("SELECT mime_type FROM documents WHERE id = $1")
            .bind(doc_id)
            .fetch_one(&pool)
            .await?;
        Ok(row.0)
    }

    /// Object storage path stored on the document row after upload.
    pub async fn query_document_object_path(&self, document_id: &str) -> anyhow::Result<String> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let doc_id = Uuid::parse_str(document_id)?;
        let row: (String,) = sqlx::query_as("SELECT object_path FROM documents WHERE id = $1")
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
        let text: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM chunks WHERE document_id = $1")
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
        let row: (Uuid,) = sqlx::query_as(
            "SELECT id FROM chunks WHERE document_id = $1 ORDER BY created_at LIMIT 1",
        )
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

    /// Latest user message jieba search tokens for a session (post-0043 migration).
    pub async fn query_latest_user_search_tokens(
        &self,
        session_id: &str,
    ) -> anyhow::Result<Option<String>> {
        let pool = sqlx::PgPool::connect(&self.pg_url).await?;
        let sid = Uuid::parse_str(session_id)?;
        let row: Option<(Option<String>,)> = sqlx::query_as(
            "SELECT search_tokens FROM chat_messages \
             WHERE session_id = $1 AND role = 'user' \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(sid)
        .fetch_optional(&pool)
        .await?;
        Ok(row.and_then(|(tokens,)| tokens))
    }
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

    /// Grant the fixed realistic-corpus identity the internal **`e2e` plan**:
    /// rolling 5h/7d limits = 0 (unlimited in `UsageLimitService`), active
    /// subscription `plan_id=e2e`, and a user override belt-and-suspenders.
    ///
    /// Product checkout never lists `e2e` (only free/plus/pro). Monthly
    /// `quota_limits` has no `e2e` rows → `hard_limit=None` → monthly check
    /// always allows. Does **not** disable enforcement for other identities
    /// (quota_boundary tests still use free + unique users).
    ///
    /// Idempotent; safe to call every `realistic_corpus_full_eval` start.
    pub async fn grant_e2e_unlimited_quota(&self, user_id: &str) -> anyhow::Result<()> {
        use sqlx::Connection;
        let uid = Uuid::parse_str(user_id)?;
        // Single connection so set_config(super_admin) sticks for the whole grant.
        let mut conn = sqlx::PgConnection::connect(&self.pg_url).await?;
        sqlx::query("SELECT set_config('app.current_role', 'super_admin', false)")
            .execute(&mut conn)
            .await?;

        sqlx::query(
            r#"
            INSERT INTO usage_limit_plan_policies
                (plan_id, rolling_5h_limit_units, rolling_7d_limit_units, enabled)
            VALUES ('e2e', 0, 0, true)
            ON CONFLICT (plan_id) DO UPDATE
            SET rolling_5h_limit_units = 0,
                rolling_7d_limit_units = 0,
                enabled = true,
                updated_at = now()
            "#,
        )
        .execute(&mut conn)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO users (id, email)
            VALUES ($1, $2)
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(uid)
        .bind(format!("{user_id}@local.dev"))
        .execute(&mut conn)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO usage_limit_user_overrides
                (user_id, rolling_5h_limit_units, rolling_7d_limit_units, enabled)
            VALUES ($1, 0, 0, true)
            ON CONFLICT (user_id) DO UPDATE
            SET rolling_5h_limit_units = 0,
                rolling_7d_limit_units = 0,
                enabled = true,
                updated_at = now()
            "#,
        )
        .bind(uid)
        .execute(&mut conn)
        .await?;

        // Prefer flipping any existing active sub; else insert one.
        let updated = sqlx::query(
            r#"
            UPDATE subscriptions
            SET plan_id = 'e2e',
                status = 'active',
                current_period_end = now() + interval '10 years',
                updated_at = now()
            WHERE user_id = $1 AND status = 'active'
            "#,
        )
        .bind(uid)
        .execute(&mut conn)
        .await?
        .rows_affected();

        if updated == 0 {
            sqlx::query(
                r#"
                INSERT INTO subscriptions (
                    user_id, plan_id, status, billing_provider,
                    current_period_start, current_period_end, cancel_at_period_end
                ) VALUES (
                    $1, 'e2e', 'active', 'creem',
                    now() - interval '1 day', now() + interval '10 years', false
                )
                "#,
            )
            .bind(uid)
            .execute(&mut conn)
            .await?;
        }

        eprintln!(
            "[e2e-quota] granted plan=e2e + override 0/0 (unlimited rolling) to user={user_id}"
        );
        Ok(())
    }
}
