//! Write-mode domain crate (ADR 0006).
//!
//! Owns material-pack / refine budget contracts and pure Write mode rules.
//! Chat pipeline orchestration (`run_write_mode`, subagent invoker, ChatContext
//! glue) remains in `app-chat` and depends on this crate.

mod contract;
mod material_pack;
mod refine_types;

pub use contract::{
    WRITE_AGENT_TYPE, WRITE_MODE, require_non_empty_write_topic, write_usage_is_unified_billing,
};
pub use material_pack::{MaterialCardView, MaterialPack, ResearchMaterials};
pub use refine_types::{
    BestSnapshot, FinishReason, RefineContext, RefineLoopBudget, WRITE_REFINE_GATE_MAX_REVISE,
    WRITE_REFINE_HARD_REACT_CAP,
};
