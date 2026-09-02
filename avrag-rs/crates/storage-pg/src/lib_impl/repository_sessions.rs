use super::*;

impl SessionRepository {
    pub async fn list_sessions(
        &self,
        context: &AuthContext,
        workspace_id: Option<Uuid>,
    ) -> Result<Vec<ChatSession>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let owner_user_id = context.user_id().into_uuid();
        let rows = match workspace_id {
            None => {
                sqlx::query(
                    r#"
                    select s.id, s.owner_user_id, s.workspace_id, w.title as workspace_name,
                           s.title, s.agent_type, s.model_role, s.pinned, s.created_at, s.updated_at
                    from chat_sessions s
                    left join workspaces w
                      on w.id = s.workspace_id and w.owner_user_id = s.owner_user_id
                    where s.owner_user_id = $1
                    order by s.updated_at desc, s.created_at desc
                    "#,
                )
                .bind(owner_user_id)
                .fetch_all(tx.inner())
                .await?
            }
            Some(workspace_id) => {
                sqlx::query(
                    r#"
                    select s.id, s.owner_user_id, s.workspace_id, w.title as workspace_name,
                           s.title, s.agent_type, s.model_role, s.pinned, s.created_at, s.updated_at
                    from chat_sessions s
                    left join workspaces w
                      on w.id = s.workspace_id and w.owner_user_id = s.owner_user_id
                    where s.workspace_id = $1 and s.owner_user_id = $2
                    order by s.pinned desc, s.updated_at desc, s.created_at desc
                    "#,
                )
                .bind(workspace_id)
                .bind(owner_user_id)
                .fetch_all(tx.inner())
                .await?
            }
        };
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
            select s.id, s.owner_user_id, s.workspace_id, w.title as workspace_name,
                   s.title, s.agent_type, s.model_role, s.pinned, s.created_at, s.updated_at
            from chat_sessions s
            left join workspaces w
              on w.id = s.workspace_id and w.owner_user_id = s.owner_user_id
            where s.id = $1 and s.owner_user_id = $2
            "#,
        )
        .bind(session_id)
        .bind(context.user_id().into_uuid())
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
            with updated as (
                update chat_sessions
                set title = COALESCE($3, title),
                    pinned = COALESCE($4, pinned),
                    updated_at = now()
                where id = $1 and owner_user_id = $2
                returning id, owner_user_id, workspace_id, title, agent_type, model_role,
                          pinned, created_at, updated_at
            )
            select u.id, u.owner_user_id, u.workspace_id, w.title as workspace_name,
                   u.title, u.agent_type, u.model_role, u.pinned, u.created_at, u.updated_at
            from updated u
            left join workspaces w
              on w.id = u.workspace_id and w.owner_user_id = u.owner_user_id
            "#,
        )
        .bind(session_id)
        .bind(context.user_id().into_uuid())
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
        workspace_id: Option<Uuid>,
        title: Option<&str>,
        agent_type: &str,
        model_role: &str,
    ) -> Result<ChatSession, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        ensure_org_and_actor(tx.inner(), context).await?;
        // Parent-guarded insert: zero rows when the workspace belongs to
        // someone else, surfacing as NotFound instead of a cross-tenant child.
        let row = sqlx::query(
            r#"
            with inserted as (
                insert into chat_sessions (owner_user_id, workspace_id, user_id, title, agent_type, model_role)
                select $1::uuid, $2::uuid, $3::uuid, $4, $5, $6
                where $2::uuid is null or exists (
                    select 1 from workspaces w
                    where w.id = $2::uuid and w.owner_user_id = $1::uuid
                )
                returning id, owner_user_id, workspace_id, title, agent_type, model_role,
                          pinned, created_at, updated_at
            )
            select i.id, i.owner_user_id, i.workspace_id, w.title as workspace_name,
                   i.title, i.agent_type, i.model_role, i.pinned, i.created_at, i.updated_at
            from inserted i
            left join workspaces w
              on w.id = i.workspace_id and w.owner_user_id = i.owner_user_id
            "#,
        )
        .bind(context.user_id().into_uuid())
        .bind(workspace_id)
        .bind(context.actor_id().map(ActorId::into_uuid))
        .bind(title)
        .bind(agent_type)
        .bind(model_role)
        .fetch_optional(tx.inner())
        .await?;
        let Some(row) = row else {
            return Err(PgStorageError::NotFound("resource not found".to_string()));
        };
        tx.commit().await?;
        map_session(row)
    }

    pub async fn delete_session(
        &self,
        context: &AuthContext,
        session_id: Uuid,
    ) -> Result<bool, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let result = sqlx::query("delete from chat_sessions where id = $1 and owner_user_id = $2")
            .bind(session_id)
            .bind(context.user_id().into_uuid())
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
            select m.id, m.session_id, m.role, m.content, m.answer_blocks, m.agent_id, m.agent_name, m.agent_icon, m.citations, m.tool_results, m.turn_metadata, m.resolved_query, m.created_at
            from chat_messages m
            join chat_sessions s on s.id = m.session_id
            where m.session_id = $1 and m.id = $2
              and s.owner_user_id = $3 and m.owner_user_id = $3
            "#,
        )
        .bind(session_id)
        .bind(message_id)
        .bind(context.user_id().into_uuid())
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
        let user_turn_metadata = turn.user_turn_metadata.clone().unwrap_or_else(|| json!({}));
        let search_tokens =
            crate::build_user_message_search_tokens(turn.user_content, turn.user_resolved_query);
        // Parent-guarded: the whole turn writes zero rows unless the session
        // belongs to the caller. Cross-tenant session ids surface as NotFound.
        let parent_guarded = sqlx::query_scalar::<_, i64>(
            r#"
            with locked as (
                select id, owner_user_id from chat_sessions
                where id = $1 and owner_user_id = $2
                for update
            )
            insert into chat_messages (owner_user_id, session_id, role, content, citations, turn_metadata, resolved_query, search_tokens)
            select l.owner_user_id, l.id, 'user', $3, '[]'::jsonb, $4, $5, $6 from locked l
            returning id
            "#,
        )
        .bind(session_id)
        .bind(context.user_id().into_uuid())
        .bind(turn.user_content)
        .bind(user_turn_metadata)
        .bind(turn.user_resolved_query)
        .bind(search_tokens)
        .fetch_optional(tx.inner())
        .await?;
        if parent_guarded.is_none() {
            return Err(PgStorageError::NotFound("resource not found".to_string()));
        }

        let tool_results_value =
            serde_json::to_value(turn.tool_results).unwrap_or_else(|_| json!([]));
        let assistant_turn_metadata = turn
            .assistant_turn_metadata
            .clone()
            .unwrap_or_else(|| json!({}));
        let assistant_search_tokens =
            crate::build_user_message_search_tokens(turn.assistant_content, None);
        let assistant_row = sqlx::query(
            r#"
            insert into chat_messages (owner_user_id, session_id, role, content, answer_blocks, agent_id, agent_name, agent_icon, citations, tool_results, turn_metadata, search_tokens)
            values ($1, $2, 'assistant', $3, $4, $5, $6, $7, $8, $9, $10, $11)
            returning id
            "#,
        )
        .bind(context.user_id().into_uuid())
        .bind(session_id)
        .bind(turn.assistant_content)
        .bind(answer_blocks_value)
        .bind(turn.agent_type)
        .bind(agent_name(turn.agent_type))
        .bind(agent_icon(turn.agent_type))
        .bind(serde_json::to_value(turn.citations).unwrap_or_else(|_| json!([])))
        .bind(tool_results_value)
        .bind(assistant_turn_metadata)
        .bind(assistant_search_tokens)
        .fetch_one(tx.inner())
        .await?;

        sqlx::query(
            "update chat_sessions set updated_at = now() where id = $1 and owner_user_id = $2",
        )
        .bind(session_id)
        .bind(context.user_id().into_uuid())
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
            insert into usage_events (owner_user_id, user_id, metric_type, quantity, source, created_at)
            values ($1, $2, $3, $4, $5, now())
            "#,
        )
        .bind(context.user_id().into_uuid())
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
