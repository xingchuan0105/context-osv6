//! Agent orchestration surface for app-chat.
//!
//! Loop / request / SSE / audit types: [`agent_loop`].
//! Tool registry / skills / capability: [`agent_tools`].
//! This module keeps only orchestrator-owned pieces (UnifiedAgent, service).

pub use agent_loop::AgentKind;

pub mod service;
pub mod unified;

// Thin compat: prefer `agent_loop::untrusted_input` at new call sites.
pub use agent_loop::untrusted_input;
