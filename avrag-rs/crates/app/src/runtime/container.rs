use crate::{
    AppState,
    services::{chat_service::ChatService, notebook_service::NotebookService},
};
use std::sync::Arc;

#[derive(Clone)]
pub struct ServiceRegistry {
    pub chat: Arc<ChatService>,
    pub notebooks: Arc<NotebookService>,
}

impl ServiceRegistry {
    pub fn from_state(state: &AppState) -> Self {
        Self {
            chat: Arc::new(ChatService::new(state.clone())),
            notebooks: Arc::new(NotebookService::new(state.clone())),
        }
    }

    pub fn from_memory_state(state: &AppState) -> Self {
        Self {
            chat: Arc::new(ChatService::new_with_test_notebook(state.clone())),
            notebooks: Arc::new(NotebookService::new(state.clone())),
        }
    }
}
