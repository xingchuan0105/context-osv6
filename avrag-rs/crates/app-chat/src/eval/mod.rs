//! Feature-gated evaluation framework (outside main agent tree).
#![cfg(feature = "eval")]

mod compare;
mod llm_judge;
mod metrics;
mod runner;
mod types;

pub mod framework;

pub use compare::*;
pub use llm_judge::LlmAsJudgeEvaluator;
pub use runner::*;
pub use types::*;
