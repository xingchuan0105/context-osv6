//! Agent output compiler (design 2026-07-27 §4): rustc-style diagnostics for
//! agent-produced structured output.
//!
//! The compiler parses + structurally validates an agent's final output and
//! produces machine-readable diagnostics (code + location + fix suggestion).
//! Error-severity diagnostics at the loop's `direct_content` decision point
//! turn into ONE compile-feedback continuation (the diagnostics rendered as a
//! compact user message); warnings never block. The same compile channel
//! serves both trigger points — mid-loop direct content and the C5
//! budget-exhausted final turn (post-loop, no continuation).
//!
//! v1 serves ONLY the worker handoff (`internal_worker_handoff_v1`); other
//! output types (skill_request, codegen, answer contract) are explicitly out
//! of scope. The hard gate checks structure only (schema, pointer truthfulness,
//! fabrication stripping) — never content semantics (§5.3).

mod handoff;
mod types;

pub use handoff::{HandoffCompileInput, compile_handoff, strip_code_execution_blocks};
pub use types::{CompileOutcome, Diagnostic, Severity};

#[cfg(test)]
mod tests;
