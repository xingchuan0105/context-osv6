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
    /// Output-compiler feedback continuations used this run (S2/E4). A worker
    /// loop (`ModeConfig.worker_handoff`) compiles each candidate final
    /// output at the `direct_content` decision point; on Error diagnostics the
    /// output is rejected and the loop continues ONCE with the rendered
    /// feedback as the next observation (bounded by
    /// `MAX_COMPILE_CONTINUATIONS` in content_dispatch). The correction turn
    /// is FREE — it does not consume the numbered iteration budget
    /// (`consumes_iteration_budget` in run_retrieval). When the continuation
    /// allowance is spent the output is accepted as final and the post-loop
    /// compile marks it degraded with diagnostic codes attached.
    pub compile_continuations: u8,
    /// K2: retrieval-log alias counter (`#1 #2 …`) for this run — one
    /// namespace per worker loop, incrementing across rounds/blocks. The
    /// sandbox bridge injects aliases in this order; downstream hydration
    /// replays the run's tool_results to resolve them.
    pub retrieval_aliases: std::sync::Arc<std::sync::atomic::AtomicU64>,
    /// A7: cross-block `save`/`load` workspace for this agent run.
    pub session_fs: std::sync::Arc<super::super::session_fs::SessionFs>,
    /// A3: allowed SaC methods for this run (empty = open).
    pub sdk_allowed: std::sync::Arc<std::collections::HashSet<String>>,
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

/// Exit reason marking a compile-feedback continuation (S2/E4). Shared by the
/// emission site (content_dispatch) and the budget accounting (run_retrieval).
/// Exit reason: host rejected a **structural** candidate final (worker handoff
/// compile). Free correction turn — does not consume numbered iteration budget.
/// Semantic stop (whether coverage is enough) is model+skill owned; see
/// `dispatch_content` DirectAnswer path and AGENTS.md prompt voice rules.
pub(crate) const COMPILE_FEEDBACK_EXIT_REASON: &str = "compile_feedback";

/// E4 (2026-07-28): whether a `Continue` outcome consumes a numbered
/// iteration. Compile-feedback continuations are FREE correction turns.
pub(crate) fn consumes_iteration_budget(outcome: &IterationOutcome) -> bool {
    outcome
        .record
        .as_ref()
        .is_none_or(|r| r.exit_reason != COMPILE_FEEDBACK_EXIT_REASON)
}

pub(crate) fn disclosed_skill_ids(disclosed: &DisclosedState) -> Vec<String> {
    disclosed.disclosed_skill_ids.iter().cloned().collect()
}
