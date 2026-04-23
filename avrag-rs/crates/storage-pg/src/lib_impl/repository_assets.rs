impl PgAppRepository {
    pub async fn store_document_asset(
        &self,
        context: &AuthContext,
        params: StoreDocumentAssetParams,
    ) -> Result<DocumentAssetRow, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            INSERT INTO document_assets (asset_id, org_id, notebook_id, document_id, parse_run_id, page, asset_kind, storage_path, mime_type, width, height, caption, parser_backend)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING asset_id, org_id, notebook_id, document_id, parse_run_id, page, asset_kind, storage_path, mime_type, width, height, caption, parser_backend, created_at
            "#,
        )
        .bind(params.asset_id)
        .bind(context.org_id().into_uuid())
        .bind(params.notebook_id)
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
            INSERT INTO document_multimodal_chunks (chunk_id, org_id, notebook_id, document_id, parse_run_id, asset_id, page, context_text, caption, normalized_text, parser_backend, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING chunk_id, org_id, notebook_id, document_id, parse_run_id, asset_id, page, context_text, caption, normalized_text, parser_backend, metadata, created_at
            "#,
        )
        .bind(params.chunk_id)
        .bind(context.org_id().into_uuid())
        .bind(params.notebook_id)
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

    pub async fn get_document_asset_by_id(
        &self,
        context: &AuthContext,
        asset_id: Uuid,
    ) -> Result<Option<DocumentAssetRow>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            SELECT asset_id, org_id, notebook_id, document_id, parse_run_id, page, asset_kind, storage_path, mime_type, width, height, caption, parser_backend, created_at
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
            SELECT chunk_id, org_id, notebook_id, document_id, parse_run_id, asset_id, page, context_text, caption, normalized_text, parser_backend, metadata, created_at
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
        let rows = sqlx::query(
            r#"
            select id, document_id, page, content, metadata
            from chunks
            where id = any($1) and chunk_type = 'body'
            "#,
        )
        .bind(chunk_ids)
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;
        let mut map = HashMap::new();
        for row in rows {
            if let Ok(chunk) = map_indexed_chunk(row) {
                if let Ok(uuid) = Uuid::parse_str(&chunk.chunk_id) {
                    map.insert(uuid, chunk);
                }
            }
        }
        Ok(map)
    }

    pub async fn get_multimodal_chunks_by_ids(
        &self,
        context: &AuthContext,
        chunk_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, MultimodalChunkRow>, PgStorageError> {
        if chunk_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            SELECT chunk_id, org_id, notebook_id, document_id, parse_run_id, asset_id, page, context_text, caption, normalized_text, parser_backend, metadata, created_at
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
            SELECT asset_id, org_id, notebook_id, document_id, parse_run_id, page, asset_kind, storage_path, mime_type, width, height, caption, parser_backend, created_at
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
