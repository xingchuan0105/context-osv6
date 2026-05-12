impl PgAppRepository {
    pub async fn clear_document_ir_projection(
        &self,
        context: &AuthContext,
        document_id: Uuid,
    ) -> Result<(), PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let guard = sqlx::query(
            r#"
            select 1
            from documents
            where id = $1
              and org_id = $2
              and status not in ('deleting', 'deleted')
            for update
            "#,
        )
        .bind(document_id)
        .bind(context.org_id().into_uuid())
        .fetch_optional(tx.inner())
        .await?;
        if guard.is_none() {
            tx.rollback().await?;
            return Err(PgStorageError::NotFound("document not found".to_string()));
        }

        sqlx::query("DELETE FROM document_multimodal_chunks WHERE org_id = $1 AND document_id = $2")
            .bind(context.org_id().into_uuid())
            .bind(document_id)
            .execute(tx.inner())
            .await?;
        sqlx::query("DELETE FROM document_assets WHERE org_id = $1 AND document_id = $2")
            .bind(context.org_id().into_uuid())
            .bind(document_id)
            .execute(tx.inner())
            .await?;
        sqlx::query("DELETE FROM document_blocks WHERE org_id = $1 AND document_id = $2")
            .bind(context.org_id().into_uuid())
            .bind(document_id)
            .execute(tx.inner())
            .await?;

        tx.commit().await?;
        Ok(())
    }
}

pub struct CreateDocumentParseRunParams<'a> {
    pub run_id: Uuid,
    pub notebook_id: Uuid,
    pub document_id: Uuid,
    pub backend_summary: &'a serde_json::Value,
    pub artifact_path: Option<&'a str>,
    pub task_id: &'a str,
    pub lock_token: Option<&'a str>,
}

pub struct FinishDocumentParseRunParams<'a> {
    pub run_id: Uuid,
    pub status: &'a str,
    pub backend_summary: &'a serde_json::Value,
    pub duration_ms: i64,
    pub warnings_json: &'a serde_json::Value,
    pub error_json: Option<&'a serde_json::Value>,
    pub artifact_path: Option<&'a str>,
    pub task_id: &'a str,
    pub lock_token: Option<&'a str>,
}

