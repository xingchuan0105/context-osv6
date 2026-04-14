use crate::AppState;

#[derive(Clone)]
pub struct NotebookService {
    state: AppState,
}

impl NotebookService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }
}
