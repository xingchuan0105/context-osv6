use async_trait::async_trait;
use common::{AppError, CreateNotebookRequest};
use contracts::notebooks::Notebook;

#[async_trait]
pub trait NotebookStore: Send + Sync {
    async fn list_notebooks(&self) -> Result<Vec<Notebook>, AppError>;
    async fn create_notebook(&self, req: CreateNotebookRequest) -> Result<Notebook, AppError>;
}
