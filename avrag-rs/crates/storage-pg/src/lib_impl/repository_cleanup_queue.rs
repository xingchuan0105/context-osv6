use super::*;
impl IngestionQueueRepository {
    pub async fn claim_next_document_cleanup_task(
        &self,
        worker_id: &str,
        stale_after_secs: Option<i32>,
    ) -> Result<Option<DocumentCleanupTask>, PgStorageError> {
        let stale_after_secs = stale_after_secs.unwrap_or(STALE_PROCESSING_TIMEOUT_SECS).max(1);
        let mut tx = self.pool.raw().begin().await?;
        sqlx::query("select set_config('app.document_cleanup_worker', 'true', true)")
            .execute(tx.as_mut())
            .await?;
        let row = sqlx::query(
            r#"
            with exhausted_tasks as (
                update document_cleanup_tasks
                set status = 'dead_letter',
                    dead_lettered_at = coalesce(dead_lettered_at, now()),
                    last_failed_at = coalesce(last_failed_at, now()),
                    last_error = coalesce(last_error, 'document cleanup task exhausted max attempts'),
                    locked_at = null,
                    locked_by = null,
                    lock_token = null,
                    updated_at = now()
                where dead_lettered_at is null
                  and completed_at is null
                  and status <> 'dead_letter'
                  and attempt_count >= max_attempts
                  and (
                    (status = 'queued' and available_at <= now())
                    or (status = 'processing' and locked_at <= now() - ($2::int * interval '1 second'))
                  )
                returning task_id
            ),
            next_task as (
                select task_id
                from document_cleanup_tasks
                where dead_lettered_at is null
                  and completed_at is null
                  and status <> 'dead_letter'
                  and attempt_count < max_attempts
                  and (
                    (status = 'queued' and available_at <= now() and locked_at is null)
                    or (status = 'processing' and locked_at <= now() - ($2::int * interval '1 second'))
                  )
                order by case when status = 'processing' then 0 else 1 end, enqueued_at asc
                limit 1
                for update skip locked
            )
            update document_cleanup_tasks dct
            set status = 'processing',
                locked_at = now(),
                locked_by = $1,
                lock_token = gen_random_uuid(),
                attempt_count = attempt_count + 1,
                updated_at = now()
            from next_task
            where dct.task_id = next_task.task_id
            returning dct.task_id, dct.owner_user_id, dct.workspace_id, dct.document_id, dct.requested_by,
                      dct.idempotency_key, dct.payload, dct.lock_token,
                      dct.attempt_count, dct.max_attempts
            "#,
        )
        .bind(worker_id)
        .bind(stale_after_secs)
        .fetch_optional(tx.as_mut())
        .await?;
        tx.commit().await?;
        row.map(map_document_cleanup_task).transpose()
    }

    pub async fn renew_document_cleanup_task_lock(
        &self,
        task_id: Uuid,
        lock_token: Uuid,
    ) -> Result<bool, PgStorageError> {
        let mut tx = self.pool.raw().begin().await?;
        sqlx::query("select set_config('app.document_cleanup_worker', 'true', true)")
            .execute(tx.as_mut())
            .await?;
        let result = sqlx::query(
            r#"
            update document_cleanup_tasks
            set locked_at = now(),
                updated_at = now()
            where task_id = $1
              and lock_token = $2
              and status = 'processing'
              and dead_lettered_at is null
              and completed_at is null
            "#,
        )
        .bind(task_id)
        .bind(lock_token)
        .execute(tx.as_mut())
        .await?;
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn complete_document_cleanup_task(
        &self,
        task_id: Uuid,
        lock_token: Uuid,
    ) -> Result<DocumentCleanupTaskCompletionOutcome, PgStorageError> {
        let mut tx = self.pool.raw().begin().await?;
        sqlx::query("select set_config('app.document_cleanup_worker', 'true', true)")
            .execute(tx.as_mut())
            .await?;
        let result = sqlx::query(
            r#"
            update document_cleanup_tasks
            set status = 'completed',
                completed_at = coalesce(completed_at, now()),
                locked_at = null,
                locked_by = null,
                lock_token = null,
                updated_at = now()
            where task_id = $1
              and lock_token = $2
              and status = 'processing'
              and dead_lettered_at is null
              and completed_at is null
            "#,
        )
        .bind(task_id)
        .bind(lock_token)
        .execute(tx.as_mut())
        .await?;
        tx.commit().await?;
        if result.rows_affected() > 0 {
            Ok(DocumentCleanupTaskCompletionOutcome::Completed)
        } else {
            Ok(DocumentCleanupTaskCompletionOutcome::LeaseLost)
        }
    }

    pub async fn fail_document_cleanup_task(
        &self,
        task_id: Uuid,
        lock_token: Uuid,
        error: &str,
    ) -> Result<DocumentCleanupTaskFailureOutcome, PgStorageError> {
        let mut tx = self.pool.raw().begin().await?;
        sqlx::query("select set_config('app.document_cleanup_worker', 'true', true)")
            .execute(tx.as_mut())
            .await?;
        let row = sqlx::query(
            r#"
            select owner_user_id, document_id, attempt_count, max_attempts
            from document_cleanup_tasks
            where task_id = $1
              and lock_token = $2
              and status = 'processing'
              and dead_lettered_at is null
              and completed_at is null
            for update
            "#,
        )
        .bind(task_id)
        .bind(lock_token)
        .fetch_optional(tx.as_mut())
        .await?;

        let Some(row) = row else {
            tx.commit().await?;
            return Ok(DocumentCleanupTaskFailureOutcome::LeaseLost);
        };
        let owner_user_id: Uuid = row.try_get("owner_user_id")?;
        let document_id: Uuid = row.try_get("document_id")?;
        let attempt_count: i32 = row.try_get("attempt_count")?;
        let max_attempts: i32 = row.try_get("max_attempts")?;
        sqlx::query("select set_config('app.current_user', $1, true)")
            .bind(owner_user_id.to_string())
            .execute(tx.as_mut())
            .await?;
        sqlx::query(
            r#"
            update documents
            set deletion_error = $3,
                updated_at = now()
            where owner_user_id = $1
              and id = $2
              and status = 'deleting'
            "#,
        )
        .bind(owner_user_id)
        .bind(document_id)
        .bind(error)
        .execute(tx.as_mut())
        .await?;

        if attempt_count >= max_attempts {
            sqlx::query(
                r#"
                update document_cleanup_tasks
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
            return Ok(DocumentCleanupTaskFailureOutcome::DeadLettered);
        }

        let backoff_seconds = retry_backoff_seconds(attempt_count);
        sqlx::query(
            r#"
            update document_cleanup_tasks
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
        Ok(DocumentCleanupTaskFailureOutcome::Requeued)
    }
}
