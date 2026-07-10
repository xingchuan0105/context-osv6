use super::*;
/// A conversation message hit from hybrid memory search.
#[derive(Debug, Clone)]
pub struct ConversationHistoryHit {
    pub message_id: i64,
    pub session_id: Uuid,
    pub role: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationHistoryScope {
    Session,
    Workspace,
}

const RECENT_CANDIDATE_LIMIT: i64 = 50;
const FTS_CANDIDATE_LIMIT: i64 = 30;

impl ConversationMemoryRepository {
    pub async fn search_conversation_history(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
        query: &str,
        scope: ConversationHistoryScope,
        limit: i64,
        exclude_message_ids: &[i64],
    ) -> Result<Vec<ConversationHistoryHit>, PgStorageError> {
        let limit = limit.clamp(1, 50);
        let workspace_id = self.resolve_session_workspace_id(auth, session_id).await?;
        let user_id = auth
            .actor_id()
            .map(|actor| actor.into_uuid())
            .ok_or_else(|| PgStorageError::NotFound("authenticated user required".to_string()))?;

        let recent = self.load_recent_messages(
            auth,
            session_id,
            workspace_id,
            user_id,
            scope,
            RECENT_CANDIDATE_LIMIT,
            exclude_message_ids,
        )
        .await?;

        let segmented_query = segment_for_fts(query);
        let fts = if segmented_query.is_empty() {
            Vec::new()
        } else {
            self.search_messages_fts(
                auth,
                session_id,
                workspace_id,
                user_id,
                scope,
                &segmented_query,
                FTS_CANDIDATE_LIMIT,
                exclude_message_ids,
            )
            .await?
        };

        if fts.is_empty() {
            return Ok(recent.into_iter().take(limit as usize).collect());
        }

        let merged = rrf_merge(
            &[&recent, &fts],
            |hit: &ConversationHistoryHit| hit.message_id,
            limit as usize,
        );
        Ok(merged)
    }

    async fn resolve_session_workspace_id(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
    ) -> Result<Uuid, PgStorageError> {
        let mut tx = self.pool.begin(auth).await?;
        let row = sqlx::query(
            r#"
            select workspace_id
            from chat_sessions
            where id = $1 and owner_user_id = $2
            "#,
        )
        .bind(session_id)
        .bind(auth.user_id().into_uuid())
        .fetch_optional(tx.inner())
        .await?;
        tx.commit().await?;
        let workspace_id = row
            .and_then(|r| r.try_get::<Uuid, _>("workspace_id").ok())
            .ok_or_else(|| PgStorageError::NotFound("session not found".to_string()))?;
        Ok(workspace_id)
    }

    async fn load_recent_messages(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
        workspace_id: Uuid,
        user_id: Uuid,
        scope: ConversationHistoryScope,
        limit: i64,
        exclude_message_ids: &[i64],
    ) -> Result<Vec<ConversationHistoryHit>, PgStorageError> {
        let mut tx = self.pool.begin(auth).await?;
        let rows = if scope == ConversationHistoryScope::Session {
            sqlx::query(
                r#"
                select m.id as message_id, m.session_id, m.role, m.content, m.created_at
                from chat_messages m
                where m.owner_user_id = $1
                  and m.session_id = $2
                  and m.role in ('user', 'assistant')
                  and not (m.id = any($3))
                order by m.id desc
                limit $4
                "#,
            )
            .bind(auth.user_id().into_uuid())
            .bind(session_id)
            .bind(exclude_message_ids)
            .bind(limit)
            .fetch_all(tx.inner())
            .await?
        } else {
            sqlx::query(
                r#"
                select m.id as message_id, m.session_id, m.role, m.content, m.created_at
                from chat_messages m
                join chat_sessions s on s.id = m.session_id
                where m.owner_user_id = $1
                  and s.workspace_id = $2
                  and s.user_id = $3
                  and m.role in ('user', 'assistant')
                  and not (m.id = any($4))
                order by m.id desc
                limit $5
                "#,
            )
            .bind(auth.user_id().into_uuid())
            .bind(workspace_id)
            .bind(user_id)
            .bind(exclude_message_ids)
            .bind(limit)
            .fetch_all(tx.inner())
            .await?
        };
        tx.commit().await?;
        rows.into_iter().map(map_history_hit).collect()
    }

