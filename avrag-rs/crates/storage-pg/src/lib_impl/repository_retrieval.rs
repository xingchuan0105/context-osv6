impl PgAppRepository {
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
              and status not in ('deleting', 'deleted')
            "#,
        )
        .bind(doc_ids)
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
        notebook_id: Uuid,
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
            where d.notebook_id = $1
              and d.status not in ('deleting', 'deleted')
              and c.chunk_type = 'body'
              and c.search_vector @@ plainto_tsquery('simple', $2)
            order by rank desc, c.id
            limit $3
            "#,
        )
        .bind(notebook_id)
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

    pub async fn set_document_status(
        &self,
        context: &AuthContext,
        document_id: Uuid,
        status: DocumentStatus,
    ) -> Result<bool, PgStorageError> {
        if matches!(status, DocumentStatus::Deleting | DocumentStatus::Deleted) {
            return Ok(false);
        }
        let mut tx = self.pool.begin(context).await?;
        let result = sqlx::query(
            r#"
            update documents
            set status = $2, updated_at = now()
            where id = $1
              and status not in ('deleting', 'deleted')
            "#,
        )
        .bind(document_id)
        .bind(document_status_str(&status))
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn document_upload_is_mutable(
        &self,
        context: &AuthContext,
        document_id: Uuid,
    ) -> Result<Option<DocumentStatus>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query("select status from documents where id = $1")
            .bind(document_id)
            .fetch_optional(tx.inner())
            .await?;
        tx.commit().await?;
        row.map(|row| {
            let status: String = row.try_get("status")?;
            Ok(parse_document_status(&status))
        })
        .transpose()
    }

    pub async fn record_document_upload_validation(
        &self,
        context: &AuthContext,
        document_id: Uuid,
        size_bytes: u64,
        sha256_hex: Option<&str>,
    ) -> Result<DocumentUploadMutationOutcome, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query("select status from documents where id = $1 for update")
            .bind(document_id)
            .fetch_optional(tx.inner())
            .await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(DocumentUploadMutationOutcome::NotFound);
        };
        let status = parse_document_status(&row.try_get::<String, _>("status")?);
        if !document_upload_status_is_mutable(&status) {
            tx.commit().await?;
            return Ok(DocumentUploadMutationOutcome::StatusConflict(status));
        }

        sqlx::query(
            r#"
            update documents
            set upload_size_bytes = $2,
                upload_sha256 = $3,
                upload_validated_at = now(),
                upload_validation_error = null,
                updated_at = now()
            where id = $1
            "#,
        )
        .bind(document_id)
        .bind(i64::try_from(size_bytes).unwrap_or(i64::MAX))
        .bind(sha256_hex)
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(DocumentUploadMutationOutcome::Updated)
    }

    pub async fn set_document_upload_invalid(
        &self,
        context: &AuthContext,
        document_id: Uuid,
        validation_error: &str,
    ) -> Result<DocumentUploadMutationOutcome, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query("select status from documents where id = $1 for update")
            .bind(document_id)
            .fetch_optional(tx.inner())
            .await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(DocumentUploadMutationOutcome::NotFound);
        };
        let status = parse_document_status(&row.try_get::<String, _>("status")?);
        if !document_upload_status_is_mutable(&status) {
            tx.commit().await?;
            return Ok(DocumentUploadMutationOutcome::StatusConflict(status));
        }

        sqlx::query(
            r#"
            update documents
            set status = 'upload_invalid',
                upload_size_bytes = null,
                upload_sha256 = null,
                upload_validated_at = now(),
                upload_validation_error = $2,
                updated_at = now()
            where id = $1
            "#,
        )
        .bind(document_id)
        .bind(validation_error)
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(DocumentUploadMutationOutcome::Updated)
    }

    pub async fn get_document_upload_validation(
        &self,
        context: &AuthContext,
        document_id: Uuid,
    ) -> Result<Option<DocumentUploadValidation>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            select upload_size_bytes, upload_sha256, upload_validated_at, upload_validation_error
            from documents
            where id = $1
            "#,
        )
        .bind(document_id)
        .fetch_optional(tx.inner())
        .await?;
        tx.commit().await?;
        row.map(map_document_upload_validation).transpose()
    }

    pub async fn update_document(
        &self,
        context: &AuthContext,
        document_id: Uuid,
        filename: Option<&str>,
        notebook_id: Option<Uuid>,
        status: Option<DocumentStatus>,
    ) -> Result<bool, PgStorageError> {
        if matches!(
            status,
            Some(DocumentStatus::Deleting | DocumentStatus::Deleted)
        ) {
            return Ok(false);
        }
        let mut tx = self.pool.begin(context).await?;
        let status_text = status.as_ref().map(document_status_str);
        let result = sqlx::query(
            r#"
            update documents
            set file_name = coalesce($2, file_name),
                notebook_id = coalesce($3, notebook_id),
                status = coalesce($4, status),
                updated_at = now()
            where id = $1
              and status not in ('deleting', 'deleted')
            "#,
        )
        .bind(document_id)
        .bind(filename)
        .bind(notebook_id)
        .bind(status_text)
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_document(
        &self,
        context: &AuthContext,
        document_id: Uuid,
    ) -> Result<DocumentDeletionOutcome, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;
        let row = sqlx::query(
            r#"
            select id, org_id, notebook_id, file_name, mime_type, file_size, status, object_path
            from documents
            where id = $1
            for update
            "#,
        )
        .bind(document_id)
        .fetch_optional(tx.inner())
        .await?;

        let Some(row) = row else {
            tx.commit().await?;
            return Ok(DocumentDeletionOutcome::NotFound);
        };

        let org_id: Uuid = row.try_get("org_id")?;
        let notebook_id: Uuid = row.try_get("notebook_id")?;
        let status_text: String = row.try_get("status")?;
        let status = parse_document_status(&status_text);

        if matches!(status, DocumentStatus::Deleted) {
            tx.commit().await?;
            return Ok(DocumentDeletionOutcome::AlreadyDeleted);
        }

        let task_inserted = insert_document_cleanup_task(
            tx.inner(),
            org_id,
            notebook_id,
            document_id,
            context.actor_id().map(ActorId::into_uuid),
            &row,
        )
        .await?;

        if matches!(status, DocumentStatus::Deleting) {
            tx.commit().await?;
            return Ok(DocumentDeletionOutcome::AlreadyDeleting { task_inserted });
        }

        sqlx::query(
            r#"
            update documents
            set status = 'deleting',
                deletion_requested_at = coalesce(deletion_requested_at, now()),
                deletion_error = null,
                updated_at = now()
            where id = $1
            "#,
        )
        .bind(document_id)
        .execute(tx.inner())
        .await?;

        sqlx::query(
            r#"
            update ingestion_tasks
            set status = 'dead_letter',
                dead_lettered_at = coalesce(dead_lettered_at, now()),
                last_failed_at = coalesce(last_failed_at, now()),
                last_error = coalesce(last_error, 'document deletion requested'),
                locked_at = null,
                locked_by = null,
                lock_token = null,
                updated_at = now()
            where org_id = $1
              and document_id = $2
              and status in ('queued', 'processing')
              and dead_lettered_at is null
            "#,
        )
        .bind(org_id)
        .bind(document_id)
        .execute(tx.inner())
        .await?;

        tx.commit().await?;
        Ok(DocumentDeletionOutcome::Queued { task_inserted })
    }

    pub async fn count_document_cleanup_tasks_for_document(
        &self,
        context: &AuthContext,
        document_id: Uuid,
    ) -> Result<i64, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            select count(*)::bigint as task_count
            from document_cleanup_tasks
            where org_id = $1
              and document_id = $2
            "#,
        )
        .bind(context.org_id().into_uuid())
        .bind(document_id)
        .fetch_one(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(row.try_get("task_count")?)
    }

    pub async fn get_document_cleanup_targets(
        &self,
        context: &AuthContext,
        document_id: Uuid,
        task_payload: &serde_json::Value,
    ) -> Result<Option<DocumentCleanupTargets>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            select id, org_id, notebook_id, status, object_path
            from documents
            where id = $1
              and org_id = $2
              and status in ('deleting', 'deleted')
            "#,
        )
        .bind(document_id)
        .bind(context.org_id().into_uuid())
        .fetch_optional(tx.inner())
        .await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };
        let org_id: Uuid = row.try_get("org_id")?;
        let notebook_id: Uuid = row.try_get("notebook_id")?;

        let asset_rows = sqlx::query(
            r#"
            select storage_path
            from document_assets
            where org_id = $1
              and notebook_id = $2
              and document_id = $3
              and storage_path is not null
            order by created_at asc, asset_id asc
            "#,
        )
        .bind(org_id)
        .bind(notebook_id)
        .bind(document_id)
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;

        let object_path: Option<String> = row.try_get("object_path")?;
        let fallback_object_path = task_payload
            .get("object_path")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let status_text: String = row.try_get("status")?;
        Ok(Some(DocumentCleanupTargets {
            org_id,
            notebook_id,
            document_id: row.try_get("id")?,
            status: parse_document_status(&status_text),
            object_path: object_path.or(fallback_object_path),
            asset_storage_paths: asset_rows
                .into_iter()
                .filter_map(|row| {
                    row.try_get::<Option<String>, _>("storage_path")
                        .ok()
                        .flatten()
                })
                .collect(),
        }))
    }

    pub async fn cleanup_document_derived_rows(
        &self,
        context: &AuthContext,
        document_id: Uuid,
    ) -> Result<bool, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            select org_id, notebook_id
            from documents
            where id = $1
              and org_id = $2
              and status in ('deleting', 'deleted')
            for update
            "#,
        )
        .bind(document_id)
        .bind(context.org_id().into_uuid())
        .fetch_optional(tx.inner())
        .await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(false);
        };
        let org_id: Uuid = row.try_get("org_id")?;
        let notebook_id: Uuid = row.try_get("notebook_id")?;

        sqlx::query(
            "delete from document_multimodal_chunks where org_id = $1 and notebook_id = $2 and document_id = $3",
        )
        .bind(org_id)
        .bind(notebook_id)
        .bind(document_id)
        .execute(tx.inner())
        .await?;
        sqlx::query(
            "delete from document_assets where org_id = $1 and notebook_id = $2 and document_id = $3",
        )
        .bind(org_id)
        .bind(notebook_id)
        .bind(document_id)
        .execute(tx.inner())
        .await?;
        sqlx::query(
            "delete from document_blocks where org_id = $1 and notebook_id = $2 and document_id = $3",
        )
        .bind(org_id)
        .bind(notebook_id)
        .bind(document_id)
        .execute(tx.inner())
        .await?;
        sqlx::query("delete from chunks where org_id = $1 and document_id = $2")
            .bind(org_id)
            .bind(document_id)
            .execute(tx.inner())
            .await?;
        sqlx::query(
            "delete from document_parse_runs where org_id = $1 and notebook_id = $2 and document_id = $3",
        )
        .bind(org_id)
        .bind(notebook_id)
        .bind(document_id)
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn mark_document_deleted(
        &self,
        context: &AuthContext,
        document_id: Uuid,
    ) -> Result<bool, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let result = sqlx::query(
            r#"
            update documents
            set status = 'deleted',
                deleted_at = now(),
                deletion_error = null,
                updated_at = now()
            where id = $1
              and org_id = $2
              and status in ('deleting', 'deleted')
            "#,
        )
        .bind(document_id)
        .bind(context.org_id().into_uuid())
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn get_document_status(
        &self,
        context: &AuthContext,
        document_id: Uuid,
    ) -> Result<Option<DocumentStatus>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query("select status from documents where id = $1")
            .bind(document_id)
            .fetch_optional(tx.inner())
            .await?;
        tx.commit().await?;
        row.map(|row| {
            let status: String = row.try_get("status")?;
            Ok(parse_document_status(&status))
        })
        .transpose()
    }

    pub async fn set_document_status_for_ingestion_task(
        &self,
        context: &AuthContext,
        document_id: Uuid,
        status: DocumentStatus,
        task_id: &str,
        lock_token: Option<&str>,
    ) -> Result<bool, PgStorageError> {
        if matches!(status, DocumentStatus::Deleting | DocumentStatus::Deleted) {
            return Ok(false);
        }
        let Some(lock_token) = lock_token else {
            return Ok(false);
        };
        let task_id = match Uuid::parse_str(task_id) {
            Ok(value) => value,
            Err(_) => return Ok(false),
        };
        let lock_token = match Uuid::parse_str(lock_token) {
            Ok(value) => value,
            Err(_) => return Ok(false),
        };

        let mut tx = self.pool.begin(context).await?;
        let result = sqlx::query(
            r#"
            update documents d
            set status = $2, updated_at = now()
            where d.id = $1
              and d.org_id = $3
              and d.status not in ('deleting', 'deleted')
              and exists (
                  select 1
                  from ingestion_tasks it
                  where it.org_id = d.org_id
                    and it.document_id = d.id
                    and it.task_id = $4
                    and it.lock_token = $5
                    and it.status = 'processing'
                    and it.dead_lettered_at is null
              )
            "#,
        )
        .bind(document_id)
        .bind(document_status_str(&status))
        .bind(context.org_id().into_uuid())
        .bind(task_id)
        .bind(lock_token)
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn document_allows_ingestion_side_effects(
        &self,
        context: &AuthContext,
        document_id: Uuid,
        task_id: &str,
        lock_token: Option<&str>,
    ) -> Result<bool, PgStorageError> {
        let Some(lock_token) = lock_token else {
            return Ok(false);
        };
        let task_id = match Uuid::parse_str(task_id) {
            Ok(value) => value,
            Err(_) => return Ok(false),
        };
        let lock_token = match Uuid::parse_str(lock_token) {
            Ok(value) => value,
            Err(_) => return Ok(false),
        };

        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            select exists(
                select 1
                from documents d
                join ingestion_tasks it
                  on it.org_id = d.org_id
                 and it.document_id = d.id
                where d.org_id = $1
                  and d.id = $2
                  and d.status not in ('deleting', 'deleted')
                  and it.task_id = $3
                  and it.lock_token = $4
                  and it.status = 'processing'
                  and it.dead_lettered_at is null
            ) as allowed
            "#,
        )
        .bind(context.org_id().into_uuid())
        .bind(document_id)
        .bind(task_id)
        .bind(lock_token)
        .fetch_one(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(row.try_get("allowed")?)
    }

    pub async fn document_cleanup_task_lease_is_current(
        &self,
        task_id: Uuid,
        lock_token: Uuid,
    ) -> Result<bool, PgStorageError> {
        let mut tx = self.pool.raw().begin().await?;
        sqlx::query("select set_config('app.document_cleanup_worker', 'true', true)")
            .execute(tx.as_mut())
            .await?;
        let row = sqlx::query(
            r#"
            select exists(
                select 1
                from document_cleanup_tasks
                where task_id = $1
                  and lock_token = $2
                  and status = 'processing'
                  and dead_lettered_at is null
                  and completed_at is null
            ) as lease_current
            "#,
        )
        .bind(task_id)
        .bind(lock_token)
        .fetch_one(tx.as_mut())
        .await?;
        tx.commit().await?;
        Ok(row.try_get("lease_current")?)
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

    pub async fn update_document_summary(
        &self,
        context: &AuthContext,
        document_id: Uuid,
        summary: &common::SummaryOutput,
        task_id: Option<&str>,
        lock_token: Option<&str>,
    ) -> Result<(), PgStorageError> {
        let task_id = task_id
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|_| PgStorageError::NotFound("ingestion task not found".to_string()))?;
        let lock_token = lock_token
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|_| PgStorageError::NotFound("ingestion task lease not found".to_string()))?;
        if task_id.is_some() && lock_token.is_none() {
            return Err(PgStorageError::NotFound(
                "ingestion task lease not found".to_string(),
            ));
        }

        let mut tx = self.pool.begin(context).await?;
        let metadata = serde_json::to_value(&summary.summary_metadata).unwrap_or_default();
        let org_id = context.org_id().into_uuid();
        let result = sqlx::query(
            r#"
            update chunks c
            set content = $2, metadata = $3
            where c.document_id = $1
              and c.chunk_type = 'summary'
              and c.org_id = $4
              and exists (
                  select 1
                  from documents d
                  where d.id = c.document_id
                    and d.org_id = c.org_id
                    and d.status not in ('deleting', 'deleted')
                  for update
              )
              and (
                  $5::uuid is null
                  or exists (
                      select 1
                      from ingestion_tasks it
                      where it.org_id = c.org_id
                        and it.document_id = c.document_id
                        and it.task_id = $5
                        and it.lock_token = $6
                        and it.status = 'processing'
                        and it.dead_lettered_at is null
                  )
              )
            "#,
        )
        .bind(document_id)
        .bind(&summary.summary_text)
        .bind(&metadata)
        .bind(org_id)
        .bind(task_id)
        .bind(lock_token)
        .execute(tx.inner())
        .await?;

        if result.rows_affected() == 0 {
            let result = sqlx::query(
                r#"
                insert into chunks (id, org_id, document_id, chunk_type, content, metadata)
                select gen_random_uuid(), $4, $1, 'summary', $2, $3
                where exists (
                    select 1
                    from documents d
                    where d.id = $1
                      and d.org_id = $4
                      and d.status not in ('deleting', 'deleted')
                    for update
                )
                  and (
                      $5::uuid is null
                      or exists (
                          select 1
                          from ingestion_tasks it
                          where it.org_id = $4
                            and it.document_id = $1
                            and it.task_id = $5
                            and it.lock_token = $6
                            and it.status = 'processing'
                            and it.dead_lettered_at is null
                      )
                  )
                "#,
            )
            .bind(document_id)
            .bind(&summary.summary_text)
            .bind(&metadata)
            .bind(org_id)
            .bind(task_id)
            .bind(lock_token)
            .execute(tx.inner())
            .await?;
            if result.rows_affected() == 0 {
                tx.rollback().await?;
                return Err(PgStorageError::NotFound("document not found".to_string()));
            }
        }
        tx.commit().await?;
        Ok(())
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
            select metadata
            from chunks
            where document_id = any($1) and chunk_type = 'summary'
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
        notebook_id: Option<Uuid>,
    ) -> Result<Vec<SourceRow>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select d.id, d.notebook_id, n.title as notebook_name, d.file_name, d.status
            from documents d
            join notebooks n on n.id = d.notebook_id
            where ($1::uuid is null or d.notebook_id = $1)
              and d.status not in ('deleting', 'deleted')
            order by d.updated_at desc, d.created_at desc
            "#,
        )
        .bind(notebook_id)
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
                notebook_id: row
                    .try_get::<Uuid, _>("notebook_id")
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                notebook_name: row.try_get("notebook_name").unwrap_or_default(),
                title: row.try_get("file_name").unwrap_or_default(),
                file_name: row.try_get("file_name").unwrap_or_default(),
                status: row
                    .try_get("status")
                    .unwrap_or_else(|_| "pending".to_string()),
            })
            .collect())
    }
}

