use crate::ports::notebooks::notebook_store::NotebookStore;
use common::{AppError, CreateNotebookRequest};
use contracts::notebooks::Notebook;
use std::sync::Arc;

#[derive(Clone)]
pub struct NotebookService {
    store: Arc<dyn NotebookStore>,
}

impl NotebookService {
    pub fn new(store: Arc<dyn NotebookStore>) -> Self {
        Self { store }
    }

    pub async fn create(&self, req: CreateNotebookRequest) -> Result<Notebook, AppError> {
        self.store.create_notebook(req).await
    }

    pub async fn list(&self) -> Result<Vec<Notebook>, AppError> {
        self.store.list_notebooks().await
    }
}
