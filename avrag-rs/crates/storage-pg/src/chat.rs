use std::sync::Arc;

use contracts::auth_runtime::AuthContext;
use contracts::chat::ChatMessage;
use uuid::Uuid;

#[derive(Clone)]
pub struct PgChatQueries {
    repo: Arc<crate::PgAppRepository>,
}

impl PgChatQueries {
    pub fn new(repo: Arc<crate::PgAppRepository>) -> Self {
        Self { repo }
    }

    pub async fn list_messages(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
    ) -> Result<Vec<ChatMessage>, crate::PgStorageError> {
        self.repo.list_messages(auth, session_id).await
    }
}

impl crate::PgAppRepository {
    pub async fn list_messages(
        &self,
        context: &AuthContext,
        session_id: Uuid,
    ) -> Result<Vec<ChatMessage>, crate::PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select m.id, m.session_id, m.role, m.content, m.answer_blocks, m.agent_id, m.agent_name, m.agent_icon, m.citations, m.tool_results, m.turn_metadata, m.resolved_query, m.created_at
            from chat_messages m
            join chat_sessions s on s.id = m.session_id
            where m.session_id = $1
              and m.owner_user_id = $2
              and s.owner_user_id = $2
            order by m.id asc
            "#,
        )
        .bind(session_id)
        .bind(context.user_id().into_uuid())
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;
        rows.into_iter().map(crate::map_message).collect()
    }
}
