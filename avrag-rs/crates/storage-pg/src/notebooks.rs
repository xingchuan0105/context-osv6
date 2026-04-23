use std::sync::Arc;

use avrag_auth::AuthContext;
use common::Notebook;

#[derive(Clone)]
pub struct PgNotebookQueries {
    repo: Arc<crate::PgAppRepository>,
}

impl PgNotebookQueries {
    pub fn new(repo: Arc<crate::PgAppRepository>) -> Self {
        Self { repo }
    }

    pub async fn list(&self, auth: &AuthContext) -> Result<Vec<Notebook>, crate::PgStorageError> {
        self.repo.list_notebooks(auth).await
    }
}

impl crate::PgAppRepository {
    pub async fn list_notebooks(
        &self,
        context: &AuthContext,
    ) -> Result<Vec<Notebook>, crate::PgStorageError> {
        let mut tx = self.pool.begin(context).await?;
        let rows = sqlx::query(
            r#"
            select
                n.id, n.org_id, n.owner_id, n.title, n.description, n.created_at, n.updated_at,
                coalesce(doc_stats.document_count, 0) as document_count,
                coalesce(doc_stats.status_summary, '{}'::jsonb) as status_summary,
                exists(select 1 from share_tokens st where st.notebook_id = n.id and st.revoked_at is null) as shared
            from notebooks n
            left join lateral (
                select count(*) as document_count,
                    jsonb_object_agg(status, cnt) as status_summary
                from (select status, count(*) as cnt from documents d where d.notebook_id = n.id group by status) sub
            ) doc_stats on true
            order by n.updated_at desc, n.created_at desc
            "#,
        )
        .fetch_all(tx.inner())
        .await?;
        tx.commit().await?;
        rows.into_iter().map(crate::map_notebook).collect()
    }
}
