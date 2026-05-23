//! Strategy Layer — v5 independent state machines for each agent mode.
//!
//! Each mode (Chat/RAG/Search) implements the [`Strategy`] trait, defining
//! its own states and transitions. A generic [`StrategyExecutor`] drives any
//! state machine until termination.
//!
//! This replaces the v4 `ProgressiveLoop` + `LoopAdapter` fixed-phase architecture.

pub mod chat;
pub mod executor;
pub mod prompts;
pub mod rag;
pub mod search;

use crate::agents::error_kind::AgentErrorKind;
use crate::agents::events::AgentEventSink;
use crate::agents::react_loop::LoopBudget;
use crate::agents::runtime::AgentRunResult;
use common::AppError;
use tokio_util::sync::CancellationToken;

/// Outcome of executing one step in a Strategy state machine.
pub enum StepOutcome {
    /// Transition to the next state.
    Next(Box<dyn State>),
    /// Terminal — return the final result.
    Terminate(AgentRunResult),
}

/// Classification of states for generic executor handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateKind {
    /// Involves LLM planning/decision making.
    Plan,
    /// Pure tool execution, no LLM involvement.
    Execute,
    /// Evaluation/assessment of results.
    Evaluate,
    /// Final answer generation.
    Answer,
    /// Control flow (aggregation, decomposition, etc).
    Control,
}

/// All states in a Strategy must implement this interface.
///
/// The executor uses [`state_id`] for tracing/observability and [`state_kind`]
/// for generic handling (timeouts, cancellation checks, etc).
pub trait State: Send + std::any::Any {
    /// Unique identifier for this state (e.g. "plan", "execute_retrieve").
    fn state_id(&self) -> &'static str;

    /// Classification for generic executor handling.
    fn state_kind(&self) -> StateKind;

    /// Serialize for observability/debugging.
    fn to_observable(&self) -> serde_json::Value;

    /// Downcast helper for Strategy::step to recover concrete type.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Context shared across all strategies.
///
/// Each concrete strategy defines its own context type that implements this
/// trait, allowing the executor to inject generic capabilities.
pub trait StrategyContext: Send + Sync {
    fn trace_id(&self) -> &str;
    fn budget(&self) -> &LoopBudget;
    fn budget_mut(&mut self) -> &mut LoopBudget;
    fn sink(&self) -> &dyn AgentEventSink;
    fn cancel(&self) -> &CancellationToken;
    /// Organization ID for audit/logging.
    fn org_id(&self) -> Option<String>;
    /// Actor (user) ID for audit/logging.
    fn actor_id(&self) -> Option<String>;

    /// The original agent request, if available.
    /// Used by the executor to build replay snapshots.
    fn request(&self) -> Option<&crate::agents::runtime::AgentRequest> {
        None
    }

    /// Risk R3: every step must short-circuit if cancelled.
    fn check_cancelled(&self) -> Result<(), AppError> {
        if self.cancel().is_cancelled() {
            Err(AppError::Internal {
                code: "request_cancelled",
                message: "request cancelled".to_string(),
                http_status: 499,
            })
        } else {
            Ok(())
        }
    }
}

/// Strategy state machine interface.
///
/// Each mode (Chat/RAG/Search) implements this trait with its own state enum
/// and context type.
#[async_trait::async_trait]
pub trait Strategy: Send + Sync {
    type Context: StrategyContext;

    /// Initialize the state machine and return the first state.
    async fn init(
        &self,
        ctx: &mut Self::Context,
    ) -> Result<Box<dyn State>, AppError>;

    /// Execute one state, returning the next state or a terminal result.
    async fn step(
        &self,
        state: Box<dyn State>,
        ctx: &mut Self::Context,
    ) -> Result<StepOutcome, AgentErrorKind>;

    /// Return the static schema describing this strategy's states and transitions.
    fn schema() -> crate::agents::capability::StrategySchema
    where
        Self: Sized;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_kind_variants_exist() {
        // Smoke test that all variants are constructible
        let _ = StateKind::Plan;
        let _ = StateKind::Execute;
        let _ = StateKind::Evaluate;
        let _ = StateKind::Answer;
        let _ = StateKind::Control;
    }

    #[test]
    fn step_outcome_variants() {
        let result = AgentRunResult::default();
        let _ = StepOutcome::Terminate(result);
    }
}
