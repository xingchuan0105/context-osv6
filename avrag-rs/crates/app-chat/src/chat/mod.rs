// Chat orchestration module.
//
// All chat execution flows through a linear async pipeline:
//   preflight → resolve_session → dispatch_mode → output_guard → persist → usage
//   → notifications → terminal stream events.
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

pub(crate) use pipeline::{
    ChatExecution, ChatPreflight, execute_chat_pipeline, execute_chat_pipeline_stream,
};
pub(crate) use service::{BuildChatExecutionParams, build_chat_execution_from_result};
