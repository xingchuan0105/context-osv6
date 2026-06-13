use app_core::{MemoryState, StoredDocument};
use contracts::documents::DocumentStatus;
use contracts::notebooks::ChatSession;

use crate::context::ChatContext;

impl ChatContext {
    pub(crate) async fn list_ready_documents_for_chat(
        &self,
        notebook_id: &str,
        doc_scope: &[String],
    ) -> Vec<StoredDocument> {
        let state = self.storage.inner().read().await;
        state
            .documents
            .values()
            .filter(|stored| stored.document.notebook_id == notebook_id)
            .filter(|stored| matches!(stored.document.status, DocumentStatus::Completed))
            .filter(|stored| doc_scope.is_empty() || doc_scope.contains(&stored.document.id))
            .cloned()
            .collect()
    }

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
