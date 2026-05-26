/// Tag operation sent by the agent.
#[derive(Debug, Clone)]
pub enum TagOperation {
    AddTag { message_id: i64, tag: String },
    RemoveTag { message_id: i64, tag: String },
    ReplaceTags { message_id: i64, tags: Vec<String> },
}

/// A chat message with its tags.
#[derive(Debug, Clone)]
pub struct TaggedMessage {
    pub message_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub tags: Vec<String>,
}

impl PgAppRepository {
    /// Load messages from a session, optionally filtered by tags.
    pub async fn load_history_by_tags(
        &self,
        auth: &AuthContext,
        session_id: Uuid,
        tags: Option<Vec<String>>,
        limit: i64,
    ) -> Result<Vec<TaggedMessage>, PgStorageError> {
        let rows = if let Some(ref tag_list) = tags {
            sqlx::query(
                r#"
                SELECT
                    m.id as message_id,
                    m.role,
                    m.content,
                    m.created_at,
                    COALESCE(
                        ARRAY_AGG(mt.tag) FILTER (WHERE mt.tag IS NOT NULL),
                        ARRAY[]::TEXT[]
                    ) as tags
                FROM chat_messages m
                LEFT JOIN message_tags mt ON m.id = mt.message_id
                WHERE m.session_id = $1 AND m.org_id = $2
                  AND EXISTS (
                      SELECT 1 FROM message_tags mt2
                      WHERE mt2.message_id = m.id AND mt2.tag = ANY($3)
                  )
                GROUP BY m.id
                ORDER BY m.id DESC
                LIMIT $4
                "#
            )
            .bind(session_id)
            .bind(auth.org_id().into_uuid())
            .bind(tag_list)
            .bind(limit)
            .fetch_all(self.pool.raw())
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT
                    m.id as message_id,
                    m.role,
                    m.content,
                    m.created_at,
                    COALESCE(
                        ARRAY_AGG(mt.tag) FILTER (WHERE mt.tag IS NOT NULL),
                        ARRAY[]::TEXT[]
                    ) as tags
                FROM chat_messages m
                LEFT JOIN message_tags mt ON m.id = mt.message_id
                WHERE m.session_id = $1 AND m.org_id = $2
                GROUP BY m.id
                ORDER BY m.id DESC
                LIMIT $3
                "#
            )
            .bind(session_id)
            .bind(auth.org_id().into_uuid())
            .bind(limit)
            .fetch_all(self.pool.raw())
            .await?
        };

        let mut messages = Vec::new();
        for row in rows {
            let tags: Vec<String> = row.try_get("tags").unwrap_or_default();
            messages.push(TaggedMessage {
                message_id: row.try_get("message_id")?,
                role: row.try_get("role")?,
                content: row.try_get("content")?,
                created_at: row.try_get("created_at")?,
                tags,
            });
        }
        Ok(messages)
    }

    /// Apply tag operations (add/remove/replace).
    pub async fn apply_tag_operations(
        &self,
        _auth: &AuthContext,
        operations: Vec<TagOperation>,
    ) -> Result<(), PgStorageError> {
        let mut tx = self.pool.raw().begin().await?;

        for op in operations {
            match op {
                TagOperation::AddTag { message_id, tag } => {
                    sqlx::query(
                        "INSERT INTO message_tags (message_id, tag) VALUES ($1, $2) ON CONFLICT DO NOTHING"
                    )
                    .bind(message_id)
                    .bind(&tag)
                    .execute(&mut *tx)
                    .await?;
                }
                TagOperation::RemoveTag { message_id, tag } => {
                    sqlx::query(
                        "DELETE FROM message_tags WHERE message_id = $1 AND tag = $2"
                    )
                    .bind(message_id)
                    .bind(&tag)
                    .execute(&mut *tx)
                    .await?;
                }
                TagOperation::ReplaceTags { message_id, tags } => {
                    sqlx::query("DELETE FROM message_tags WHERE message_id = $1")
                        .bind(message_id)
                        .execute(&mut *tx)
                        .await?;
                    for tag in tags {
                        sqlx::query(
                            "INSERT INTO message_tags (message_id, tag) VALUES ($1, $2) ON CONFLICT DO NOTHING"
                        )
                        .bind(message_id)
                        .bind(&tag)
                        .execute(&mut *tx)
                        .await?;
                    }
                }
            }
        }

        tx.commit().await?;
        Ok(())
    }
}
