//! Product-facing answer / handoff contracts (Wave C1).
//!
//! **Boundary:** parse / validate / lift of user-visible answers and worker
//! handoff compile live here (via re-exports). The retrieve loop should not
//! grow new answer-format branches without an explicit product decision —
//! prefer extending these modules over sprinkling logic in `run_*`.
//!
//! Stable import paths remain:
//! - [`crate::answer_contract`] (same as `react_loop::answer_contract`)
//! - [`crate::output_compiler`]

pub use crate::output_compiler;
pub use crate::react_loop::answer_contract;
