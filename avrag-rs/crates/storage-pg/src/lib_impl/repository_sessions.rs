use super::*;

impl SessionRepository {
    pub async fn list_sessions(
        &self,
        context: &AuthContext,
        workspace_id: Option<Uuid>,
    ) -> Result<Vec<ChatSession>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select id, workspace_id, title, agent_type, pinned, created_at, updated_at
            from chat_sessions
            where ($1::uuid is null or workspace_id = $1)
            order by pinned desc, updated_at desc, created_at desc
            "#,
        )
        .bind(workspace_id)
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
            select id, workspace_id, title, agent_type, pinned, created_at, updated_at
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
            returning id, workspace_id, title, agent_type, pinned, created_at, updated_at
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
        workspace_id: Uuid,
        title: Option<&str>,
        agent_type: &str,
    ) -> Result<ChatSession, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;
        let row = sqlx::query(
            r#"
            insert into chat_sessions (org_id, workspace_id, user_id, title, agent_type)
            values ($1, $2, $3, $4, $5)
            returning id, workspace_id, title, agent_type, pinned, created_at, updated_at
            "#,
        )
        .bind(context.org_id().into_uuid())
        .bind(workspace_id)
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
        turn: &ChatTurn<'_>,
    ) -> Result<i64, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;
        let answer_blocks_value =
            serde_json::to_value(turn.assistant_answer_blocks).unwrap_or_else(|_| json!([]));
        let user_turn_metadata = turn
            .user_turn_metadata
            .clone()
            .unwrap_or_else(|| json!({}));
        let search_tokens = crate::build_user_message_search_tokens(
            turn.user_content,
            turn.user_resolved_query,
        );
        sqlx::query(
            r#"
            insert into chat_messages (org_id, session_id, role, content, citations, turn_metadata, resolved_query, search_tokens)
            values ($1, $2, 'user', $3, '[]'::jsonb, $4, $5, $6)
            "#,
        )
        .bind(context.org_id().into_uuid())
        .bind(session_id)
        .bind(turn.user_content)
        .bind(user_turn_metadata)
        .bind(turn.user_resolved_query)
        .bind(search_tokens)
        .execute(tx.inner())
        .await?;

        let tool_results_value =
            serde_json::to_value(turn.tool_results).unwrap_or_else(|_| json!([]));
        let assistant_search_tokens =
            crate::build_user_message_search_tokens(turn.assistant_content, None);
        let assistant_row = sqlx::query(
            r#"
            insert into chat_messages (org_id, session_id, role, content, answer_blocks, agent_id, agent_name, agent_icon, citations, tool_results, search_tokens)
            values ($1, $2, 'assistant', $3, $4, $5, $6, $7, $8, $9, $10)
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
        .bind(assistant_search_tokens)
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
}
