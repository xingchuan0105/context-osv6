use super::*;
impl ChunkRepository {
    pub async fn get_document_scope_states(
        &self,
        context: &AuthContext,
        doc_ids: &[Uuid],
    ) -> Result<Vec<DocumentScopeState>, PgStorageError> {
        if doc_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select id, status
            from documents
            where id = any($1)
              and owner_user_id = $2
              and status not in ('deleting', 'deleted')
            "#,
        )
        .bind(doc_ids)
        .bind(context.user_id().into_uuid())
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;

        rows.into_iter()
            .map(|row| {
                let document_id: Uuid = row.try_get("id")?;
                let status: String = row.try_get("status")?;
                Ok(DocumentScopeState {
                    document_id,
                    status: parse_document_status(&status),
                })
            })
            .collect()
    }

    pub async fn get_chunk_by_id(
        &self,
        context: &AuthContext,
        chunk_id: Uuid,
    ) -> Result<Option<IndexedChunk>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            select id, document_id, page, content, metadata
            from chunks
            where id = $1 and chunk_type = 'body'
            "#,
        )
        .bind(chunk_id)
        .fetch_optional(tx.inner())
        .await?;
        tx.commit().await?;
        row.map(map_indexed_chunk).transpose()
    }

    pub async fn search_chunks_text(
        &self,
        context: &AuthContext,
        workspace_id: Uuid,
        query: &str,
        limit: usize,
    ) -> Result<Vec<IndexedChunk>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select
              c.id,
              c.document_id,
              c.page,
              c.content,
              c.metadata,
              ts_rank_cd(c.search_vector, plainto_tsquery('simple', $2)) as rank
            from chunks c
            join documents d on d.id = c.document_id
            where d.workspace_id = $1
              and d.status not in ('deleting', 'deleted')
              and c.chunk_type = 'body'
              and c.search_vector @@ plainto_tsquery('simple', $2)
            order by rank desc, c.id
            limit $3
            "#,
        )
        .bind(workspace_id)
        .bind(query)
        .bind(i64::try_from(limit).unwrap_or(i64::MAX))
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;
        rows.into_iter().map(map_indexed_chunk).collect()
    }

    /// BM25-style full-text search on chunks
    pub async fn search_chunks_bm25(
        &self,
        ctx: &AuthContext,
        query: &str,
        doc_ids: Option<&[Uuid]>,
        limit: usize,
    ) -> Result<Vec<IndexedChunk>, PgStorageError> {
        let mut tx = self.pool.begin(ctx).await?;

        let rows = if let Some(ids) = doc_ids {
            if ids.is_empty() {
                tx.commit().await?;
                return Ok(vec![]);
            }
            sqlx::query(
                r#"
                select
                  c.id,
                  c.document_id,
                  c.page,
                  c.content,
                  c.metadata,
                  ts_rank(to_tsvector('simple', c.content), plainto_tsquery('simple', $1)) as rank
                from chunks c
                join documents d on d.id = c.document_id
                where c.document_id = any($2::uuid[])
                  and d.status not in ('deleting', 'deleted')
                  and c.chunk_type = 'body'
                  and to_tsvector('simple', c.content) @@ plainto_tsquery('simple', $1)
                order by rank desc, c.id
                limit $3
                "#,
            )
            .bind(query)
            .bind(ids)
            .bind(i64::try_from(limit).unwrap_or(i64::MAX))
            .fetch_all(tx.inner())
            .await?
        } else {
            sqlx::query(
                r#"
                select
                  c.id,
                  c.document_id,
                  c.page,
                  c.content,
                  c.metadata,
                  ts_rank(to_tsvector('simple', c.content), plainto_tsquery('simple', $1)) as rank
                from chunks c
                join documents d on d.id = c.document_id
                where c.chunk_type = 'body'
                  and d.status not in ('deleting', 'deleted')
                  and to_tsvector('simple', c.content) @@ plainto_tsquery('simple', $1)
                order by rank desc, c.id
                limit $2
                "#,
            )
            .bind(query)
            .bind(i64::try_from(limit).unwrap_or(i64::MAX))
            .fetch_all(tx.inner())
            .await?
        };

        tx.commit().await?;
        rows.into_iter().map(map_indexed_chunk).collect()
    }

    pub async fn get_document_content(
        &self,
        context: &AuthContext,
        document_id: Uuid,
    ) -> Result<Option<DocumentContentResponse>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select chunk_type, content, coalesce((metadata->>'cursor')::int, 0) as cursor_value
            from chunks c
            join documents d on d.id = c.document_id
            where c.document_id = $1
              and d.status not in ('deleting', 'deleted')
            order by
              case when c.chunk_type = 'summary' then 1 else 0 end,
              coalesce((c.metadata->>'cursor')::int, 0),
              c.id
            "#,
        )
        .bind(document_id)
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;

        if rows.is_empty() {
            return Ok(None);
        }

        let mut content_parts = Vec::new();
        let mut summary = None;
        for row in rows {
            let chunk_type: String = row.try_get("chunk_type")?;
            let content: String = row.try_get("content")?;
            if chunk_type == "summary" {
                summary = Some(content);
            } else {
                content_parts.push(content);
            }
        }

        Ok(Some(DocumentContentResponse {
            content: content_parts.join("\n"),
            summary,
        }))
    }

    pub async fn get_document_names(
        &self,
        context: &AuthContext,
        doc_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, String>, PgStorageError> {
        if doc_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select id, file_name
            from documents
            where id = any($1)
            "#,
        )
        .bind(doc_ids)
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;
        let mut map = HashMap::new();
        for row in rows {
            let id: Uuid = row.try_get("id")?;
            let file_name: String = row.try_get("file_name")?;
            map.insert(id, file_name);
        }
        Ok(map)
    }

    pub async fn get_document_metadata_by_ids(
        &self,
        context: &AuthContext,
        doc_ids: &[Uuid],
    ) -> Result<Vec<common::DocumentMetadata>, PgStorageError> {
        if doc_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select id, file_name, mime_type, file_size, status, chunk_count
            from documents
            where id = any($1)
            "#,
        )
        .bind(doc_ids)
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let id: Uuid = row.try_get("id").ok()?;
                let file_name: String = row.try_get("file_name").ok()?;
                let mime_type: Option<String> = row.try_get("mime_type").ok().flatten();
                let file_size: i64 = row.try_get("file_size").ok().unwrap_or(0);
                let status: String = row.try_get("status").ok().unwrap_or_default();
                let chunk_count: i32 = row.try_get("chunk_count").ok().unwrap_or(0);
                Some(common::DocumentMetadata {
                    doc_id: id.to_string(),
                    name: file_name,
                    mime_type: mime_type.unwrap_or_default(),
                    file_size: file_size as u64,
                    status: parse_document_status(&status),
                    chunk_count: chunk_count as usize,
                })
            })
            .collect())
    }

    pub async fn get_summary_chunks(
        &self,
        context: &AuthContext,
        doc_ids: &[Uuid],
    ) -> Result<Vec<(Uuid, String)>, PgStorageError> {
        if doc_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select document_id, content
            from chunks
            where document_id = any($1) and chunk_type = 'summary'
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
                let content: String = row.try_get("content").ok()?;
                Some((doc_id, content))
            })
            .collect())
    }

    pub async fn get_summary_metadata(
        &self,
        context: &AuthContext,
        doc_ids: &[Uuid],
    ) -> Result<Vec<common::SummaryMetadata>, PgStorageError> {
        if doc_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select distinct on (document_id) document_id, metadata
            from chunks
            where document_id = any($1)
              and chunk_type in ('profile', 'summary')
            order by document_id,
                     case chunk_type when 'profile' then 0 else 1 end
            "#,
        )
        .bind(doc_ids)
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;

        let mut results = Vec::new();
        for row in rows {
            let metadata_value: serde_json::Value = row.try_get("metadata")?;
            if let Ok(metadata) = serde_json::from_value::<common::SummaryMetadata>(metadata_value)
            {
                results.push(metadata);
            }
        }
        Ok(results)
    }

    pub async fn get_parsed_preview(
        &self,
        context: &AuthContext,
        document_id: Uuid,
        cursor: usize,
        limit: usize,
    ) -> Result<Option<ParsedPreviewResponse>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let summary_row = sqlx::query(
            r#"
            select c.content
            from chunks c
            join documents d on d.id = c.document_id
            where c.document_id = $1
              and c.chunk_type = 'summary'
              and d.status not in ('deleting', 'deleted')
            limit 1
            "#,
        )
        .bind(document_id)
        .fetch_optional(tx.inner())
        .await?;
        let rows = sqlx::query(
            r#"
            select
              content,
              coalesce((metadata->>'kind')::text, 'paragraph') as kind,
              coalesce((metadata->>'page')::int, 1) as page_value,
              coalesce((metadata->>'cursor')::int, 0) as cursor_value
            from chunks c
            join documents d on d.id = c.document_id
            where c.document_id = $1 and c.chunk_type = 'body'
              and d.status not in ('deleting', 'deleted')
            order by cursor_value, c.id
            offset $2
            limit $3
            "#,
        )
        .bind(document_id)
        .bind(i64::try_from(cursor).unwrap_or(i64::MAX))
        .bind(i64::try_from(limit + 1).unwrap_or(i64::MAX))
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;

        if rows.is_empty() && summary_row.is_none() {
            return Ok(None);
        }

        let mut items = rows
            .into_iter()
            .map(|row| ParsedPreviewItem {
                kind: row
                    .try_get::<String, _>("kind")
                    .unwrap_or_else(|_| "paragraph".to_string()),
                text: row.try_get::<String, _>("content").unwrap_or_default(),
                page: usize::try_from(row.try_get::<i32, _>("page_value").unwrap_or(1))
                    .unwrap_or(1),
                cursor: usize::try_from(row.try_get::<i32, _>("cursor_value").unwrap_or(0))
                    .unwrap_or_default(),
            })
            .collect::<Vec<_>>();

        let has_more = items.len() > limit;
        if has_more {
            items.truncate(limit);
        }

        Ok(Some(ParsedPreviewResponse {
            items,
            has_more,
            next_cursor: cursor.saturating_add(limit),
            summary: summary_row.and_then(|row| row.try_get::<String, _>("content").ok()),
        }))
    }

    pub async fn list_sources(
        &self,
        context: &AuthContext,
        workspace_id: Option<Uuid>,
    ) -> Result<Vec<SourceRow>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select d.id, d.workspace_id, n.title as workspace_name, d.file_name, d.status,
                   (
                     select t.last_error
                     from ingestion_tasks t
                     where t.document_id = d.id
                       and t.last_error is not null
                       and length(trim(t.last_error)) > 0
                     order by t.updated_at desc nulls last
                     limit 1
                   ) as last_error
            from documents d
            join workspaces n on n.id = d.workspace_id
            where ($1::uuid is null or d.workspace_id = $1)
              and d.status not in ('deleting', 'deleted')
            order by d.updated_at desc, d.created_at desc
            "#,
        )
        .bind(workspace_id)
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;

        Ok(rows
            .into_iter()
            .map(|row| SourceRow {
                id: row
                    .try_get::<Uuid, _>("id")
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                workspace_id: row
                    .try_get::<Uuid, _>("workspace_id")
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                workspace_name: row.try_get("workspace_name").unwrap_or_default(),
                title: row.try_get("file_name").unwrap_or_default(),
                file_name: row.try_get("file_name").unwrap_or_default(),
                status: row
                    .try_get("status")
                    .unwrap_or_else(|_| "pending".to_string()),
                last_error: row
                    .try_get::<Option<String>, _>("last_error")
                    .ok()
                    .flatten()
                    .filter(|value| !value.trim().is_empty()),
            })
            .collect())
    }
}
