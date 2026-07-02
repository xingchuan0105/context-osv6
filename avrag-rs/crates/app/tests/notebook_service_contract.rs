use app::ports::notebooks::notebook_store::NotebookStore;
use app::services::notebooks::service::NotebookService;
use async_trait::async_trait;
use common::{AppError, CreateNotebookRequest};
use contracts::notebooks::Notebook;

#[derive(Clone, Default)]
struct FakeNotebookStore;

#[async_trait]
impl NotebookStore for FakeNotebookStore {
    async fn list_notebooks(&self) -> Result<Vec<Notebook>, AppError> {
        Ok(Vec::new())
    }

    async fn create_notebook(&self, req: CreateNotebookRequest) -> Result<Notebook, AppError> {
        Ok(Notebook {
            id: "nb-1".into(),
            org_id: "org-1".into(),
            owner_id: "user-1".into(),
            name: req.name.clone(),
            title: req.name,
            description: req.description,
            created_at: "now".into(),
            updated_at: "now".into(),
            document_count: 0,
            status_summary: std::collections::HashMap::new(),
            shared: false,
        })
    }
}

#[tokio::test]
async fn notebook_service_uses_store_port() {
    let service = NotebookService::new(std::sync::Arc::new(FakeNotebookStore));
    let notebook = service
        .create(CreateNotebookRequest {
            name: "Ops".into(),
            description: String::new(),
        })
        .await
        .unwrap();
    assert_eq!(notebook.name, "Ops");
}
