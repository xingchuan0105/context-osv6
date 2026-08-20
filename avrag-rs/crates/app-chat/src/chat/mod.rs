// Chat orchestration module.
//
// All chat execution flows through a linear async pipeline:
//   preflight → resolve_session → Start event → dispatch_agent_mode | run_write_mode
//   → audit → output_guard → persist → terminal stream events
//   → usage → attach operation guide.
//
// Notes on the real order (locked by `pipeline_spine_locks_audit_before_persist`):
// - The audit record is appended right after the agent step, before the output
//   guard runs.
// - Persist runs before terminal stream events so Done carries the real PG
//   message_id (citation lookup / frontend chips). Share turns skip persist;
//   those Done events still use STREAM_PLACEHOLDER_MESSAGE_ID (0).
//   The spine test locks audit → persist, Done-presence, and Done.message_id > 0
//   on the persisted (non-share) path. The event stream is drained only after
//   the pipeline completes, so it cannot prove persist-before-Done by marker
//   interleaving; message_id > 0 is the observable consequence of that order.
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
mod service_modes;
mod service_postprocess;

#[cfg(test)]
mod pipeline_tests;

pub(crate) use pipeline::{
    ChatExecution, ChatPreflight, PipelineLane, StreamConfig, execute_pipeline,
    execute_pipeline_stream,
};
pub use pipeline::{is_reserved_internal_agent_type, is_write_agent_type};
pub(crate) use pipeline_steps::attach_debug_trace_from_sink;
pub(crate) use service_modes::{BuildChatExecutionParams, build_chat_execution_from_result};
