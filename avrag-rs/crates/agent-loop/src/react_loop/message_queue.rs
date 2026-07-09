use std::collections::VecDeque;

use avrag_llm::ChatMessage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QueueDrainMode {
    #[default]
    OneAtATime,
    All,
}

#[derive(Debug, Clone, Default)]
pub struct LoopMessageQueue {
    _steering: VecDeque<ChatMessage>,
    _follow_up: VecDeque<ChatMessage>,
    pub steering_mode: QueueDrainMode,
    pub follow_up_mode: QueueDrainMode,
}

impl LoopMessageQueue {
    pub fn new() -> Self {
        Self {
            steering_mode: QueueDrainMode::OneAtATime,
            follow_up_mode: QueueDrainMode::OneAtATime,
            ..Default::default()
        }
    }

    /// v0.1 placeholder — steering mid-turn injection deferred to ADR-0008 v0.2.
    pub fn drain_steering_before_turn(&mut self) -> Vec<ChatMessage> {
        Vec::new()
    }
}
