//! Product App — Write surface (non-execute).
//!
//! **Execute** goes through [`super::ConversationApp`] only.
//! Write refine tools: `agent_tools::skills::builtin::write_refine::tool_specs_for_pool`.

use contracts::auth_runtime::AuthContext;

/// Product marker / future write-task surface. Execution is via ConversationApp.
pub struct WriteApp<'a> {
    #[allow(dead_code)]
    pub(crate) chat: &'a app_chat::ChatContext,
    #[allow(dead_code)]
    pub(crate) auth: &'a AuthContext,
}

impl<'a> WriteApp<'a> {
    pub fn is_write_agent_type(agent_type: &str) -> bool {
        app_chat::is_write_agent_type(agent_type)
    }
}
