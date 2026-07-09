//! Write-mode domain crate (ADR 0006).
//!
//! Owns material-pack / refine types / pure refine helpers and Write contracts.
//! Agent-coupled glue (`run_write_mode`, `SubagentInvoker`, `WriteRefineLoopRunner`,
//! ChatContext) stays in `app-chat` and depends on this crate.

mod contract;
mod material_pack;
mod refine_helpers;
mod refine_types;

pub use contract::{
    WRITE_AGENT_TYPE, WRITE_MODE, is_write_internal_feature_tag, require_non_empty_write_topic,
    write_usage_is_unified_billing,
};
pub use material_pack::{MaterialCardView, MaterialPack, ResearchMaterials};
pub use refine_helpers::{
    build_write_refine_round_counter_zh, checkpoint_refine, core_lexical_bands_met,
    core_lexical_bands_unmet, parse_sentence_id_args, should_prefer_current_workspace,
    strip_task_section, synthesize_force_lexical_call, tool_error,
};
pub use refine_types::{
    BestSnapshot, FinishReason, RefineContext, RefineLoopBudget, WRITE_REFINE_GATE_MAX_REVISE,
    WRITE_REFINE_HARD_REACT_CAP,
};
