//! Orchestrator + channel workers + chat exit (AGENT_ORCHESTRATOR_V1).
//!
//! Design: `docs/engineering/ORCHESTRATOR_SUBAGENT_CHAT_DESIGN_2026-07-16.md`

mod chat_exit;
mod host;
mod invariant;
mod materialize;
mod types;
mod workers;

pub use chat_exit::{direct_handoff, query_for_agent, synthesize_handoff};
pub use host::{
    orchestrator_v1_enabled, run_orchestrated_turn, AgentServiceExecutor, OrchestratedTurn,
    OrchestratorExecutor,
};
pub use invariant::{
    assert_complete, default_brief, looks_like_user_did_not_provide_doc, missing_dispatches,
    partial_notices_from_packs, MissingChannels,
};
pub use materialize::materialize_channels;
pub use types::*;
pub use workers::{attach_worker_evidence, pack_error, pack_from_run};
