//! ReAct agent loop + request/event types (TN Wave 6).
//!
//! - [`react_loop`] — `ReActLoop`, iteration, mode policy, answer contracts
//! - [`runtime`] — `AgentRequest`, `AgentRunResult`, `Agent` trait
//! - [`events`] — `AgentEvent` / sinks
//! - [`helpers`] — citation/codegen helpers used by the loop
//! - [`untrusted_input`] — scrub untrusted tool / observation text
//!
//! Tool execution stays in [`agent_tools`]. Orchestration (chat pipeline,
//! UnifiedAgent shell) remains in `app-chat`.
//!
//! Extension guide: crate-level `EXTENDING.md` (next to this crate’s `Cargo.toml`).

#![recursion_limit = "256"]

use serde::{Deserialize, Serialize};
use std::fmt;

/// Canonical agent mode kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentKind {
    Chat,
    Rag,
    Search,
    Write,
}

impl fmt::Display for AgentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentKind::Chat => write!(f, "chat"),
            AgentKind::Rag => write!(f, "rag"),
            AgentKind::Search => write!(f, "search"),
            AgentKind::Write => write!(f, "write"),
        }
    }
}

impl AgentKind {
    pub fn parse(agent_type: &str) -> Option<Self> {
        match agent_type.to_ascii_lowercase().as_str() {
            "chat" | "general" => Some(AgentKind::Chat),
            "rag" => Some(AgentKind::Rag),
            "search" => Some(AgentKind::Search),
            "write" => Some(AgentKind::Write),
            _ => None,
        }
    }

    pub fn as_canonical_str(&self) -> &'static str {
        match self {
            AgentKind::Chat => "chat",
            AgentKind::Rag => "rag",
            AgentKind::Search => "search",
            AgentKind::Write => "write",
        }
    }
}

pub mod audit;
pub mod cite_extract;
pub mod content_guard;
pub mod error_kind;
pub mod events;
pub mod helpers;
pub mod progress;
pub mod react_loop;
pub mod runtime;
pub mod sse_sink;
pub mod untrusted_input;

#[cfg(feature = "eval")]
pub mod eval;
#[cfg(feature = "eval")]
pub mod redteam;

/// Alias used by existing code (`agents::r#loop`).
pub use react_loop as r#loop;

pub use events::{AgentEvent, AgentEventSink, AgentUsage, CollectingSink, NoopSink};
pub use react_loop::config::{ModeConfig, load_mode_config, load_system_prompt};
pub use react_loop::{
    DegradeReason, LoopPolicy, ReActLoop, answer_contract, assembler, disclosure_plan, exit_policy,
};
pub use runtime::{
    Agent, AgentRequest, AgentRunResult, AgentRunUsage, AgentUserPreferences, EvaluationSignals,
    FinalDecision, IterationRecord, MAX_PROMPT_HISTORY_TURNS, recent_messages, stub_agent_auth,
};
pub use sse_sink::SseSink;

// Re-export rag scope helper for callers that used `loop::force_doc_scope` paths.
pub use agent_tools::force_doc_scope;
