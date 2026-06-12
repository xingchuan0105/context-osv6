const STALE_PROCESSING_TIMEOUT_SECS: i32 = 30 * 60;
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

pub struct ChatTurn<'a> {
    pub user_content: &'a str,
    pub assistant_content: &'a str,
    pub assistant_answer_blocks: &'a [contracts::chat::AnswerBlock],
    pub agent_type: &'a str,
    pub citations: &'a [contracts::chat::Citation],
    pub tool_results: &'a [common::ToolResult],
    /// Metadata for the user message row (e.g. query_resolution per ADR-0008).
    pub user_turn_metadata: Option<serde_json::Value>,
    /// Non-destructive resolved query for retrieval (ADR-0008); `user_content` stays raw.
    pub user_resolved_query: Option<&'a str>,
}

impl PgAppRepository {
    pub async fn list_sessions(
        &self,
        context: &AuthContext,
        notebook_id: Option<Uuid>,
    ) -> Result<Vec<ChatSession>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select id, notebook_id, title, agent_type, COALESCE(summary, NULL) as summary, pinned, created_at, updated_at
            from chat_sessions
            where ($1::uuid is null or notebook_id = $1)
            order by pinned desc, updated_at desc, created_at desc
            "#,
        )
        .bind(notebook_id)
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;
        rows.into_iter().map(map_session).collect()
    }

    pub async fn get_session(
        &self,
        context: &AuthContext,
        session_id: Uuid,
    ) -> Result<Option<ChatSession>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            select id, notebook_id, title, agent_type, COALESCE(summary, NULL) as summary, pinned, created_at, updated_at
            from chat_sessions
            where id = $1
            "#,
        )
        .bind(session_id)
        .fetch_optional(tx.inner())
        .await?;
        tx.commit().await?;
        row.map(map_session).transpose()
    }

    pub async fn update_session_summary(
        &self,
        context: &AuthContext,
        session_id: Uuid,
        summary: &str,
    ) -> Result<(), PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        sqlx::query(
            r#"
            update chat_sessions
            set summary = $1, updated_at = now()
            where id = $2
            "#,
        )
        .bind(summary)
        .bind(session_id)
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn update_session(
        &self,
        context: &AuthContext,
        session_id: Uuid,
        title: Option<&str>,
        pinned: Option<bool>,
    ) -> Result<Option<ChatSession>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            update chat_sessions
            set title = COALESCE($2, title),
                pinned = COALESCE($3, pinned),
                updated_at = now()
            where id = $1
            returning id, notebook_id, title, agent_type, COALESCE(summary, NULL) as summary, pinned, created_at, updated_at
            "#,
        )
        .bind(session_id)
        .bind(title)
        .bind(pinned)
        .fetch_optional(tx.inner())
        .await?;
        tx.commit().await?;
        row.map(map_session).transpose()
    }

    pub async fn create_session(
        &self,
        context: &AuthContext,
        notebook_id: Uuid,
        title: Option<&str>,
        agent_type: &str,
    ) -> Result<ChatSession, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;
        let row = sqlx::query(
            r#"
            insert into chat_sessions (org_id, notebook_id, user_id, title, agent_type)
            values ($1, $2, $3, $4, $5)
            returning id, notebook_id, title, agent_type, summary, pinned, created_at, updated_at
            "#,
        )
        .bind(context.org_id().into_uuid())
        .bind(notebook_id)
        .bind(context.actor_id().map(ActorId::into_uuid))
        .bind(title)
        .bind(agent_type)
        .fetch_one(tx.inner())
        .await?;
        tx.commit().await?;
        map_session(row)
    }

    pub async fn delete_session(
        &self,
        context: &AuthContext,
        session_id: Uuid,
    ) -> Result<bool, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let result = sqlx::query("delete from chat_sessions where id = $1")
            .bind(session_id)
            .execute(tx.inner())
            .await?;
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn get_message(
        &self,
        context: &AuthContext,
        session_id: Uuid,
        message_id: i64,
    ) -> Result<Option<ChatMessage>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let row = sqlx::query(
            r#"
            select id, session_id, role, content, answer_blocks, agent_id, agent_name, agent_icon, citations, tool_results, created_at
            from chat_messages
            where session_id = $1 and id = $2
            "#,
        )
        .bind(session_id)
        .bind(message_id)
        .fetch_optional(tx.inner())
        .await?;
        tx.commit().await?;
        row.map(map_message).transpose()
    }

    pub async fn append_chat_turn(
        &self,
        context: &AuthContext,
        session_id: Uuid,
        turn: ChatTurn<'_>,
    ) -> Result<i64, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;
        let answer_blocks_value =
            serde_json::to_value(turn.assistant_answer_blocks).unwrap_or_else(|_| json!([]));
        let user_turn_metadata = turn
            .user_turn_metadata
            .clone()
            .unwrap_or_else(|| json!({}));
        sqlx::query(
            r#"
            insert into chat_messages (org_id, session_id, role, content, citations, turn_metadata, resolved_query)
            values ($1, $2, 'user', $3, '[]'::jsonb, $4, $5)
            "#,
        )
        .bind(context.org_id().into_uuid())
        .bind(session_id)
        .bind(turn.user_content)
        .bind(user_turn_metadata)
        .bind(turn.user_resolved_query)
        .execute(tx.inner())
        .await?;

        let tool_results_value =
            serde_json::to_value(turn.tool_results).unwrap_or_else(|_| json!([]));
        let assistant_row = sqlx::query(
            r#"
            insert into chat_messages (org_id, session_id, role, content, answer_blocks, agent_id, agent_name, agent_icon, citations, tool_results)
            values ($1, $2, 'assistant', $3, $4, $5, $6, $7, $8, $9)
            returning id
            "#,
        )
        .bind(context.org_id().into_uuid())
        .bind(session_id)
        .bind(turn.assistant_content)
        .bind(answer_blocks_value)
        .bind(turn.agent_type)
        .bind(agent_name(turn.agent_type))
        .bind(agent_icon(turn.agent_type))
        .bind(serde_json::to_value(turn.citations).unwrap_or_else(|_| json!([])))
        .bind(tool_results_value)
        .fetch_one(tx.inner())
        .await?;

        sqlx::query("update chat_sessions set updated_at = now() where id = $1")
            .bind(session_id)
            .execute(tx.inner())
            .await?;

        tx.commit().await?;
        Ok(assistant_row.try_get::<i64, _>("id")?)
    }

    pub async fn record_usage_event(
        &self,
        context: &AuthContext,
        metric_type: &str,
        quantity: i64,
        source: &str,
    ) -> Result<(), PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;
        sqlx::query(
            r#"
            insert into usage_events (org_id, user_id, metric_type, quantity, source, created_at)
            values ($1, $2, $3, $4, $5, now())
            "#,
        )
        .bind(context.org_id().into_uuid())
        .bind(context.actor_id().map(ActorId::into_uuid))
        .bind(metric_type)
        .bind(quantity)
        .bind(source)
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn enqueue_ingestion_task(
        &self,
        task: &IngestionTask,
    ) -> Result<bool, PgStorageError> {
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
                payload, status, attempt_count, max_attempts, available_at, enqueued_at, updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, 'queued', $9, $10, now(), $11, now())
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
                payload, status, attempt_count, max_attempts, available_at, enqueued_at, updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, 'queued', $9, $10, now(), $11, now())
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
                    or (status = 'processing' and locked_at <= now() - ($2::int * interval '1 second'))
                  )
                returning task_id
            ),
            next_task as (
                select task_id
                from ingestion_tasks
                where dead_lettered_at is null
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
            returning dct.task_id, dct.org_id, dct.notebook_id, dct.document_id, dct.requested_by,
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
            select org_id, document_id, attempt_count, max_attempts
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
        let org_id: Uuid = row.try_get("org_id")?;
        let document_id: Uuid = row.try_get("document_id")?;
        let attempt_count: i32 = row.try_get("attempt_count")?;
        let max_attempts: i32 = row.try_get("max_attempts")?;
        sqlx::query("select set_config('app.current_org', $1, true)")
            .bind(org_id.to_string())
            .execute(tx.as_mut())
            .await?;
        sqlx::query(
            r#"
            update documents
            set deletion_error = $3,
                updated_at = now()
            where org_id = $1
              and id = $2
              and status = 'deleting'
            "#,
        )
        .bind(org_id)
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

    /// Global search: notebooks by title/description using ILIKE.
    pub async fn search_notebooks(
        &self,
        context: &AuthContext,
        pattern: &str,
    ) -> Result<Vec<Notebook>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select id, org_id, owner_id, title, description, created_at, updated_at
            from notebooks
            where (title ilike $1 or description ilike $1)
            order by updated_at desc, created_at desc
            limit 50
            "#,
        )
        .bind(pattern)
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;
        rows.into_iter().map(map_notebook).collect()
    }

    /// Global search: chat sessions by title using ILIKE.
    pub async fn search_sessions(
        &self,
        context: &AuthContext,
        pattern: &str,
    ) -> Result<Vec<ChatSession>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select id, notebook_id, title, agent_type, COALESCE(summary, NULL) as summary, pinned, created_at, updated_at
            from chat_sessions
            where title ilike $1
            order by updated_at desc, created_at desc
            limit 50
            "#,
        )
        .bind(pattern)
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;
        rows.into_iter().map(map_session).collect()
    }

    /// Global search: documents (sources) by file_name using ILIKE.
    pub async fn search_sources(
        &self,
        context: &AuthContext,
        pattern: &str,
    ) -> Result<Vec<SourceRow>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select d.id, d.notebook_id, n.title as notebook_name, d.file_name, d.status
            from documents d
            join notebooks n on n.id = d.notebook_id
            where d.file_name ilike $1
              and d.status not in ('deleting', 'deleted')
            order by d.updated_at desc, d.created_at desc
            limit 50
            "#,
        )
        .bind(pattern)
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

    pub async fn append_audit_record(&self, record: &AuditRecord) -> Result<(), PgStorageError> {
        let org_id = Uuid::parse_str(&record.org_id)
            .map_err(|_| PgStorageError::NotFound("invalid audit org id".to_string()))?;
        let context = AuthContext::new(OrgId::from(org_id), avrag_auth::SubjectKind::System);
        let mut tx = self.pool.begin(&context).await?;
        ensure_org_and_actor(tx.inner(), &context).await?;
        sqlx::query(
            r#"
            insert into audit_log (org_id, actor_id, action, resource_type, resource_id, payload, created_at)
            values ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(org_id)
        .bind(record.actor_id.as_deref().and_then(|value| Uuid::parse_str(value).ok()))
        .bind(record.action.as_str())
        .bind(&record.resource_type)
        .bind(&record.resource_id)
        .bind(&record.payload)
        .bind(parse_rfc3339(&record.created_at)?)
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Prune audit_log records older than the retention period.
    /// Returns the number of deleted rows.
    pub async fn prune_audit_log(
        &self,
        retention_days: i32,
    ) -> Result<u64, PgStorageError> {
        let result = sqlx::query(
            r#"
            delete from audit_log
            where created_at < now() - ($1::int * interval '1 day')
            "#,
        )
        .bind(retention_days)
        .execute(self.pool.raw())
        .await?;
        Ok(result.rows_affected())
    }

}
