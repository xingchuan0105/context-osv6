use crate::ports::notebook_store::NotebookStore;
use async_trait::async_trait;
use common::{AppError, CreateNotebookRequest, Notebook};
use std::sync::Arc;

#[derive(Clone)]
pub struct PgNotebookStore {
    pub repo: Arc<avrag_storage_pg::PgAppRepository>,
    pub auth: avrag_auth::AuthContext,
}

#[async_trait]
impl NotebookStore for PgNotebookStore {
    async fn list_notebooks(&self) -> Result<Vec<Notebook>, AppError> {
        self.repo
            .list_notebooks(&self.auth)
            .await
            .map_err(crate::map_pg_error)
    }

    async fn create_notebook(&self, req: CreateNotebookRequest) -> Result<Notebook, AppError> {
        self.repo
            .create_notebook(&self.auth, req.name.trim(), req.description.trim())
            .await
            .map_err(crate::map_pg_error)
    }
}