impl PgAppRepository {
    pub async fn create_document_parse_run(
        &self,
        context: &AuthContext,
        params: CreateDocumentParseRunParams<'_>,
    ) -> Result<(), PgStorageError> {
        let task_id = Uuid::parse_str(params.task_id)
            .map_err(|_| PgStorageError::NotFound("ingestion task not found".to_string()))?;
        let lock_token = params.lock_token
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or_else(|| PgStorageError::NotFound("ingestion task lease not found".to_string()))?;

        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            INSERT INTO document_parse_runs (
                run_id,
                org_id,
                notebook_id,
                document_id,
                status,
                backend_summary,
                artifact_path
            )
            SELECT $1, $2, $3, $4, 'running', $5, $6
            WHERE EXISTS (
                SELECT 1
                FROM documents d
                WHERE d.id = $4
                  AND d.org_id = $2
                  AND d.notebook_id = $3
                  AND d.status NOT IN ('deleting', 'deleted')
                FOR UPDATE
            )
              AND EXISTS (
                SELECT 1
                FROM ingestion_tasks it
                WHERE it.org_id = $2
                  AND it.document_id = $4
                  AND it.task_id = $7
                  AND it.lock_token = $8
                  AND it.status = 'processing'
                  AND it.dead_lettered_at IS NULL
            )
            RETURNING run_id
            "#,
        )
        .bind(params.run_id)
        .bind(context.org_id().into_uuid())
        .bind(params.notebook_id)
        .bind(params.document_id)
        .bind(params.backend_summary)
        .bind(params.artifact_path)
        .bind(task_id)
        .bind(lock_token)
        .fetch_optional(tx.inner())
        .await?;
        if row.is_none() {
            tx.rollback().await?;
            return Err(PgStorageError::NotFound("document not found".to_string()));
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn finish_document_parse_run(
        &self,
        context: &AuthContext,
        params: FinishDocumentParseRunParams<'_>,
    ) -> Result<(), PgStorageError> {
        let task_id = Uuid::parse_str(params.task_id)
            .map_err(|_| PgStorageError::NotFound("ingestion task not found".to_string()))?;
        let lock_token = params.lock_token
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or_else(|| PgStorageError::NotFound("ingestion task lease not found".to_string()))?;

        let mut tx = self.pool.begin(context).await?;
        let result = sqlx::query(
            r#"
            UPDATE document_parse_runs pr
            SET status = $2,
                backend_summary = $3,
                duration_ms = $4,
                warnings_json = $5,
                error_json = $6,
                artifact_path = COALESCE($7, artifact_path),
                updated_at = NOW()
            WHERE pr.run_id = $1
              AND pr.org_id = $8
              AND EXISTS (
                  SELECT 1
                  FROM documents d
                  WHERE d.id = pr.document_id
                    AND d.org_id = pr.org_id
                    AND d.notebook_id = pr.notebook_id
                    AND d.status NOT IN ('deleting', 'deleted')
                  FOR UPDATE
              )
              AND EXISTS (
                  SELECT 1
                  FROM ingestion_tasks it
                  WHERE it.org_id = pr.org_id
                    AND it.document_id = pr.document_id
                    AND it.task_id = $9
                    AND it.lock_token = $10
                    AND it.status = 'processing'
                    AND it.dead_lettered_at IS NULL
              )
            "#,
        )
        .bind(params.run_id)
        .bind(params.status)
        .bind(params.backend_summary)
        .bind(params.duration_ms)
        .bind(params.warnings_json)
        .bind(params.error_json)
        .bind(params.artifact_path)
        .bind(context.org_id().into_uuid())
        .bind(task_id)
        .bind(lock_token)
        .execute(tx.inner())
        .await?;
        if result.rows_affected() == 0 {
            tx.rollback().await?;
            return Err(PgStorageError::NotFound("document parse run not found".to_string()));
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn replace_document_blocks(
        &self,
        context: &AuthContext,
        notebook_id: Uuid,
        document_id: Uuid,
        blocks: &[StoredDocumentBlock],
    ) -> Result<(), PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let guard = sqlx::query(
            r#"
            select 1
            from documents
            where id = $1
              and org_id = $2
              and status not in ('deleting', 'deleted')
            for update
            "#,
        )
        .bind(document_id)
        .bind(context.org_id().into_uuid())
        .fetch_optional(tx.inner())
        .await?;
        if guard.is_none() {
            tx.rollback().await?;
            return Err(PgStorageError::NotFound("document not found".to_string()));
        }

        sqlx::query("DELETE FROM document_blocks WHERE org_id = $1 AND document_id = $2")
            .bind(context.org_id().into_uuid())
            .bind(document_id)
            .execute(tx.inner())
            .await?;

        for block in blocks {
            sqlx::query(
                r#"
                INSERT INTO document_blocks (
                    org_id,
                    notebook_id,
                    document_id,
                    parse_run_id,
                    block_id,
                    page,
                    block_type,
                    modality,
                    text,
                    summary_text,
                    caption,
                    asset_refs,
                    section_path,
                    source_locator_json,
                    parser_backend,
                    metadata_json
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
                "#,
            )
            .bind(context.org_id().into_uuid())
            .bind(notebook_id)
            .bind(document_id)
            .bind(block.parse_run_id)
            .bind(&block.block_id)
            .bind(block.page)
            .bind(&block.block_type)
            .bind(&block.modality)
            .bind(&block.text)
            .bind(&block.summary_text)
            .bind(&block.caption)
            .bind(&block.asset_refs)
            .bind(&block.section_path)
            .bind(&block.source_locator_json)
            .bind(&block.parser_backend)
            .bind(&block.metadata_json)
            .execute(tx.inner())
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}
