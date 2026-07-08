use super::*;
pub(crate) const STALE_PROCESSING_TIMEOUT_SECS: i32 = 30 * 60;
const RETRY_BACKOFF_BASE_SECS: i32 = 30;
const RETRY_BACKOFF_MAX_SECS: i32 = 60 * 60;

fn retry_backoff_seconds(attempt_count: i32) -> i32 {
    let exponent = attempt_count.saturating_sub(1).clamp(0, 7) as u32;
    let seconds = RETRY_BACKOFF_BASE_SECS.saturating_mul(1_i32 << exponent);
    seconds.clamp(RETRY_BACKOFF_BASE_SECS, RETRY_BACKOFF_MAX_SECS)
}

fn ingestion_retry_backoff_seconds(attempt_count: i32) -> i32 {
    retry_backoff_seconds(attempt_count)
}

fn ingestion_queue_group_from_env() -> String {
    std::env::var("AVRAG_INGESTION_QUEUE_GROUP").unwrap_or_else(|_| "default".to_string())
}

impl PgAppRepository {
    pub async fn enqueue_ingestion_task(
        &self,
        task: &IngestionTask,
    ) -> Result<bool, PgStorageError> {
        let queue_group = ingestion_queue_group_from_env();
        let org_id = OrgId::from(
            Uuid::parse_str(&task.org_id)
                .map_err(|_| PgStorageError::NotFound("invalid task org id".to_string()))?,
        );
        let actor_id = task
            .requested_by
            .as_deref()
            .and_then(|value| Uuid::parse_str(value).ok())
            .map(ActorId::new);
        let context = if let Some(actor_id) = actor_id {
            AuthContext::new(org_id, avrag_auth::SubjectKind::User).with_actor_id(actor_id)
        } else {
            AuthContext::new(org_id, avrag_auth::SubjectKind::System)
        };

        let mut tx = self.pool.begin(&context).await?;
        ensure_org_and_actor(tx.inner(), &context).await?;
        let result = sqlx::query(
            r#"
            insert into ingestion_tasks (
                task_id, org_id, notebook_id, document_id, kind, requested_by, idempotency_key,
                queue_group, payload, status, attempt_count, max_attempts, available_at, enqueued_at, updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'queued', $10, $11, now(), $12, now())
            on conflict (idempotency_key) do nothing
            "#,
        )
        .bind(
            Uuid::parse_str(&task.task_id)
                .map_err(|_| PgStorageError::NotFound("invalid task id".to_string()))?,
        )
        .bind(org_id.into_uuid())
        .bind(
            Uuid::parse_str(&task.notebook_id)
                .map_err(|_| PgStorageError::NotFound("invalid notebook id".to_string()))?,
        )
        .bind(
            Uuid::parse_str(&task.document_id)
                .map_err(|_| PgStorageError::NotFound("invalid document id".to_string()))?,
        )
        .bind(ingestion_kind_str(&task.kind))
        .bind(
            task.requested_by
                .as_deref()
                .and_then(|value| Uuid::parse_str(value).ok()),
        )
        .bind(&task.idempotency_key)
        .bind(&queue_group)
        .bind(serde_json::to_value(&task.payload)?)
        .bind(task.attempt_count.max(0))
        .bind(task.max_attempts.max(1))
        .bind(parse_rfc3339(&task.enqueued_at)?)
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn queue_validated_document_upload(
        &self,
        context: &AuthContext,
        document_id: Uuid,
        size_bytes: u64,
        sha256_hex: Option<&str>,
        task: &IngestionTask,
    ) -> Result<DocumentUploadQueueOutcome, PgStorageError> {
        let queue_group = ingestion_queue_group_from_env();
        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;

        let row = sqlx::query("select status from documents where id = $1 for update")
            .bind(document_id)
            .fetch_optional(tx.inner())
            .await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(DocumentUploadQueueOutcome::NotFound);
        };
        let status = parse_document_status(&row.try_get::<String, _>("status")?);
        if !document_upload_status_is_mutable(&status) {
            tx.commit().await?;
            return Ok(DocumentUploadQueueOutcome::StatusConflict(status));
        }

        sqlx::query(
            r#"
            update documents
            set status = 'queued',
                upload_size_bytes = $2,
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

        let org_id = Uuid::parse_str(&task.org_id)
            .map_err(|_| PgStorageError::NotFound("invalid task org id".to_string()))?;
        if org_id != context.org_id().into_uuid() {
            return Err(PgStorageError::NotFound("invalid task org id".to_string()));
        }
        let result = sqlx::query(
            r#"
            insert into ingestion_tasks (
                task_id, org_id, notebook_id, document_id, kind, requested_by, idempotency_key,
                queue_group, payload, status, attempt_count, max_attempts, available_at, enqueued_at, updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'queued', $10, $11, now(), $12, now())
            on conflict (idempotency_key) do nothing
            "#,
        )
        .bind(
            Uuid::parse_str(&task.task_id)
                .map_err(|_| PgStorageError::NotFound("invalid task id".to_string()))?,
        )
        .bind(org_id)
        .bind(
            Uuid::parse_str(&task.notebook_id)
                .map_err(|_| PgStorageError::NotFound("invalid notebook id".to_string()))?,
        )
        .bind(
            Uuid::parse_str(&task.document_id)
                .map_err(|_| PgStorageError::NotFound("invalid document id".to_string()))?,
        )
        .bind(ingestion_kind_str(&task.kind))
        .bind(
            task.requested_by
                .as_deref()
                .and_then(|value| Uuid::parse_str(value).ok()),
        )
        .bind(&task.idempotency_key)
        .bind(&queue_group)
        .bind(serde_json::to_value(&task.payload)?)
        .bind(task.attempt_count.max(0))
        .bind(task.max_attempts.max(1))
        .bind(parse_rfc3339(&task.enqueued_at)?)
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(DocumentUploadQueueOutcome::Queued {
            task_inserted: result.rows_affected() > 0,
        })
    }

    pub async fn count_ingestion_tasks_for_document(
        &self,
        context: &AuthContext,
        document_id: Uuid,
    ) -> Result<i64, PgStorageError> {
        let row = sqlx::query(
            r#"
            select count(*)::bigint as task_count
            from ingestion_tasks
            where org_id = $1 and document_id = $2
            "#,
        )
        .bind(context.org_id().into_uuid())
        .bind(document_id)
        .fetch_one(self.pool.raw())
        .await?;
        Ok(row.try_get("task_count")?)
    }

    pub async fn claim_next_ingestion_task(
        &self,
        worker_id: &str,
        worker_queue_group: &str,
    ) -> Result<Option<IngestionTask>, PgStorageError> {
        let row = sqlx::query(
            r#"
            with exhausted_tasks as (
                update ingestion_tasks
                set status = 'dead_letter',
                    dead_lettered_at = coalesce(dead_lettered_at, now()),
                    last_failed_at = coalesce(last_failed_at, now()),
                    last_error = coalesce(last_error, 'ingestion task exhausted max attempts'),
                    locked_at = null,
                    locked_by = null,
                    lock_token = null,
                    updated_at = now()
                where dead_lettered_at is null
                  and status <> 'dead_letter'
                  and attempt_count >= max_attempts
                  and (
                    (status = 'queued' and available_at <= now())
                    or (status = 'processing' and locked_at <= now() - ($3::int * interval '1 second'))
                  )
                returning task_id
            ),
            next_task as (
                select task_id
                from ingestion_tasks
                where dead_lettered_at is null
                  and queue_group = $2
                  and status <> 'dead_letter'
                  and attempt_count < max_attempts
                  and (
                    (status = 'queued' and available_at <= now() and locked_at is null)
                    or (status = 'processing' and locked_at <= now() - ($3::int * interval '1 second'))
                  )
                order by case when status = 'processing' then 0 else 1 end, enqueued_at asc
                limit 1
                for update skip locked
            )
            update ingestion_tasks it
            set status = 'processing',
                locked_at = now(),
                locked_by = $1,
                lock_token = gen_random_uuid(),
                attempt_count = attempt_count + 1,
                updated_at = now()
            from next_task
            where it.task_id = next_task.task_id
            returning it.task_id, it.org_id, it.notebook_id, it.document_id, it.kind, it.requested_by,
                      it.idempotency_key, it.enqueued_at, it.payload, it.lock_token,
                      it.attempt_count, it.max_attempts
            "#,
        )
        .bind(worker_id)
        .bind(worker_queue_group)
        .bind(STALE_PROCESSING_TIMEOUT_SECS)
        .fetch_optional(self.pool.raw())
        .await?;
        row.map(map_ingestion_task).transpose()
    }

    pub async fn renew_ingestion_task_lock(
        &self,
        task_id: &str,
        lock_token: &str,
    ) -> Result<bool, PgStorageError> {
        let task_id = Uuid::parse_str(task_id)
            .map_err(|_| PgStorageError::NotFound("invalid task id".to_string()))?;
        let lock_token = match Uuid::parse_str(lock_token) {
            Ok(value) => value,
            Err(_) => return Ok(false),
        };

        let result = sqlx::query(
            r#"
            update ingestion_tasks
            set locked_at = now(),
                updated_at = now()
            where task_id = $1
              and lock_token = $2
              and status = 'processing'
              and dead_lettered_at is null
            "#,
        )
        .bind(task_id)
        .bind(lock_token)
        .execute(self.pool.raw())
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn complete_ingestion_task(
        &self,
        task_id: &str,
        lock_token: Option<&str>,
    ) -> Result<TaskCompletionOutcome, PgStorageError> {
        let Some(lock_token) = lock_token else {
            return Ok(TaskCompletionOutcome::LeaseLost);
        };
        let lock_token = match Uuid::parse_str(lock_token) {
            Ok(value) => value,
            Err(_) => return Ok(TaskCompletionOutcome::LeaseLost),
        };
        let result = sqlx::query(
            r#"
            delete from ingestion_tasks
            where task_id = $1
              and lock_token = $2
              and status = 'processing'
              and dead_lettered_at is null
            "#,
        )
        .bind(
            Uuid::parse_str(task_id)
                .map_err(|_| PgStorageError::NotFound("invalid task id".to_string()))?,
        )
        .bind(lock_token)
        .execute(self.pool.raw())
        .await?;
        if result.rows_affected() > 0 {
            Ok(TaskCompletionOutcome::Completed)
        } else {
            Ok(TaskCompletionOutcome::LeaseLost)
        }
    }

    pub async fn fail_ingestion_task(
        &self,
        task_id: &str,
        lock_token: Option<&str>,
        error: &str,
    ) -> Result<TaskFailureOutcome, PgStorageError> {
        let Some(lock_token) = lock_token else {
            return Ok(TaskFailureOutcome::LeaseLost);
        };
        let lock_token = match Uuid::parse_str(lock_token) {
            Ok(value) => value,
            Err(_) => return Ok(TaskFailureOutcome::LeaseLost),
        };
        let task_id = Uuid::parse_str(task_id)
            .map_err(|_| PgStorageError::NotFound("invalid task id".to_string()))?;

        let mut tx = self.pool.raw().begin().await?;
        let row = sqlx::query(
            r#"
            select attempt_count, max_attempts
            from ingestion_tasks
            where task_id = $1
              and lock_token = $2
              and status = 'processing'
              and dead_lettered_at is null
            for update
            "#,
        )
        .bind(task_id)
        .bind(lock_token)
        .fetch_optional(tx.as_mut())
        .await?;

        let Some(row) = row else {
            tx.commit().await?;
            return Ok(TaskFailureOutcome::LeaseLost);
        };
        let attempt_count: i32 = row.try_get("attempt_count")?;
        let max_attempts: i32 = row.try_get("max_attempts")?;

        if attempt_count >= max_attempts {
            sqlx::query(
                r#"
                update ingestion_tasks
                set status = 'dead_letter',
                    locked_at = null,
                    locked_by = null,
                    lock_token = null,
                    last_error = $3,
                    last_failed_at = now(),
                    dead_lettered_at = now(),
                    updated_at = now()
                where task_id = $1 and lock_token = $2
                "#,
            )
            .bind(task_id)
            .bind(lock_token)
            .bind(error)
            .execute(tx.as_mut())
            .await?;
            tx.commit().await?;
            return Ok(TaskFailureOutcome::DeadLettered);
        }

        let backoff_seconds = ingestion_retry_backoff_seconds(attempt_count);
        sqlx::query(
            r#"
            update ingestion_tasks
            set status = 'queued',
                locked_at = null,
                locked_by = null,
                lock_token = null,
                available_at = now() + ($3::int * interval '1 second'),
                last_error = $4,
                last_failed_at = now(),
                updated_at = now()
            where task_id = $1 and lock_token = $2
            "#,
        )
        .bind(task_id)
        .bind(lock_token)
        .bind(backoff_seconds)
        .bind(error)
        .execute(tx.as_mut())
        .await?;
        tx.commit().await?;
        Ok(TaskFailureOutcome::Requeued)
    }
}
