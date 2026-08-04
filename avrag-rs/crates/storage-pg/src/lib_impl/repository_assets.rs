use super::*;

/// table_evidence 证据 chunk 入参（struct_query 表级证据；
/// 与 struct-supervision `EvidenceChunk` 同语义，跨 crate 解耦）。
#[derive(Debug, Clone)]
pub struct TableEvidenceChunkRow {
    pub chunk_id: Uuid,
    pub table: String,
    pub start_line: i64,
    pub n_rows: i64,
    pub md: String,
}

/// body chunk 的 md 源行区间（W6 行级证据；0-based 闭区间，
/// ingestion 侧键见 ingestion::ir `MD_LINE_START_KEY`/`MD_LINE_END_KEY`）。
#[derive(Debug, Clone)]
pub struct BodyChunkMdLineRow {
    pub chunk_id: Uuid,
    pub md_line_start: i64,
    pub md_line_end: i64,
}

impl AssetRepository {
    pub async fn store_document_asset(
        &self,
        context: &AuthContext,
        params: StoreDocumentAssetParams,
    ) -> Result<DocumentAssetRow, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            INSERT INTO document_assets (asset_id, owner_user_id, workspace_id, document_id, parse_run_id, page, asset_kind, storage_path, mime_type, width, height, caption, parser_backend)
            SELECT $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13
            WHERE EXISTS (
                SELECT 1
                FROM documents
                WHERE id = $4
                  AND owner_user_id = $2
                  AND status NOT IN ('deleting', 'deleted')
                FOR UPDATE
            )
            RETURNING asset_id, owner_user_id, workspace_id, document_id, parse_run_id, page, asset_kind, storage_path, mime_type, width, height, caption, parser_backend, created_at
            "#,
        )
        .bind(params.asset_id)
        .bind(context.user_id().into_uuid())
        .bind(params.workspace_id)
        .bind(params.document_id)
        .bind(params.parse_run_id)
        .bind(params.page)
        .bind(params.asset_kind)
        .bind(params.storage_path)
        .bind(params.mime_type)
        .bind(params.width)
        .bind(params.height)
        .bind(params.caption)
        .bind(params.parser_backend)
        .fetch_one(tx.inner())
        .await?;
        tx.commit().await?;
        map_document_asset(row)
    }

    pub async fn store_multimodal_chunk(
        &self,
        context: &AuthContext,
        params: StoreMultimodalChunkParams,
    ) -> Result<MultimodalChunkRow, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            INSERT INTO document_multimodal_chunks (chunk_id, owner_user_id, workspace_id, document_id, parse_run_id, asset_id, page, context_text, caption, normalized_text, parser_backend, metadata)
            SELECT $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12
            WHERE EXISTS (
                SELECT 1
                FROM documents
                WHERE id = $4
                  AND owner_user_id = $2
                  AND status NOT IN ('deleting', 'deleted')
                FOR UPDATE
            )
            RETURNING chunk_id, owner_user_id, workspace_id, document_id, parse_run_id, asset_id, page, context_text, caption, normalized_text, parser_backend, metadata, created_at
            "#,
        )
        .bind(params.chunk_id)
        .bind(context.user_id().into_uuid())
        .bind(params.workspace_id)
        .bind(params.document_id)
        .bind(params.parse_run_id)
        .bind(params.asset_id)
        .bind(params.page)
        .bind(params.context_text)
        .bind(params.caption)
        .bind(params.normalized_text)
        .bind(params.parser_backend)
        .bind(params.metadata)
        .fetch_one(tx.inner())
        .await?;
        tx.commit().await?;
        map_multimodal_chunk(row)
    }

    /// Refresh VLM/OCR-derived retrieval text after the row was first inserted.
    pub async fn update_multimodal_chunk_context_text(
        &self,
        context: &AuthContext,
        chunk_id: Uuid,
        context_text: &str,
    ) -> Result<(), PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let result = sqlx::query(
            r#"
            UPDATE document_multimodal_chunks
            SET context_text = $2
            WHERE chunk_id = $1
              AND owner_user_id = $3
            "#,
        )
        .bind(chunk_id)
        .bind(context_text)
        .bind(context.user_id().into_uuid())
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        if result.rows_affected() == 0 {
            return Err(PgStorageError::NotFound(format!(
                "multimodal chunk {chunk_id} not found for context_text update"
            )));
        }
        Ok(())
    }

    pub async fn get_document_asset_by_id(
        &self,
        context: &AuthContext,
        asset_id: Uuid,
    ) -> Result<Option<DocumentAssetRow>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            SELECT asset_id, owner_user_id, workspace_id, document_id, parse_run_id, page, asset_kind, storage_path, mime_type, width, height, caption, parser_backend, created_at
            FROM document_assets
            WHERE asset_id = $1
            "#,
        )
        .bind(asset_id)
        .fetch_optional(tx.inner())
        .await?;
        tx.commit().await?;
        row.map(map_document_asset).transpose()
    }

    pub async fn get_multimodal_chunk_by_id(
        &self,
        context: &AuthContext,
        chunk_id: Uuid,
    ) -> Result<Option<MultimodalChunkRow>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            SELECT chunk_id, owner_user_id, workspace_id, document_id, parse_run_id, asset_id, page, context_text, caption, normalized_text, parser_backend, metadata, created_at
            FROM document_multimodal_chunks
            WHERE chunk_id = $1
            "#,
        )
        .bind(chunk_id)
        .fetch_optional(tx.inner())
        .await?;
        tx.commit().await?;
        row.map(map_multimodal_chunk).transpose()
    }

    pub async fn get_chunks_by_ids(
        &self,
        context: &AuthContext,
        chunk_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, IndexedChunk>, PgStorageError> {
        if chunk_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut tx = self.pool.begin(context).await?;
        // table_evidence：struct_query 表级证据 chunk（整表 md）——仅 id 水合可见，
        // 不进任何检索/计数路径（那些路径按 chunk_type='body'/'summary' 过滤）。
        let rows = sqlx::query(
            r#"
            select id, document_id, page, content, metadata
            from chunks
            where id = any($1) and chunk_type in ('body', 'table_evidence')
            "#,
        )
        .bind(chunk_ids)
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;
        let mut map = HashMap::new();
        for row in rows {
            if let Ok(chunk) = map_indexed_chunk(row)
                && let Ok(uuid) = Uuid::parse_str(&chunk.chunk_id) {
                    map.insert(uuid, chunk);
                }
        }
        Ok(map)
    }

    /// table_evidence 证据 chunk 装载（struct_query 表级证据；
    /// `load_evidence_chunks.py` 的 Rust 化——S4 ingestion 挂接）。
    /// 幂等：先按 (document_id, 'table_evidence') 删除再插入；owner_user_id 取自
    /// documents 行（与 Python `INSERT ... SELECT ... FROM documents` 同语义，
    /// document 不存在则该行跳过）。返回插入行数。
    pub async fn replace_table_evidence_chunks(
        &self,
        context: &AuthContext,
        document_id: Uuid,
        chunks: &[TableEvidenceChunkRow],
    ) -> Result<usize, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        sqlx::query("DELETE FROM chunks WHERE document_id = $1 AND chunk_type = 'table_evidence'")
            .bind(document_id)
            .execute(tx.inner())
            .await?;
        let mut inserted = 0usize;
        for c in chunks {
            let meta = serde_json::json!({
                "source": "struct_query_pipeline",
                "table": c.table,
                "start_line": c.start_line,
                "n_rows": c.n_rows,
            });
            let res = sqlx::query(
                r#"
                INSERT INTO chunks (id, owner_user_id, document_id, chunk_type, content, metadata)
                SELECT $1, d.owner_user_id, d.id, 'table_evidence', $2, $3
                FROM documents d WHERE d.id = $4
                "#,
            )
            .bind(c.chunk_id)
            .bind(&c.md)
            .bind(meta)
            .bind(document_id)
            .execute(tx.inner())
            .await?;
            inserted += res.rows_affected() as usize;
        }
        tx.commit().await?;
        Ok(inserted)
    }

    /// W6 行级证据：按 document_id 列 body chunks 的
    /// (chunk_id, md_line_start, md_line_end)——worker `_line_map` 写入的数据源。
    /// 仅取 chunk_type='body' 且 block_metadata 下 md 行区间两键均非 NULL 的行
    /// （无键的老数据/非 markitdown 路径自然排除）；按 md_line_start 排序，
    /// 同 start 按 id 稳定序。
    pub async fn list_body_chunk_md_line_ranges(
        &self,
        context: &AuthContext,
        document_id: Uuid,
    ) -> Result<Vec<BodyChunkMdLineRow>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            SELECT id,
                   (metadata->'block_metadata'->>'md_line_start')::bigint AS md_line_start,
                   (metadata->'block_metadata'->>'md_line_end')::bigint   AS md_line_end
            FROM chunks
            WHERE document_id = $1
              AND chunk_type = 'body'
              AND metadata->'block_metadata'->>'md_line_start' IS NOT NULL
              AND metadata->'block_metadata'->>'md_line_end' IS NOT NULL
            ORDER BY md_line_start, id
            "#,
        )
        .bind(document_id)
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;
        rows.iter()
            .map(|row| {
                Ok(BodyChunkMdLineRow {
                    chunk_id: row.try_get("id")?,
                    md_line_start: row.try_get("md_line_start")?,
                    md_line_end: row.try_get("md_line_end")?,
                })
            })
            .collect()
    }

    pub async fn get_multimodal_chunks_by_ids(
        &self,
        context: &AuthContext,
        chunk_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, MultimodalChunkRow>, PgStorageError> {        if chunk_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            SELECT chunk_id, owner_user_id, workspace_id, document_id, parse_run_id, asset_id, page, context_text, caption, normalized_text, parser_backend, metadata, created_at
            FROM document_multimodal_chunks
            WHERE chunk_id = any($1)
            "#,
        )
        .bind(chunk_ids)
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;
        let mut map = HashMap::new();
        for row in rows {
            if let Ok(chunk) = map_multimodal_chunk(row) {
                map.insert(chunk.chunk_id, chunk);
            }
        }
        Ok(map)
    }

    pub async fn get_document_assets_by_ids(
        &self,
        context: &AuthContext,
        asset_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, DocumentAssetRow>, PgStorageError> {
        if asset_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            SELECT asset_id, owner_user_id, workspace_id, document_id, parse_run_id, page, asset_kind, storage_path, mime_type, width, height, caption, parser_backend, created_at
            FROM document_assets
            WHERE asset_id = any($1)
            ORDER BY created_at
            "#,
        )
        .bind(asset_ids)
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;
        let mut map = HashMap::new();
        for row in rows {
            let asset = map_document_asset(row)?;
            map.insert(asset.asset_id, asset);
        }
        Ok(map)
    }

}
