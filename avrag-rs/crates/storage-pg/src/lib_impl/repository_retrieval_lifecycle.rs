use super::*;
impl PgAppRepository {
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
        let metadata = serde_json::json!({});
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

    pub async fn update_document_profile(
        &self,
        context: &AuthContext,
        document_id: Uuid,
        profile: &common::SummaryMetadata,
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
        let metadata = serde_json::to_value(profile).unwrap_or_default();
        let org_id = context.org_id().into_uuid();
        let result = sqlx::query(
            r#"
            update chunks c
            set metadata = $2
            where c.document_id = $1
              and c.chunk_type = 'profile'
              and c.org_id = $3
              and exists (
                  select 1
                  from documents d
                  where d.id = c.document_id
                    and d.org_id = c.org_id
                    and d.status not in ('deleting', 'deleted')
                  for update
              )
              and (
                  $4::uuid is null
                  or exists (
                      select 1
                      from ingestion_tasks it
                      where it.org_id = c.org_id
                        and it.document_id = c.document_id
                        and it.task_id = $4
                        and it.lock_token = $5
                        and it.status = 'processing'
                        and it.dead_lettered_at is null
                  )
              )
            "#,
        )
        .bind(document_id)
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
                select gen_random_uuid(), $3, $1, 'profile', '', $2
                where exists (
                    select 1
                    from documents d
                    where d.id = $1
                      and d.org_id = $3
                      and d.status not in ('deleting', 'deleted')
                    for update
                )
                  and (
                      $4::uuid is null
                      or exists (
                          select 1
                          from ingestion_tasks it
                          where it.org_id = $3
                            and it.document_id = $1
                            and it.task_id = $4
                            and it.lock_token = $5
                            and it.status = 'processing'
                            and it.dead_lettered_at is null
                      )
                  )
                "#,
            )
            .bind(document_id)
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

}