async fn insert_document_cleanup_task(
    tx: &mut PgConnection,
    org_id: Uuid,
    notebook_id: Uuid,
    document_id: Uuid,
    requested_by: Option<Uuid>,
    row: &PgRow,
) -> Result<bool, PgStorageError> {
    let file_name: String = row.try_get("file_name")?;
    let mime_type: Option<String> = row.try_get("mime_type")?;
    let file_size: i64 = row.try_get("file_size")?;
    let object_path: Option<String> = row.try_get("object_path")?;
    let status: String = row.try_get("status")?;
    let idempotency_key = format!("document-cleanup:{org_id}:{document_id}");
    let payload = json!({
        "org_id": org_id.to_string(),
        "notebook_id": notebook_id.to_string(),
        "document_id": document_id.to_string(),
        "file_name": file_name,
        "mime_type": mime_type.unwrap_or_default(),
        "file_size": u64::try_from(file_size).unwrap_or_default(),
        "object_path": object_path,
        "status_at_request": status,
    });

    let result = sqlx::query(
        r#"
        insert into document_cleanup_tasks (
            org_id, notebook_id, document_id, requested_by, idempotency_key, payload
        )
        values ($1, $2, $3, $4, $5, $6)
        on conflict (idempotency_key) do nothing
        "#,
    )
    .bind(org_id)
    .bind(notebook_id)
    .bind(document_id)
    .bind(requested_by)
    .bind(idempotency_key)
    .bind(payload)
    .execute(tx)
    .await?;

    Ok(result.rows_affected() > 0)
}
