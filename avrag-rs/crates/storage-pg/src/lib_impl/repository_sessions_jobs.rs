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
            select id, session_id, role, content, answer_blocks, agent_id, agent_name, agent_icon, citations, created_at
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
        user_content: &str,
        assistant_content: &str,
        assistant_answer_blocks: &[common::AnswerBlock],
        agent_type: &str,
        citations: &[Citation],
    ) -> Result<i64, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;
        let answer_blocks_value =
            serde_json::to_value(assistant_answer_blocks).unwrap_or_else(|_| json!([]));
        sqlx::query(
            r#"
            insert into chat_messages (org_id, session_id, role, content, citations)
            values ($1, $2, 'user', $3, '[]'::jsonb)
            "#,
        )
        .bind(context.org_id().into_uuid())
        .bind(session_id)
        .bind(user_content)
        .execute(tx.inner())
        .await?;

        let assistant_row = sqlx::query(
            r#"
            insert into chat_messages (org_id, session_id, role, content, answer_blocks, agent_id, agent_name, agent_icon, citations)
            values ($1, $2, 'assistant', $3, $4, $5, $6, $7, $8)
            returning id
            "#,
        )
        .bind(context.org_id().into_uuid())
        .bind(session_id)
        .bind(assistant_content)
        .bind(answer_blocks_value)
        .bind(agent_type)
        .bind(agent_name(agent_type))
        .bind(agent_icon(agent_type))
        .bind(serde_json::to_value(citations).unwrap_or_else(|_| json!([])))
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
            insert into usage_events (org_id, metric_type, quantity, source, created_at)
            values ($1, $2, $3, $4, now())
            "#,
        )
        .bind(context.org_id().into_uuid())
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
                payload, status, available_at, enqueued_at, updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, 'queued', now(), $9, now())
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
        .bind(parse_rfc3339(&task.enqueued_at)?)
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn claim_next_ingestion_task(
        &self,
        worker_id: &str,
    ) -> Result<Option<IngestionTask>, PgStorageError> {
        let row = sqlx::query(
            r#"
            with next_task as (
                select task_id
                from ingestion_tasks
                where status = 'queued'
                  and available_at <= now()
                  and (locked_at is null)
                order by enqueued_at asc
                limit 1
                for update skip locked
            )
            update ingestion_tasks it
            set status = 'processing',
                locked_at = now(),
                locked_by = $1,
                attempt_count = attempt_count + 1,
                updated_at = now()
            from next_task
            where it.task_id = next_task.task_id
            returning it.task_id, it.org_id, it.notebook_id, it.document_id, it.kind, it.requested_by,
                      it.idempotency_key, it.enqueued_at, it.payload
            "#,
        )
        .bind(worker_id)
        .fetch_optional(self.pool.raw())
        .await?;
        row.map(map_ingestion_task).transpose()
    }

    pub async fn complete_ingestion_task(&self, task_id: &str) -> Result<(), PgStorageError> {
        sqlx::query("delete from ingestion_tasks where task_id = $1")
            .bind(
                Uuid::parse_str(task_id)
                    .map_err(|_| PgStorageError::NotFound("invalid task id".to_string()))?,
            )
            .execute(self.pool.raw())
            .await?;
        Ok(())
    }

    pub async fn fail_ingestion_task(
        &self,
        task_id: &str,
        error: &str,
    ) -> Result<(), PgStorageError> {
        sqlx::query(
            r#"
            update ingestion_tasks
            set status = 'queued',
                locked_at = null,
                locked_by = null,
                available_at = now() + interval '30 seconds',
                last_error = $2,
                updated_at = now()
            where task_id = $1
            "#,
        )
        .bind(
            Uuid::parse_str(task_id)
                .map_err(|_| PgStorageError::NotFound("invalid task id".to_string()))?,
        )
        .bind(error)
        .execute(self.pool.raw())
        .await?;
        Ok(())
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
        .bind(audit_action_str(&record.action))
        .bind(&record.resource_type)
        .bind(&record.resource_id)
        .bind(&record.payload)
        .bind(parse_rfc3339(&record.created_at)?)
        .execute(tx.inner())
        .await?;
        tx.commit().await?;
        Ok(())
    }

}
