//! Orchestrator + channel workers + chat exit (AGENT_ORCHESTRATOR_V1).
//!
//! Design: `docs/engineering/ORCHESTRATOR_SUBAGENT_CHAT_DESIGN_2026-07-16.md`
//! Evidence store: `docs/engineering/ORCHESTRATOR_V2_REACT_EVIDENCE_STORE_DESIGN_2026-07-18.md`

mod brain;
mod chat_exit;
mod host;
mod invariant;
mod materialize;
mod store;
mod types;
mod workers;

pub use brain::{orchestrator_v2_enabled, run_llm_orchestrated_turn};
pub use chat_exit::{direct_handoff, query_for_agent, render_synthesize_context, synthesize_handoff};
pub use host::{
    orchestrator_v1_enabled, run_orchestrated_turn, AgentServiceExecutor, OrchestratedTurn,
    OrchestratorExecutor,
};
pub use invariant::{
    assert_complete, default_brief, looks_like_user_did_not_provide_doc, missing_dispatches,
    partial_notices_from_records, MissingChannels,
};
pub use materialize::materialize_channels;
pub use store::{EvidenceEntry, EvidenceKind, EvidenceListing, EvidenceStore, SourceDoc};
pub use types::*;
pub use workers::{
    channel_note_from_run, finalize_answer_evidence, parse_worker_handoff, tool_failures,
    worker_handoff_from_run,
};
