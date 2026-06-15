use app_core::MemoryState;
use contracts::notebooks::ChatSession;

use crate::context::ChatContext;

impl ChatContext {
    pub(crate) fn current_org_id(&self) -> String {
        self.auth.org_id().to_string()
    }

    pub fn memory_session_visible(
        &self,
        state: &MemoryState,
        session: &ChatSession,
    ) -> bool {
        state
            .notebooks
            .get(&session.notebook_id)
            .map(|notebook| notebook.org_id == self.current_org_id())
            .unwrap_or(false)
    }
}
