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
        notebook_id: Uuid,
    ) -> Result<Vec<Document>, crate::PgStorageError> {
        self.repo
            .list_documents(auth, Some(notebook_id), None)
            .await
    }
}

impl crate::PgAppRepository {
    pub async fn list_documents(
        &self,
        context: &AuthContext,
        notebook_id: Option<Uuid>,
        document_id: Option<Uuid>,
    ) -> Result<Vec<Document>, crate::PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select id, org_id, notebook_id, file_name, mime_type, file_size, status, chunk_count, created_at, updated_at
            from documents
            where ($1::uuid is null or notebook_id = $1)
              and ($2::uuid is null or id = $2)
              and status not in ('deleting', 'deleted')
            order by updated_at desc, created_at desc
            "#,
        )
        .bind(notebook_id)
        .bind(document_id)
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;
        rows.into_iter().map(crate::map_document).collect()
    }
}
