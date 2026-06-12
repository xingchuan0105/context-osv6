use common::{AppError, CreateNotebookRequest, Notebook, StatusOnlyResponse, UpdateNotebookRequest};

use super::AppState;

impl AppState {
    pub async fn list_notebooks(&self) -> Vec<Notebook> {
        self.documents
            .list_notebooks(&self.auth, &self.storage)
            .await
    }

    pub async fn get_notebook(&self, notebook_id: &str) -> Option<Notebook> {
        self.documents
            .get_notebook(&self.auth, &self.storage, notebook_id)
            .await
    }

    pub async fn create_notebook(&self, req: CreateNotebookRequest) -> Result<Notebook, AppError> {
        self.documents
            .create_notebook(&self.auth, &self.storage, &self.analytics, req)
            .await
    }

    pub async fn update_notebook(
        &self,
        notebook_id: &str,
        req: UpdateNotebookRequest,
    ) -> Result<Notebook, AppError> {
        self.documents
            .update_notebook(&self.auth, &self.storage, notebook_id, req)
            .await
    }

    pub async fn delete_notebook(&self, notebook_id: &str) -> Result<StatusOnlyResponse, AppError> {
        self.documents
            .delete_notebook(&self.auth, &self.storage, notebook_id)
            .await
    }
}
