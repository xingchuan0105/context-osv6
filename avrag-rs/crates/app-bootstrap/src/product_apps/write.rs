//! Product App — Write mode control ring.
//!
//! **Iron rule (ADR-0007 T2):** `write_refine_*` and write refine control ops are **never**
//! registered in `ToolCatalog` / mode `tool_pool` / Capabilities full table.
//! Execution path: chat pipeline → `app_chat::writer::run_write_mode` / refine loop in write-core.

use contracts::auth_runtime::AuthContext;

/// Product entry for writing tasks / refine loop. Independent of ReAct ToolCatalog.
pub struct WriteApp<'a> {
    pub(crate) chat: &'a app_chat::ChatContext,
    pub(crate) auth: &'a AuthContext,
}

impl<'a> WriteApp<'a> {
    /// Chat context used by write pipeline (sessions, LLM wiring).
    pub fn chat(&self) -> &'a app_chat::ChatContext {
        self.chat
    }

    pub fn auth(&self) -> &'a AuthContext {
        self.auth
    }

    /// Marker: write refine tools must not appear in ToolCatalog registration.
    pub const WRITE_REFINE_OUTSIDE_TOOL_CATALOG: bool = true;
}
