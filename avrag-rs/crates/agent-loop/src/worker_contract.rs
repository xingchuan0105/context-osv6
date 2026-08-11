//! Shared metadata keys for multi-brief retrieval alias continuity.
//!
//! Historically used by orchestrator channel workers; Lead+Workers (2026-08-11)
//! reuses the same key when a Worker session needs a non-zero alias start.
//! See `docs/plans/2026-08-11-lead-rag-web-workers-design.md` §6.6.
//!
//! The loop **reads** these keys from `AgentRequest.metadata`.

/// `AgentRequest.metadata` key: `u64` start for retrieval-log alias counter.
///
/// Absent ⇒ 0 (byte-identical to pre-W1 single-brief behavior).
/// Set by `WorkerSession` when resuming a channel with prior briefs.
pub const RETRIEVAL_ALIAS_START_METADATA: &str = "retrieval_alias_start";

/// Product-side channel iteration cap (documented for cross-crate readers).
/// Enforced in `app-chat` `WorkerSession::CHANNEL_ITERATION_CAP`, not the loop.
pub const CHANNEL_ITERATION_CAP_DOC: u8 = 10;
