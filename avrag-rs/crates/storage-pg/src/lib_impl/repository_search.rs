use super::*;
impl ChunkRepository {
    /// Global search: workspaces by title/description using ILIKE.
    pub async fn search_workspaces(
        &self,
        context: &AuthContext,
        pattern: &str,
    ) -> Result<Vec<Workspace>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select id, owner_user_id, owner_id, title, description, created_at, updated_at
            from workspaces
            where (title ilike $1 or description ilike $1)
              and owner_user_id = $2
            order by updated_at desc, created_at desc
            limit 50
            "#,
        )
        .bind(pattern)
        .bind(context.user_id().into_uuid())
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;
        rows.into_iter().map(map_notebook).collect()
    }

    /// Global search: chat sessions by title (ILIKE) or user message body (FTS).
    pub async fn search_sessions(
        &self,
        context: &AuthContext,
        pattern: &str,
    ) -> Result<Vec<ChatSession>, PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let segmented_query = common::segment_for_fts(pattern.trim_matches('%'));
        let rows = if segmented_query.is_empty() {
            sqlx::query(
                r#"
                select s.id, s.owner_user_id, s.workspace_id, w.title as workspace_name,
                       s.title, s.agent_type, s.model_role, s.pinned, s.created_at, s.updated_at
                from chat_sessions s
                left join workspaces w
                  on w.id = s.workspace_id and w.owner_user_id = s.owner_user_id
                where s.owner_user_id = $2
                  and s.title ilike $1
                order by s.updated_at desc, s.created_at desc
                limit 50
                "#,
            )
            .bind(pattern)
            .bind(context.user_id().into_uuid())
            .fetch_all(tx.inner())
            .await?
        } else {
            sqlx::query(
                r#"
                select distinct s.id, s.owner_user_id, s.workspace_id,
                       w.title as workspace_name, s.title, s.agent_type, s.model_role,
                       s.pinned, s.created_at, s.updated_at
                from chat_sessions s
                left join workspaces w
                  on w.id = s.workspace_id and w.owner_user_id = s.owner_user_id
                where s.owner_user_id = $3
                  and (s.title ilike $1
                   or exists (
                       select 1
                       from chat_messages m
                       where m.session_id = s.id
                         and m.role in ('user', 'assistant')
                         and m.owner_user_id = s.owner_user_id
                         and m.search_vector @@ plainto_tsquery('simple', $2)
                   ))
                order by s.updated_at desc, s.created_at desc
                limit 50
                "#,
            )
            .bind(pattern)
            .bind(&segmented_query)
            .bind(context.user_id().into_uuid())
            .fetch_all(tx.inner())
            .await?
        };
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
            select d.id, wb.workspace_id, n.title as workspace_name, d.file_name, d.status,
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
            join workspace_document_bindings wb on wb.artifact_id = d.id
            join workspaces n on n.id = wb.workspace_id
            where d.owner_user_id = $2
              and n.owner_user_id = d.owner_user_id
              and wb.owner_user_id = d.owner_user_id
              and d.file_name ilike $1
              and d.status not in ('deleting', 'deleted')
            order by d.updated_at desc, d.created_at desc
            limit 50
            "#,
        )
        .bind(pattern)
        .bind(context.user_id().into_uuid())
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
