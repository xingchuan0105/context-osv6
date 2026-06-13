//! Atomic tool executor for the UnifiedAgent.
//!
//! Dispatches calculator, code_interpreter, weather_query, and web_search tool calls
//! via the `SkillRegistry` so they can be used from any agent mode without
//! hard-coding the dispatch table.
//!
//! v5: All dispatch paths now run through `PolicyEnforcer` (standard rules) when
//! an `auth` context is provided.  The legacy no-auth paths use a permissive
//! enforcer so that existing tests and call-sites continue to work.

mod dispatch;

pub use dispatch::{
    dispatch_atomic_tool, dispatch_atomic_tool_with_enforcement, dispatch_atomic_tools,
    dispatch_atomic_tools_with_enforcement, dispatch_atomic_tools_with_provider,
};

#[cfg(test)]
mod tests;