    async fn search_messages_fts(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
        workspace_id: Uuid,
        user_id: Uuid,
        scope: ConversationHistoryScope,
        segmented_query: &str,
        limit: i64,
        exclude_message_ids: &[i64],
    ) -> Result<Vec<ConversationHistoryHit>, PgStorageError> {
        let mut tx = self.pool.begin(auth).await?;
        let rows = if scope == ConversationHistoryScope::Session {
            sqlx::query(
                r#"
                select m.id as message_id, m.session_id, m.role, m.content, m.created_at,
                       ts_rank_cd(m.search_vector, plainto_tsquery('simple', $3)) as rank
                from chat_messages m
                where m.owner_user_id = $1
                  and m.session_id = $2
                  and m.role in ('user', 'assistant')
                  and not (m.id = any($4))
                  and m.search_vector @@ plainto_tsquery('simple', $3)
                order by rank desc, m.id desc
                limit $5
                "#,
            )
            .bind(auth.user_id().into_uuid())
            .bind(session_id)
            .bind(segmented_query)
            .bind(exclude_message_ids)
            .bind(limit)
            .fetch_all(tx.inner())
            .await?
        } else {
            sqlx::query(
                r#"
                select m.id as message_id, m.session_id, m.role, m.content, m.created_at,
                       ts_rank_cd(m.search_vector, plainto_tsquery('simple', $4)) as rank
                from chat_messages m
                join chat_sessions s on s.id = m.session_id
                where m.owner_user_id = $1
                  and s.workspace_id = $2
                  and s.user_id = $3
                  and m.role in ('user', 'assistant')
                  and not (m.id = any($5))
                  and m.search_vector @@ plainto_tsquery('simple', $4)
                order by rank desc, m.id desc
                limit $6
                "#,
            )
            .bind(auth.user_id().into_uuid())
            .bind(workspace_id)
            .bind(user_id)
            .bind(segmented_query)
            .bind(exclude_message_ids)
            .bind(limit)
            .fetch_all(tx.inner())
            .await?
        };
        tx.commit().await?;
        rows.into_iter().map(map_history_hit).collect()
    }
}

pub fn map_history_hit(row: sqlx::postgres::PgRow) -> Result<ConversationHistoryHit, PgStorageError> {
    Ok(ConversationHistoryHit {
        message_id: row.try_get("message_id")?,
        session_id: row.try_get("session_id")?,
        role: row.try_get("role")?,
        content: row.try_get("content")?,
        created_at: row.try_get("created_at")?,
    })
}

pub fn build_user_message_search_tokens(content: &str, resolved_query: Option<&str>) -> String {
    merge_search_tokens(content, resolved_query)
}

impl ConversationMemoryRepository {
    /// Re-segment `search_tokens` with jieba for all chat messages (post-migrate backfill).
    pub async fn resegment_chat_message_search_tokens(&self) -> Result<u64, PgStorageError> {
        let rows = sqlx::query(
            r#"
            select id, content, resolved_query, role
            from chat_messages
            where coalesce(content, '') <> ''
            order by id
            "#,
        )
        .fetch_all(self.pool.raw())
        .await?;

        let mut updated = 0u64;
        for row in rows {
            let id: i64 = row.try_get("id")?;
            let content: String = row.try_get("content")?;
            let resolved_query: Option<String> = row.try_get("resolved_query")?;
            let role: String = row.try_get("role")?;
            let tokens = if role == "user" {
                build_user_message_search_tokens(&content, resolved_query.as_deref())
            } else {
                build_user_message_search_tokens(&content, None)
            };
            let result = sqlx::query(
                r#"
                update chat_messages
                set search_tokens = $2
                where id = $1
                  and search_tokens is distinct from $2
                "#,
            )
            .bind(id)
            .bind(tokens)
            .execute(self.pool.raw())
            .await?;
            updated += result.rows_affected();
        }
        Ok(updated)
    }
}
