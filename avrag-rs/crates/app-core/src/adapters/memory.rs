use crate::ports::notebooks::notebook_store::NotebookStore;
use async_trait::async_trait;
use common::{
    AppError, CreateNotebookRequest, default_org_id, default_user_id, new_id, now_rfc3339,
};
use contracts::notebooks::Notebook;

#[derive(Default, Clone)]
pub struct MemoryNotebookStore;

#[async_trait]
impl NotebookStore for MemoryNotebookStore {
    async fn list_notebooks(&self) -> Result<Vec<Notebook>, AppError> {
        Ok(Vec::new())
    }

    async fn create_notebook(&self, req: CreateNotebookRequest) -> Result<Notebook, AppError> {
        let now = now_rfc3339();
        Ok(Notebook {
            id: new_id(),
            org_id: default_org_id(),
            owner_id: default_user_id(),
            name: req.name.clone(),
            title: req.name,
            description: req.description,
            created_at: now.clone(),
            updated_at: now,
            document_count: 0,
            status_summary: std::collections::HashMap::new(),
            shared: false,
        })
    }
}
