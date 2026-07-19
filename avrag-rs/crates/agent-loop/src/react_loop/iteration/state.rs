use avrag_llm::ChatMessage;
use contracts::ToolResult;

use super::super::assembler::DisclosedState;
use super::super::telemetry::ReActIterationRecord;

pub struct IterationState {
    pub messages: Vec<ChatMessage>,
    pub disclosed: DisclosedState,
    pub tool_results: Vec<ToolResult>,
    pub total_tool_calls: u32,
    pub consecutive_sandbox_errors: u8,
    pub reasoning_acc: String,
    /// True when retrieve already emitted live `MessageDelta`s (stream path).
    /// `finish_direct_answer_run` must not re-emit the whole answer as one token.
    pub answer_deltas_streamed: bool,
}

pub enum IterationControl {
    Continue,
    BreakToSynthesis { reason: String },
    DirectAnswer { content: String },
}

pub struct IterationOutcome {
    pub control: IterationControl,
    pub record: Option<ReActIterationRecord>,
    /// Sandbox break emits telemetry inline and skips TurnEnd/record (legacy behavior).
    pub sandbox_break: bool,
}

pub(crate) fn disclosed_skill_ids(disclosed: &DisclosedState) -> Vec<String> {
    disclosed.disclosed_skill_ids.iter().cloned().collect()
}
