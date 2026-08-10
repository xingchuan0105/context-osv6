//! Loop policy deep module: mode config, exit gates, and disclosure planning.
//!
//! Public surface is [`LoopPolicy`] (≤3 methods). Submodule items remain
//! reachable via `policy::config`, `policy::exit_policy`, and `policy::disclosure_plan`.

pub mod config;
pub mod disclosure_plan;
pub mod exit_policy;

pub use config::*;
pub use disclosure_plan::*;
pub use exit_policy::*;

/// Facade for loop policy decisions — callers use this instead of reaching into submodules.
pub struct LoopPolicy;

impl LoopPolicy {
    /// Load YAML mode configuration for a canonical mode id (`rag`, `search`, `chat`).
    pub fn load_mode(mode_id: &str) -> Result<ModeConfig, common::AppError> {
        config::load_mode_config(mode_id)
    }

    /// Decide whether synthesis should run after the retrieve loop ends.
    pub fn synthesis_gate(
        loop_exit: &LoopExitConfig,
        has_evidence: bool,
        direct_answer: Option<&str>,
        tool_results: &[contracts::ToolResult],
        query: &str,
    ) -> SynthesisGate {
        exit_policy::decide_synthesis_gate(
            loop_exit,
            has_evidence,
            direct_answer,
            tool_results,
            query,
        )
    }

    /// Plan progressive disclosure slices for a retrieve round.
    pub fn plan_retrieve(
        mode: &ModeConfig,
        first_round: bool,
        skill_request: Option<&[String]>,
        already_disclosed: &std::collections::HashSet<String>,
        request: Option<&crate::runtime::AgentRequest>,
        reexpose_kb_api_detail: bool,
    ) -> DisclosurePlan {
        DisclosurePlanner::plan_retrieve(
            mode,
            first_round,
            skill_request,
            already_disclosed,
            request,
            reexpose_kb_api_detail,
        )
    }
}

/// D9: derive the mandatory retrieve skill list straight from capability flags.
///
/// The mode YAML `skill_catalog.mandatory` indirect layer is gone.
/// `knowledge-base` / `search` are mandatory only when the matching capability
/// is mounted. **Memory is not mandatory full-body** (2026-08-10): agent-base
/// already points at `client.history` / `user_profile` and
/// `{"skill_request":["memory"]}` for the thick body — avoids per-round tax
/// on pure fact questions.
pub fn derive_mandatory_retrieve(rag: bool, search: bool) -> Vec<String> {
    let mut ids = Vec::new();
    if rag {
        ids.push("knowledge-base".to_string());
    }
    if search {
        ids.push("search".to_string());
    }
    ids
}
