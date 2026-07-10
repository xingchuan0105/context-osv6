// Chat orchestration module.
//
// All chat execution flows through a linear async pipeline:
//   preflight → resolve_session → dispatch_agent_mode | run_write_mode
//   → output_guard → persist → usage → notifications → terminal stream events.
//
// Rationale:
// - Chat orchestration is intrinsically static and linear; an external graph
//   framework added complexity (HashMap-typed context, error bridging) without
//   delivering on dynamic-routing or persistence promises.
// - Dynamic state-machine behavior (plan / retrieve / react loops) belongs at
//   the agent layer, where each agent owns its own bounded loop with strongly
//   typed state — not in the chat coordinator.
//
// If you need to bypass the pipeline for testing, use the test harness in
// `pipeline_tests.rs` instead of reintroducing a second production path.

// i18n lives at crate root (`app_chat::i18n`).
mod pipeline;
mod pipeline_steps;
mod service;

#[cfg(test)]
mod pipeline_tests;

pub use pipeline::{is_reserved_internal_agent_type, is_write_agent_type};
pub(crate) use pipeline::{
    ChatExecution, ChatPreflight, PipelineLane, StreamConfig, execute_pipeline,
    execute_pipeline_stream,
};
pub(crate) use pipeline_steps::attach_debug_trace_from_sink;
pub(crate) use service::{BuildChatExecutionParams, build_chat_execution_from_result};
