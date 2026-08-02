use avrag_rag_core::context::SessionContext as RagSessionContext;
use contracts::chat::ChatMessage;

use crate::context::ChatContext;

impl ChatContext {
    pub fn build_rag_session_context(messages: Vec<ChatMessage>) -> Option<RagSessionContext> {
        if messages.is_empty() {
            None
        } else {
            Some(RagSessionContext { messages })
        }
    }
}

