//! Mid-turn message queues (steering / follow-up).
//!
//! # Status (Wave A2, 2026-07-29)
//!
//! **Placeholder only — not productized.** HTTP one-shot SaaS turns do not
//! inject mid-loop user messages. Source historically deferred steering to
//! “ADR-0008 v0.2”; that product capability is **not scheduled**. Do **not**
//! delete this module without an explicit ADR decision (plan Wave D8).
//!
//! See: `docs/plans/2026-07-29-pi-informed-agent-architecture-optimization.md` A2/D8,
//! `docs/adr/0008-query-normalization-and-answer-contract.md` (steering note).

use std::collections::VecDeque;

use avrag_llm::ChatMessage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QueueDrainMode {
    #[default]
    OneAtATime,
    All,
}

/// Placeholder dual queue (steering + follow-up). Not wired into `ReActLoop::run`.
#[deprecated(
    note = "SaaS one-shot turns do not use steering/follow-up; deferred product work \
            (see module docs / plan 2026-07-29 A2). Do not wire without product decision."
)]
#[derive(Debug, Clone, Default)]
pub struct LoopMessageQueue {
    _steering: VecDeque<ChatMessage>,
    _follow_up: VecDeque<ChatMessage>,
    pub steering_mode: QueueDrainMode,
    pub follow_up_mode: QueueDrainMode,
}

#[allow(deprecated)]
impl LoopMessageQueue {
    pub fn new() -> Self {
        Self {
            steering_mode: QueueDrainMode::OneAtATime,
            follow_up_mode: QueueDrainMode::OneAtATime,
            ..Default::default()
        }
    }

    /// Placeholder — always empty until product ships multi-turn inject.
    pub fn drain_steering_before_turn(&mut self) -> Vec<ChatMessage> {
        Vec::new()
    }
}
