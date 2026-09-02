use std::sync::Arc;

use contracts::auth_runtime::AuthContext;
use common::Document;
use uuid::Uuid;

#[derive(Clone)]
pub struct PgDocumentQueries {
    repo: Arc<crate::PgAppRepository>,
}

impl PgDocumentQueries {
    pub fn new(repo: Arc<crate::PgAppRepository>) -> Self {
        Self { repo }
    }

    pub async fn list(
        &self,
        auth: &AuthContext,
        workspace_id: Uuid,
    ) -> Result<Vec<Document>, crate::PgStorageError> {
        self.repo
            .list_documents(auth, Some(workspace_id), None)
            .await
    }
}

impl crate::PgAppRepository {
    pub async fn list_documents(
        &self,
        context: &AuthContext,
        workspace_id: Option<Uuid>,
        document_id: Option<Uuid>,
    ) -> Result<Vec<Document>, crate::PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select d.id, d.owner_user_id, wb.workspace_id, d.file_name, d.mime_type, d.file_size, d.status, d.chunk_count, d.created_at, d.updated_at
            from documents d
            left join lateral (
                select b.workspace_id
                from workspace_document_bindings b
                where b.artifact_id = d.id
                order by b.created_at asc, b.id asc
                limit 1
            ) wb on true
            where d.owner_user_id = $3
              and ($1::uuid is null or exists (
                    select 1 from workspace_document_bindings b
                    where b.artifact_id = d.id and b.workspace_id = $1
              ))
              and ($2::uuid is null or d.id = $2)
              and d.status not in ('deleting', 'deleted')
            order by d.updated_at desc, d.created_at desc
            "#,
        )
        .bind(workspace_id)
        .bind(document_id)
        .bind(context.user_id().into_uuid())
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;
        rows.into_iter().map(crate::map_document).collect()
    }
}
