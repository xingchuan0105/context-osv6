use app_core::MemoryState;
use contracts::workspaces::ChatSession;

use crate::context::ChatContext;

impl ChatContext {
    pub(crate) fn current_owner_user_id(&self) -> String {
        self.auth.user_id().to_string()
    }

    pub fn memory_session_visible(&self, state: &MemoryState, session: &ChatSession) -> bool {
        state
            .workspaces
            .get(&session.workspace_id)
            .map(|notebook| notebook.owner_user_id == self.current_owner_user_id())
            .unwrap_or(false)
    }
}
