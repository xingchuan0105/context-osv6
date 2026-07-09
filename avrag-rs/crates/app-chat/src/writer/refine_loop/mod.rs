//! WriteRefine loop: re-export from write-core + app-chat wiring helpers.

pub mod types {
    pub use write_core::{
        BestSnapshot, FinishReason, RefineContext, RefineLoopBudget, WRITE_REFINE_GATE_MAX_REVISE,
        WRITE_REFINE_HARD_REACT_CAP,
    };
}

pub use types::{
    BestSnapshot, FinishReason, RefineContext, RefineLoopBudget, WRITE_REFINE_GATE_MAX_REVISE,
    WRITE_REFINE_HARD_REACT_CAP,
};
pub use write_core::WriteRefineLoopRunner;

use common::AppError;
use heavytail::llm::WriterLlm;
use heavytail::state::WriterState;
use heavytail::StyleParams;

use crate::agents::events::AgentEventSink;
use crate::agents::runtime::AgentRequest;
use crate::writer::adapters::{
    parent_meta_from_request, AgentWriteActivitySink, AppWriteRefineMode, SubagentResearchPort,
};
use crate::writer::invoker::SubagentInvoker;

/// Build and run WriteRefine with app-chat adapters (orchestrator entry).
pub async fn run_write_refine(
    llm: &WriterLlm,
    invoker: &SubagentInvoker,
    parent_request: &AgentRequest,
    style: StyleParams,
    budget: RefineLoopBudget,
    ctx: &mut RefineContext,
    reservoir: &[String],
    state: &mut WriterState,
    sink: &dyn AgentEventSink,
    job_dir: &std::path::Path,
) -> Result<(), AppError> {
    let mode = AppWriteRefineMode::load()
        .map_err(|e| AppError::internal(format!("write_refine mode config load failed: {e}")))?;
    let research = SubagentResearchPort {
        invoker,
        parent: parent_request,
    };
    let parent = parent_meta_from_request(parent_request);
    let runner = WriteRefineLoopRunner::new(llm, &research, &mode, parent, style, budget);
    let activity = AgentWriteActivitySink { inner: sink };
    runner
        .run(ctx, reservoir, state, &activity, job_dir)
        .await
}

#[cfg(test)]
mod tests;
