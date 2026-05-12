use crate::{AppConfig, AppState};
use common::{AppError, ChatRequest, ChatResponse, CreateNotebookRequest};

#[derive(Clone)]
pub struct ChatService {
    state: AppState,
    inject_default_notebook: bool,
}

impl ChatService {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            inject_default_notebook: false,
        }
    }

    pub fn new_with_test_notebook(state: AppState) -> Self {
        Self {
            state,
            inject_default_notebook: true,
        }
    }

    pub fn for_tests() -> Self {
        Self::new_with_test_notebook(AppState::new(AppConfig::default()))
    }

    pub async fn execute(&self, mut req: ChatRequest) -> Result<ChatResponse, AppError> {
        if self.inject_default_notebook && req.notebook_id.is_none() {
            let notebook = self
                .state
                .create_notebook(CreateNotebookRequest {
                    name: "Test Notebook".to_string(),
                    description: String::new(),
                })
                .await?;
            req.notebook_id = Some(notebook.id);
        }

        self.state.execute_chat_pipeline(req).await
    }
}
