use std::sync::Arc;

use avrag_auth::AuthContext;
use common::ChatMessage;
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
            select id, session_id, role, content, answer_blocks, agent_id, agent_name, agent_icon, citations, tool_results, turn_metadata, resolved_query, created_at
            from chat_messages
            where session_id = $1
            order by id asc
            "#,
        )
        .bind(session_id)
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;
        rows.into_iter().map(crate::map_message).collect()
    }
}
