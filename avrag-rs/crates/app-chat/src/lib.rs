// Large async agent layouts (UnifiedAgent / ReActLoop) exceed default query depth on rustc 1.96.
#![recursion_limit = "256"]

//! Chat product orchestrator: sessions, pipeline, UnifiedAgent shell, writer glue.
//!
//! - Loop platform: [`agent_loop`] (see `agent-loop/EXTENDING.md`)
//! - Tools: [`agent_tools`] (`ToolCatalog` / `dispatch_tool` only)
//! - Do not re-grow tool match arms or loop forks in this crate.

pub mod agent_runtime;
pub mod agents;
pub mod capabilities;
pub mod chat;
pub mod chat_private;
pub mod chat_streaming;
pub mod citations;
pub mod context;
pub mod mode_assemble;
/// Eval harness lives in `agent_loop`; re-export only when the `eval` feature is on.
#[cfg(feature = "eval")]
pub use agent_loop::eval;
pub mod i18n;
pub mod llm_context;
pub mod memory_helpers;
pub mod orchestrator_context;
pub mod prompts;
pub mod profile_update;
pub mod rag_execute;
pub mod rag_prompts;
pub mod sessions;
#[cfg(any(test, feature = "dev-tools"))]
pub mod token_budget;
pub mod writer;

mod chat_service;
mod external_agent_guide;

pub use agents::AgentKind;
pub use capabilities::{CapabilitySet, resolve_capabilities, write_disabled_error};
pub use chat::{is_reserved_internal_agent_type, is_write_agent_type};
pub use chat_service::ChatService;
pub use chat_streaming::{
    STREAM_PLACEHOLDER_MESSAGE_ID, chat_done_payload, chunk_text_for_stream,
    emit_buffered_agent_answer_if_needed, stream_event_message_id,
};
pub use context::ChatContext;
pub use external_agent_guide::{attach_operation_guide, load_invoke_operation_guide};
pub use llm_context::LlmContext;
pub use memory_helpers::{
    agent_icon, agent_name, build_answer, build_citations, build_degrade_trace, build_mode_debug,
    build_planner_output, build_sources, estimate_token_count, next_message_id, status_label,
};
pub use mode_assemble::{AssembledMode, assemble_mode};
pub use orchestrator_context::OrchestratorContext;
