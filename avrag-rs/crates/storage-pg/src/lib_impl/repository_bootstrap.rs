impl PgAppRepository {
    pub async fn connect(database_url: &str) -> Result<Self, PgStorageError> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;
        Ok(Self {
            pool: TenantPgPool::new(pool),
        })
    }

    pub async fn migrate(&self) -> Result<(), PgStorageError> {
        let migrations_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
        let migrator = sqlx::migrate::Migrator::new(migrations_path.as_path()).await?;
        migrator.run(self.pool.raw()).await?;
        Ok(())
    }

    pub async fn ping(&self) -> Result<(), PgStorageError> {
        sqlx::query("select 1").execute(self.pool.raw()).await?;
        Ok(())
    }

    pub fn raw(&self) -> &PgPool {
        self.pool.raw()
    }

    pub async fn get_notebook(
        &self,
        context: &AuthContext,
        notebook_id: Uuid,
    ) -> Result<Option<Notebook>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            select id, org_id, owner_id, title, description, created_at, updated_at
            from notebooks
            where id = $1
            "#,
        )
        .bind(notebook_id)
        .fetch_optional(tx.inner())
        .await?;
        tx.commit().await?;
        row.map(map_notebook).transpose()
    }

    pub async fn create_notebook(
        &self,
        context: &AuthContext,
        name: &str,
        description: &str,
    ) -> Result<Notebook, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;
        let row = sqlx::query(
            r#"
            insert into notebooks (org_id, owner_id, title, description)
            values ($1, $2, $3, $4)
            returning id, org_id, owner_id, title, description, created_at, updated_at
            "#,
        )
        .bind(context.org_id().into_uuid())
        .bind(context.actor_id().map(ActorId::into_uuid))
        .bind(name)
        .bind(description)
        .fetch_one(tx.inner())
        .await?;
        tx.commit().await?;
        map_notebook(row)
    }

    pub async fn update_notebook(
        &self,
        context: &AuthContext,
        notebook_id: Uuid,
        name: &str,
        description: &str,
    ) -> Result<Option<Notebook>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            update notebooks
            set title = $2, description = $3, updated_at = now()
            where id = $1
            returning id, org_id, owner_id, title, description, created_at, updated_at
            "#,
        )
        .bind(notebook_id)
        .bind(name)
        .bind(description)
        .fetch_optional(tx.inner())
        .await?;
        tx.commit().await?;
        row.map(map_notebook).transpose()
    }

    pub async fn delete_notebook(
        &self,
        context: &AuthContext,
        notebook_id: Uuid,
    ) -> Result<bool, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let result = sqlx::query("delete from notebooks where id = $1")
            .bind(notebook_id)
            .execute(tx.inner())
            .await?;
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn create_document(
        &self,
        context: &AuthContext,
        notebook_id: Uuid,
        filename: &str,
        file_size: u64,
        mime_type: &str,
    ) -> Result<Document, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;
        let document_id = Uuid::new_v4();
        let row = sqlx::query(
            r#"
            insert into documents (id, org_id, notebook_id, file_name, mime_type, file_size, status, chunk_count, object_path)
            values ($1, $2, $3, $4, $5, $6, 'pending', 0, $7)
            returning id, org_id, notebook_id, file_name, mime_type, file_size, status, chunk_count, created_at, updated_at
            "#,
        )
        .bind(document_id)
        .bind(context.org_id().into_uuid())
        .bind(notebook_id)
        .bind(filename)
        .bind(mime_type)
        .bind(i64::try_from(file_size).unwrap_or(i64::MAX))
        .bind(build_object_path(context, notebook_id, document_id, filename))
        .fetch_one(tx.inner())
        .await?;
        tx.commit().await?;
        map_document(row)
    }

    pub async fn get_document_task_seed(
        &self,
        context: &AuthContext,
        document_id: Uuid,
    ) -> Result<Option<DocumentTaskSeed>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            select id, org_id, notebook_id, file_name, mime_type, file_size, object_path
            from documents
            where id = $1
            "#,
        )
        .bind(document_id)
        .fetch_optional(tx.inner())
        .await?;
        tx.commit().await?;
        row.map(map_document_task_seed).transpose()
    }

    pub async fn store_document_body(
        &self,
        context: &AuthContext,
        document_id: Uuid,
        content: &str,
    ) -> Result<Vec<IndexedChunk>, PgStorageError> {
        let body_items = build_preview_items(content);
        self.store_document_body_items(context, document_id, None, content, &body_items)
            .await
    }

    pub async fn store_document_body_chunks(
        &self,
        context: &AuthContext,
        document_id: Uuid,
        parse_run_id: Option<Uuid>,
        content: &str,
        body_chunks: &[StoreDocumentChunkParams],
    ) -> Result<Vec<IndexedChunk>, PgStorageError> {
        let summary = build_summary(content);
        let mut indexed_chunks = Vec::new();

        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;
        let result = sqlx::query(
            r#"
            update documents
            set file_size = $2, chunk_count = $3, updated_at = now()
            where id = $1
            "#,
        )
        .bind(document_id)
        .bind(i64::try_from(content.len()).unwrap_or(i64::MAX))
        .bind(i32::try_from(body_chunks.len()).unwrap_or(i32::MAX))
        .execute(tx.inner())
        .await?;

        if result.rows_affected() == 0 {
            tx.rollback().await?;
            return Err(PgStorageError::NotFound("document not found".to_string()));
        }

        sqlx::query("delete from chunks where document_id = $1")
            .bind(document_id)
            .execute(tx.inner())
            .await?;

        for chunk in body_chunks {
            let row = sqlx::query(
                r#"
                insert into chunks (org_id, document_id, parse_run_id, chunk_type, page, content, metadata)
                values ($1, $2, $3, 'body', $4, $5, $6)
                returning id, document_id, page, content, metadata
                "#,
            )
            .bind(context.org_id().into_uuid())
            .bind(document_id)
            .bind(chunk.parse_run_id)
            .bind(chunk.page)
            .bind(&chunk.content)
            .bind(&chunk.metadata)
            .fetch_one(tx.inner())
            .await?;

            indexed_chunks.push(IndexedChunk {
                chunk_id: row
                    .try_get::<Uuid, _>("id")
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                doc_id: row
                    .try_get::<Uuid, _>("document_id")
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                page: row
                    .try_get::<Option<i32>, _>("page")
                    .ok()
                    .flatten()
                    .map(i64::from),
                content: row.try_get("content").unwrap_or_default(),
                score: None,
                metadata: row.try_get("metadata").unwrap_or_else(|_| json!({})),
            });
        }

        sqlx::query(
            r#"
            insert into chunks (org_id, document_id, parse_run_id, chunk_type, page, content, metadata)
            values ($1, $2, $3, 'summary', 1, $4, '{}'::jsonb)
            "#,
        )
        .bind(context.org_id().into_uuid())
        .bind(document_id)
        .bind(parse_run_id)
        .bind(summary)
        .execute(tx.inner())
        .await?;

        tx.commit().await?;
        Ok(indexed_chunks)
    }

    pub async fn store_document_body_items(
        &self,
        context: &AuthContext,
        document_id: Uuid,
        parse_run_id: Option<Uuid>,
        content: &str,
        body_items: &[ParsedPreviewItem],
    ) -> Result<Vec<IndexedChunk>, PgStorageError> {
        let body_chunks = body_items
            .iter()
            .map(|item| StoreDocumentChunkParams {
                parse_run_id,
                page: Some(i32::try_from(item.page).unwrap_or(1)),
                content: item.text.clone(),
                metadata: json!({
                    "kind": item.kind,
                    "cursor": item.cursor,
                    "page": item.page,
                }),
            })
            .collect::<Vec<_>>();

        self.store_document_body_chunks(context, document_id, parse_run_id, content, &body_chunks)
            .await
    }


}
