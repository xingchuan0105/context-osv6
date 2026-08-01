//! Orchestrator + channel workers + chat exit (Product Agent Dispatch → Answer).
//!
//! Design: `docs/engineering/ORCHESTRATOR_SUBAGENT_CHAT_DESIGN_2026-07-16.md`
//! Evidence store: `docs/engineering/ORCHESTRATOR_V2_REACT_EVIDENCE_STORE_DESIGN_2026-07-18.md`

mod brain;
mod chat_exit;
mod fact_verify;
mod host;
mod host_tools;
mod invariant;
mod materialize;
mod selected;
mod store;
mod types;
mod worker_session;
mod workers;

pub use host_tools::{
    CONVERSATION_HISTORY_LOAD, DELEGATE_RAG, DELEGATE_SEARCH, EVIDENCE_FETCH, FINISH_ANSWER,
    HOST_ONLY_TOOL_NAMES, HOST_TOOL_NAMES,
};

pub use brain::{orchestrator_v2_enabled, run_llm_orchestrated_turn};
pub use chat_exit::{
    direct_handoff, query_for_agent, render_synthesize_context, synthesize_handoff,
};
pub use fact_verify::verify_handoff_facts;
pub use host::{
    AgentServiceExecutor, OrchestratedTurn, OrchestratorExecutor, run_orchestrated_turn,
};
pub use invariant::{
    MissingChannels, assert_complete, default_brief, looks_like_user_did_not_provide_doc,
    missing_dispatches, partial_notices_from_records,
};
pub use materialize::materialize_channels;
pub use selected::{
    HydratedChunk, alias_chunks_in_order, hydrate_selected, parse_selected_aliases,
};
pub use store::{EvidenceEntry, EvidenceKind, EvidenceListing, EvidenceStore, SourceDoc};
pub use types::*;
pub use worker_session::{BriefOutcome, BriefRecord, SessionError, WorkerSession};
pub use workers::{
    WorkerBriefObservability, WorkerIterationObs, WorkerRunObservability, WorkerThinkingStep,
    WorkerToolObs, attach_store_retrieval_tool_results, attach_worker_thinking_events,
    channel_note_from_run, finalize_answer_evidence, parse_worker_handoff, tool_failures,
    worker_handoff_from_run, worker_observability_from_run,
};
