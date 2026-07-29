//! Shared metadata keys between channel workers (`app-chat` orchestrator) and
//! [`crate::ReActLoop`] (Wave C3).
//!
//! Workers seed `AgentRequest.metadata` so multi-brief sessions keep unique
//! retrieval-log aliases and budgets. The loop **reads** these keys; product
//! ownership of channel sessions stays in `app-chat`.

/// `AgentRequest.metadata` key: `u64` start for retrieval-log alias counter.
///
/// Absent ⇒ 0 (byte-identical to pre-W1 single-brief behavior).
/// Set by `WorkerSession` when resuming a channel with prior briefs.
pub const RETRIEVAL_ALIAS_START_METADATA: &str = "retrieval_alias_start";

/// Product-side channel iteration cap (documented for cross-crate readers).
/// Enforced in `app-chat` `WorkerSession::CHANNEL_ITERATION_CAP`, not the loop.
pub const CHANNEL_ITERATION_CAP_DOC: u8 = 10;
