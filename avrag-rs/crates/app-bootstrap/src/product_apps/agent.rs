//! Product App — Agent (Chat / RAG / Search). Tools execute only via ToolCatalog::dispatch_tool.
//! Write is **not** part of this App (see WriteApp).

use contracts::auth_runtime::AuthContext;

/// Product entry for chat/RAG/search sessions. Domain logic stays in `app_chat`.
pub struct AgentApp<'a> {
    pub(crate) chat: &'a app_chat::ChatContext,
    pub(crate) auth: &'a AuthContext,
}

impl<'a> AgentApp<'a> {
    /// Underlying chat context (sessions, pipeline). Prefer going through this App from transport.
    pub fn chat(&self) -> &'a app_chat::ChatContext {
        self.chat
    }

    pub fn auth(&self) -> &'a AuthContext {
        self.auth
    }
}
