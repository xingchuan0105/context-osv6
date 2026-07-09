//! Write-mode domain crate (ADR 0006 — split plan **Accepted**).
//!
//! Owns material-pack / refine types / pure helpers / **WriteRefineLoopRunner**
//! behind research/mode/activity ports. app-chat supplies adapters
//! (`SubagentInvoker`, ModeConfig, AgentEventSink) and `run_write_mode` entry.
//! See `docs/adr/0006-write-heavytail-crate-split-plan.md`.

mod contract;
mod material_pack;
mod message_format;
mod ports;
mod refine_helpers;
mod refine_loop;
mod refine_types;

pub use contract::{
    WRITE_AGENT_TYPE, WRITE_MODE, is_write_internal_feature_tag, require_non_empty_write_topic,
    write_usage_is_unified_billing,
};
pub use material_pack::{MaterialCardView, MaterialPack, ResearchMaterials};
pub use message_format::{build_assistant_message_with_tool_calls, build_tool_message};
pub use ports::{
    WriteActivitySink, WriteParentMeta, WriteRefineModeHost, WriteResearchHit, WriteResearchKind,
    WriteResearchPort,
};
pub use refine_helpers::{
    build_write_refine_round_counter_zh, checkpoint_refine, core_lexical_bands_met,
    core_lexical_bands_unmet, parse_sentence_id_args, should_prefer_current_workspace,
    strip_task_section, synthesize_force_lexical_call, tool_error,
};
pub use refine_loop::WriteRefineLoopRunner;
pub use refine_types::{
    BestSnapshot, FinishReason, RefineContext, RefineLoopBudget, WRITE_REFINE_GATE_MAX_REVISE,
    WRITE_REFINE_HARD_REACT_CAP,
};
