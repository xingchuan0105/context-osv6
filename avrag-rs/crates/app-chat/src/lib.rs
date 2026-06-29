pub mod agents;
pub mod chat;
pub mod citations;
pub mod chat_private;
pub mod chat_streaming;
pub mod context;
pub mod agent_runtime;
pub mod i18n;
pub mod llm_context;
pub mod memory_helpers;
pub mod orchestrator_context;
#[cfg(feature = "eval")]
pub mod eval;
pub mod prompts;
pub mod rag_execute;
pub mod rag_prompts;
pub mod sessions;
pub mod token_budget;

mod chat_service;
mod external_agent_guide;

pub use agents::AgentKind;
pub use chat_service::ChatService;
pub use external_agent_guide::{attach_operation_guide, load_invoke_operation_guide};
pub use chat_streaming::{
    chunk_text_for_stream, emit_buffered_agent_answer_if_needed, chat_done_payload,
    stream_event_message_id, STREAM_PLACEHOLDER_MESSAGE_ID,
};
pub use context::ChatContext;
pub use llm_context::LlmContext;
pub use memory_helpers::{
    agent_icon, agent_name, build_answer, build_citations, build_degrade_trace, build_mode_debug,
    build_planner_output, build_sources, derive_profile_domains, derive_profile_topics,
    detect_preferred_style, estimate_token_count, merge_general_profile_custom_preferences,
    next_message_id, status_label,
};
pub use orchestrator_context::OrchestratorContext;
