impl PgAppRepository {
    pub async fn clear_document_ir_projection(
        &self,
        context: &AuthContext,
        document_id: Uuid,
    ) -> Result<(), PgStorageError> {
        let mut tx = self.pool.begin(context).await?;

        sqlx::query("DELETE FROM document_multimodal_chunks WHERE document_id = $1")
            .bind(document_id)
            .execute(tx.inner())
            .await?;
        sqlx::query("DELETE FROM document_assets WHERE document_id = $1")
            .bind(document_id)
            .execute(tx.inner())
            .await?;
        sqlx::query("DELETE FROM document_blocks WHERE document_id = $1")
            .bind(document_id)
            .execute(tx.inner())
            .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn create_document_parse_run(
        &self,
        context: &AuthContext,
        run_id: Uuid,
        notebook_id: Uuid,
        document_id: Uuid,
        backend_summary: &serde_json::Value,
        artifact_path: Option<&str>,
    ) -> Result<(), PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        sqlx::query(
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
            VALUES ($1, $2, $3, $4, 'running', $5, $6)
            "#,
        )
        .bind(run_id)
        .bind(context.org_id().into_uuid())
        .bind(notebook_id)
        .bind(document_id)
        .bind(backend_summary)
        .bind(artifact_path)
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn finish_document_parse_run(
        &self,
        context: &AuthContext,
        run_id: Uuid,
        status: &str,
        backend_summary: &serde_json::Value,
        duration_ms: i64,
        warnings_json: &serde_json::Value,
        error_json: Option<&serde_json::Value>,
        artifact_path: Option<&str>,
    ) -> Result<(), PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        sqlx::query(
            r#"
            UPDATE document_parse_runs
            SET status = $2,
                backend_summary = $3,
                duration_ms = $4,
                warnings_json = $5,
                error_json = $6,
                artifact_path = COALESCE($7, artifact_path),
                updated_at = NOW()
            WHERE run_id = $1
            "#,
        )
        .bind(run_id)
        .bind(status)
        .bind(backend_summary)
        .bind(duration_ms)
        .bind(warnings_json)
        .bind(error_json)
        .bind(artifact_path)
        .execute(tx.inner())
        .await?;
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

        sqlx::query("DELETE FROM document_blocks WHERE document_id = $1")
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
