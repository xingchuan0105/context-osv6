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
//!
//! # Vocabulary (do not merge layers)
//!
//! | Term | Meaning |
//! |------|---------|
//! | **Tool** | Executable surface via `agent_tools::ToolCatalog` / `dispatch_tool` |
//! | **SkillMd** | Prompt-only `SKILL.md` body (`progressive::Skill` / `PromptRegistry`) |
//! | **SkillComponent** | Legacy name for **executable** builtins in `SkillRegistry` (is a Tool) |
//! | **Capability** | Mode/tool metadata + `PolicyEnforcer` (strategy truth for allow/deny) |
//! | **HostTool** | Orchestrator-only intercepts (`delegate_*`, `finish_answer`, …); never in ToolCatalog |
//! | **LoopHooks** | Context transforms only — **not** a second policy engine |

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
pub mod error_kind;
pub mod events;
pub mod helpers;
pub mod output_compiler;
pub mod product_contract;
pub mod progress;
pub mod react_loop;
pub mod runtime;
pub mod sse_sink;
pub mod untrusted_input;
pub mod lead_workers;
pub mod worker_contract;

#[cfg(feature = "eval")]
pub mod eval;
#[cfg(feature = "eval")]
pub mod redteam;

/// Alias used by existing code (`agents::r#loop`).
pub use react_loop as r#loop;

pub use events::{AgentEvent, AgentEventSink, AgentUsage, CollectingSink, NoopSink};
pub use lead_workers::{
    ActivatedCaps, Coverage, DocScopeSummary, EvidenceItem, EvidencePack, LeadPlanContext,
    MergedWebHit, MergedWebHits, PackGateOutcome, PreferredSource, SubTask, TaskBrief,
    TaskBriefGateError, apply_pack_gate, count_tool_ok, effective_web_queries,
    hits_to_evidence_items, merge_search_responses, validate_task_brief,
};
pub use react_loop::config::{ModeConfig, load_mode_config, load_system_prompt};
pub use react_loop::{
    BeforeToolCallOutcome, BridgeCallObs, DegradeReason, LoopContext, LoopHooks, LoopPolicy,
    LoopRuntimeDeps, ReActLoop, StandardLoopHooks, answer_contract, assembler, disclosure_plan,
    exit_policy,
};
pub use runtime::{
    Agent, AgentRequest, AgentRunResult, AgentRunUsage, AgentUserPreferences, EvaluationSignals,
    FinalDecision, IterationRecord, MAX_PROMPT_HISTORY_TURNS, recent_messages, stub_agent_auth,
};
pub use sse_sink::SseSink;
pub use worker_contract::RETRIEVAL_ALIAS_START_METADATA;

// Re-export rag scope helper for callers that used `loop::force_doc_scope` paths.
pub use agent_tools::force_doc_scope;
