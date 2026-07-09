//! Eval Framework — re-export hub for backward compatibility.
//!
//! Supports ground-truth comparison and LLM-as-judge scoring.

pub use super::compare::*;
pub use super::llm_judge::LlmAsJudgeEvaluator;
pub use super::runner::*;
pub use super::types::*;
