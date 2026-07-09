use super::*;
pub fn pg_pool_options() -> PgPoolOptions {
    let mut options = PgPoolOptions::new();
    if std::env::var("E2E_ENABLED").unwrap_or_default() == "true" {
        // Real-LLM E2E runs API server + worker pools concurrently; default 10
        // connections per pool exhausts quickly and surfaces as 404/500 errors.
        options = options
            .max_connections(25)
            .acquire_timeout(std::time::Duration::from_secs(30));
    } else {
        options = options.max_connections(10);
    }
    options
}

impl BootstrapRepository {
    pub async fn connect(database_url: &str) -> Result<Self, PgStorageError> {
        let pool = pg_pool_options().connect(database_url).await?;
        Ok(Self {
            pool: TenantPgPool::new(pool),
        })
    }

    pub async fn migrate(&self) -> Result<(), PgStorageError> {
        let migrations_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
        let migrator = sqlx::migrate::Migrator::new(migrations_path.as_path()).await?;
        migrator.run(self.pool.raw()).await?;
        if std::env::var("AVRAG_SKIP_SEARCH_TOKEN_RESEGMENT")
            .map(|value| matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            ))
            .unwrap_or(false)
        {
            return Ok(());
        }
        let updated = ConversationMemoryRepository { pool: self.pool.clone() }.resegment_chat_message_search_tokens().await?;
        if updated > 0 {
            tracing::info!(
                updated_rows = updated,
                "resegmented chat_messages.search_tokens with jieba"
            );
        }
        Ok(())
    }

    pub async fn ping(&self) -> Result<(), PgStorageError> {
        sqlx::query("select 1").execute(self.pool.raw()).await?;
        Ok(())
    }

    pub fn raw(&self) -> &PgPool {
        self.pool.raw()
    }

    pub async fn get_workspace(
        &self,
        context: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<Option<Workspace>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            select id, org_id, owner_id, title, description, created_at, updated_at
            from workspaces
            where id = $1 and org_id = $2
            "#,
        )
        .bind(workspace_id)
        .bind(context.org_id().into_uuid())
        .fetch_optional(tx.inner())
        .await?;
        tx.commit().await?;
        row.map(map_notebook).transpose()
    }

    pub async fn create_workspace(
        &self,
        context: &AuthContext,
        name: &str,
        description: &str,
    ) -> Result<Workspace, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;
        let row = sqlx::query(
            r#"
            insert into workspaces (org_id, owner_id, title, description)
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

    pub async fn update_workspace(
        &self,
        context: &AuthContext,
        workspace_id: Uuid,
        name: &str,
        description: &str,
    ) -> Result<Option<Workspace>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            update workspaces
            set title = $2, description = $3, updated_at = now()
            where id = $1
            returning id, org_id, owner_id, title, description, created_at, updated_at
            "#,
        )
        .bind(workspace_id)
        .bind(name)
        .bind(description)
        .fetch_optional(tx.inner())
        .await?;
        tx.commit().await?;
        row.map(map_notebook).transpose()
    }

    pub async fn delete_workspace(
        &self,
        context: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<bool, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let result = sqlx::query("delete from workspaces where id = $1")
            .bind(workspace_id)
            .execute(tx.inner())
            .await?;
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn create_document(
        &self,
        context: &AuthContext,
        workspace_id: Uuid,
        filename: &str,
        file_size: u64,
        mime_type: &str,
    ) -> Result<Document, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;
        let document_id = Uuid::new_v4();
        let row = sqlx::query(
            r#"
            insert into documents (id, org_id, workspace_id, file_name, mime_type, file_size, status, chunk_count, object_path, user_id)
            values ($1, $2, $3, $4, $5, $6, 'pending', 0, $7, $8)
            returning id, org_id, workspace_id, file_name, mime_type, file_size, status, chunk_count, created_at, updated_at
            "#,
        )
        .bind(document_id)
        .bind(context.org_id().into_uuid())
        .bind(workspace_id)
        .bind(filename)
        .bind(mime_type)
        .bind(i64::try_from(file_size).unwrap_or(i64::MAX))
        .bind(build_object_path(context, workspace_id, document_id, filename))
        .bind(context.actor_id().map(ActorId::into_uuid))
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
            select id, org_id, workspace_id, file_name, mime_type, file_size, object_path, status
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
              and org_id = $4
              and status not in ('deleting', 'deleted')
            "#,
        )
        .bind(document_id)
        .bind(i64::try_from(content.len()).unwrap_or(i64::MAX))
        .bind(i32::try_from(body_chunks.len()).unwrap_or(i32::MAX))
        .bind(context.org_id().into_uuid())
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

    pub async fn replace_document_toc(
        &self,
        context: &AuthContext,
        workspace_id: Uuid,
        document_id: Uuid,
        entries: &[TocEntry],
    ) -> Result<(), PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;

        sqlx::query("delete from document_toc where document_id = $1")
            .bind(document_id)
            .execute(tx.inner())
            .await?;

        for entry in entries {
            sqlx::query(
                r#"
                insert into document_toc (id, org_id, document_id, workspace_id, parent_id, title, heading_level, page, chunk_id, rank)
                values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
            )
            .bind(entry.id)
            .bind(context.org_id().into_uuid())
            .bind(document_id)
            .bind(workspace_id)
            .bind(entry.parent_id)
            .bind(&entry.title)
            .bind(entry.heading_level)
            .bind(entry.page)
            .bind(entry.chunk_id)
            .bind(entry.rank)
            .execute(tx.inner())
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn get_document_toc_entries(
        &self,
        context: &AuthContext,
        doc_ids: &[Uuid],
    ) -> Result<Vec<(Uuid, TocEntry)>, PgStorageError> {
        if doc_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select document_id, id, parent_id, title, heading_level, page, chunk_id, rank
            from document_toc
            where document_id = any($1)
            order by document_id, rank
            "#,
        )
        .bind(doc_ids)
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;

        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let doc_id: Uuid = row.try_get("document_id").ok()?;
                let id: Uuid = row.try_get("id").ok()?;
                let parent_id: Option<Uuid> = row.try_get("parent_id").ok().flatten();
                let title: String = row.try_get("title").ok()?;
                let heading_level: i32 = row.try_get("heading_level").ok()?;
                let page: Option<i32> = row.try_get("page").ok().flatten();
                let chunk_id: Option<Uuid> = row.try_get("chunk_id").ok().flatten();
                let rank: i32 = row.try_get("rank").ok()?;
                Some((
                    doc_id,
                    TocEntry {
                        id,
                        parent_id,
                        title,
                        heading_level,
                        page,
                        chunk_id,
                        rank,
                    },
                ))
            })
            .collect())
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
