//! Declarative Skill Components — unified registry for atomic tools.
//!
//! Each tool is a `SkillComponent` that bundles:
//! - Index-tier:   name + "Load when..." description (router trigger)
//! - Load-tier:    full `ToolSpec` with JSON schema
//! - Runtime-tier: gotchas (negative examples) + execution logic
//!
//! This replaces the hard-coded `match` in `atomic_tools.rs` with a
//! registry-driven dispatch so adding a new tool only requires:
//!   1. Implement `SkillComponent` in `builtin/xxx.rs`
//!   2. Register it in `builtin/mod.rs`

pub mod registry;

pub mod builtin;
pub mod eval;
pub mod memory_dispatch;

pub use registry::{ExecutionContext, SkillRegistry};

use contracts::{ToolResult, ToolSpec};
use serde_json::Value;

/// A declarative Skill component that bundles description, schema, gotchas,
/// and execution logic for a single atomic tool.
#[async_trait::async_trait]
pub trait SkillComponent: Send + Sync {
    /// Unique tool identifier (e.g. "calculator").
    fn id(&self) -> &str;

    /// Semantic version (e.g. "1.0").
    fn version(&self) -> &str;

    /// Index-tier routing trigger — **not** documentation.
    ///
    /// Perplexity best practice: start with "Load when...", target ≤50 words,
    /// describe the user's intent using words from real queries.
    /// Every word here is paid by every session, every user.
    fn description(&self) -> &str;

    /// Load-tier full specification — JSON schema + rules.
    fn spec(&self) -> ToolSpec;

    /// Runtime-tier negative examples (gotchas).
    ///
    /// These are extremely high-signal content that guides the model on what
    /// **not** to do. Start thin, grow as the agent fails.
    fn gotchas(&self) -> &[&str] {
        &[]
    }

    /// Execute the tool with the given arguments.
    async fn execute<'a>(&self, args: &Value, ctx: &'a ExecutionContext<'a>) -> ToolResult;

    /// Frontend render hint — tells the UI how to present `ToolResult.data`.
    ///
    /// Known hints:
    /// - `"calculator"`  → expression + result layout
    /// - `"code"`        → stdout/stderr/result/exit_code layout
    /// - `"weather"`     → location + temperature/feels_like/humidity/wind grid
    /// - `"json"`        → generic pretty-printed JSON (default)
    fn render_hint(&self) -> &str {
        "json"
    }
}
