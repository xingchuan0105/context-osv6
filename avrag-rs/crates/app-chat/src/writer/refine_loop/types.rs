//! Re-export refine loop types from `write-core` (ADR 0006 crate split).

pub use write_core::{
    BestSnapshot, FinishReason, RefineContext, RefineLoopBudget, WRITE_REFINE_GATE_MAX_REVISE,
    WRITE_REFINE_HARD_REACT_CAP,
};
