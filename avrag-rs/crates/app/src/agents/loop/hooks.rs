use avrag_llm::ChatMessage;

use super::assembler::LoopPhase;
use super::config::ModeConfig;
use crate::agents::runtime::AgentRequest;

pub struct LoopContext<'a> {
    pub mode: &'a ModeConfig,
    pub request: &'a AgentRequest,
    pub iteration: u8,
    pub phase: LoopPhase,
    pub has_retrieval_observation: bool,
    pub base_message_count: usize,
}

pub trait LoopHooks: Send + Sync {
    fn transform_context(&self, messages: &mut Vec<ChatMessage>, ctx: &LoopContext) {
        let _ = (messages, ctx);
    }

    fn convert_to_llm(&self, messages: &[ChatMessage]) -> Vec<ChatMessage> {
        messages.to_vec()
    }
}

pub struct StandardLoopHooks {
    pub max_react_messages: usize,
}

impl Default for StandardLoopHooks {
    fn default() -> Self {
        Self {
            max_react_messages: 20,
        }
    }
}

impl LoopHooks for StandardLoopHooks {
    fn transform_context(&self, messages: &mut Vec<ChatMessage>, ctx: &LoopContext) {
        if messages.len() > ctx.base_message_count + self.max_react_messages {
            let drain_end = messages.len() - self.max_react_messages;
            let drain_start = ctx.base_message_count;
            if drain_end > drain_start {
                messages.drain(drain_start..drain_end);
            }
        }
    }
}
