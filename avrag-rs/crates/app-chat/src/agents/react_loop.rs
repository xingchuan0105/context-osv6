//! ReAct loop skeleton — shared primitives for `RagAgent` and `WebSearchAgent`.
//!
//! Per `docs/CHAT_GRAPHFLOW_REMOVAL_AND_AGENT_REACT_2026-05-10.md` §4.1, this
//! module supplies the type-safe scaffolding for bounded ReAct iteration:
//!
//! - [`LoopBudget`]: enforces a tier-aware iteration ceiling via
//!   [`avrag_billing::ReactLoopBudgetPolicy`]. Free: RAG/Search/Chat = 2.
//!   Plus/Pro: RAG = 4, Search = 3, Chat = 3.
//! - [`LoopDecision`]: the only way to advance — `Continue` *requires* fresh
//!   `new_params`, so a fallback that does not change inputs cannot type-check
//!   (decision ⑦).
//! - [`NextStep`]: enumerates the recoverable transitions.
//! - [`DegradeReason`] + activity helpers: standardise observability so the
//!   evaluator and agents agree on telemetry strings.
//!
//! Each agent owns its own state struct and its own params type `P`; this
//! module is intentionally generic over `P` so that RAG and Search can share
//! the decision schema without sharing fields.

use crate::agents::events::{AgentEvent, AgentEventSink};
use avrag_billing::{BillingTier, ReactLoopAgentMode, ReactLoopBudgetPolicy};
use common::AppError;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

/// Canonical billing tier (re-export for agent callers).
pub use avrag_billing::BillingTier as UserTier;

/// Cancellation-aware shared context passed to every step in a ReAct loop.
///
/// The sink is `&dyn AgentEventSink` so steps can be exercised with both the
/// real `SseSink` (production) and `CollectingSink` (tests) without changes.
pub struct ReactContext<'a> {
    pub sink: &'a dyn AgentEventSink,
    pub cancel: &'a CancellationToken,
    pub trace_id: &'a str,
}

impl<'a> ReactContext<'a> {
    pub fn new(
        sink: &'a dyn AgentEventSink,
        cancel: &'a CancellationToken,
        trace_id: &'a str,
    ) -> Self {
        Self {
            sink,
            cancel,
            trace_id,
        }
    }

    /// Risk R3: every step must short-circuit if the orchestrator has cancelled.
    pub fn check_cancelled(&self) -> Result<(), AppError> {
        if self.cancel.is_cancelled() {
            Err(cancellation_error())
        } else {
            Ok(())
        }
    }

    /// Emit a state-transition activity event (best-effort; sink errors are
    /// non-fatal because clients may have already disconnected).
    pub async fn emit_activity(&self, stage: &str, message: impl Into<String>) {
        let _ = self
            .sink
            .emit(AgentEvent::Activity {
                stage: stage.to_string(),
                message: message.into(),
            })
            .await;
    }
}

/// Iteration budget — see decision ④. Limits are sourced from billing policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopBudget {
    pub max_iterations: u8,
    pub current: u8,
}

impl LoopBudget {
    pub fn new(max_iterations: u8) -> Self {
        Self {
            max_iterations,
            current: 0,
        }
    }

    /// Tier-based RAG budget from [`ReactLoopBudgetPolicy`].
    pub fn rag(tier: BillingTier) -> Self {
        Self::new(ReactLoopBudgetPolicy::max_iterations(
            ReactLoopAgentMode::Rag,
            tier,
        ))
    }

    /// Tier-based RAG budget resolved from a raw subscription `plan_id`.
    pub fn rag_for_plan(plan_id: &str) -> Self {
        Self::rag(BillingTier::from_plan_id(plan_id))
    }

    /// Tier-based Search budget from [`ReactLoopBudgetPolicy`].
    pub fn search(tier: BillingTier) -> Self {
        Self::new(ReactLoopBudgetPolicy::max_iterations(
            ReactLoopAgentMode::Search,
            tier,
        ))
    }

    /// Tier-based Search budget resolved from a raw subscription `plan_id`.
    pub fn search_for_plan(plan_id: &str) -> Self {
        Self::search(BillingTier::from_plan_id(plan_id))
    }

    /// Tier-based Chat budget from [`ReactLoopBudgetPolicy`].
    pub fn chat(tier: BillingTier) -> Self {
        Self::new(ReactLoopBudgetPolicy::max_iterations(
            ReactLoopAgentMode::Chat,
            tier,
        ))
    }

    /// Tier-based Chat budget resolved from a raw subscription `plan_id`.
    pub fn chat_for_plan(plan_id: &str) -> Self {
        Self::chat(BillingTier::from_plan_id(plan_id))
    }

    pub fn exhausted(&self) -> bool {
        self.current >= self.max_iterations
    }

    pub fn remaining(&self) -> u8 {
        self.max_iterations.saturating_sub(self.current)
    }

    /// Advance the iteration counter. Saturates at `u8::MAX` to avoid panics.
    pub fn tick(&mut self) {
        self.current = self.current.saturating_add(1);
    }
}

/// Where the loop should branch on a `Continue` decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NextStep {
    /// Re-plan the queries with a fresh planner pass.
    Replan,
    /// Reuse the plan, broaden the existing query (drop modifiers / add synonyms).
    BroadenQuery,
    /// Search-only: switch Brave vertical (general → news / discussions).
    EscalateVertical,
    /// RAG → Search escalation: hand off when local recall is empty.
    EscalateToSearch,
    /// Search-only stub (decision ⑤): signal kept for future implementation.
    FetchFullPage,
}

impl NextStep {
    /// Stage string emitted with `Activity { stage, .. }` events when the loop
    /// continues via this branch. Stable identifiers for telemetry.
    pub fn activity_stage(&self) -> &'static str {
        match self {
            NextStep::Replan => "replanning",
            NextStep::BroadenQuery => "broadening_query",
            NextStep::EscalateVertical => "escalating_vertical",
            NextStep::EscalateToSearch => "escalating_to_search",
            NextStep::FetchFullPage => "fetching_full_page",
        }
    }
}

pub use common::DegradeReason;

/// Outcome of a single ReAct iteration's evaluator.
///
/// Decision ⑦ is encoded here: `Continue` requires fresh `new_params: P`, so
/// a fallback that fails to mutate inputs cannot type-check. The compiler
/// cannot prove the new params actually differ from the previous ones, but
/// the signature forces the caller to construct a fresh value at every site.
#[derive(Debug, Clone)]
pub enum LoopDecision<P> {
    /// Continue iterating with refreshed inputs.
    Continue {
        next_step: NextStep,
        new_params: P,
        reason: &'static str,
    },
    /// Hand off accumulated context to the synthesizer.
    Synthesize,
    /// Stop iterating and degrade gracefully (return partial / fallback answer).
    Degrade { reason: DegradeReason },
    /// Ask the user for clarification before doing anything else.
    Clarify { question: String },
}

/// Outcome reported by individual `ReactStep` implementations to the loop driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    /// Step completed; the loop should run the evaluator next.
    AdvanceToEvaluate,
    /// Step completed with a terminal outcome (Synthesize / Clarify / Degrade
    /// were already produced); loop should exit.
    Terminal,
}

/// Optional trait for sharing step shape between agents.
///
/// Concrete agents may opt into this trait or call free functions directly —
/// the loop driver itself does not require trait dispatch. The trait exists
/// for the case where Plan / Evaluate become reusable across RAG and Search.
#[async_trait::async_trait]
pub trait ReactStep<S>: Send + Sync {
    async fn execute(&self, state: &mut S, ctx: &ReactContext<'_>)
    -> Result<StepOutcome, AppError>;
}

/// Emit a `retrying`-style activity event when the loop continues.
///
/// Centralising this here keeps stage strings consistent across agents and
/// satisfies the contract in §4.6 (Activity events fire only at state
/// transitions, not on every internal substep).
pub async fn emit_retry_activity(ctx: &ReactContext<'_>, next_step: NextStep, reason: &str) {
    ctx.emit_activity(next_step.activity_stage(), reason.to_string())
        .await;
}

/// Construct a cancellation-shaped `AppError`. Uses HTTP 499 (client closed
/// request) so caller chains can distinguish cancellation from real failures.
pub(crate) fn cancellation_error() -> AppError {
    AppError::Internal {
        code: "request_cancelled",
        message: "request cancelled".to_string(),
        http_status: 499,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::events::{AgentEvent, CollectingSink};

    #[test]
    fn loop_budget_starts_at_zero() {
        let b = LoopBudget::rag(BillingTier::Pro);
        assert_eq!(b.current, 0);
        assert_eq!(b.max_iterations, 4);
        assert!(!b.exhausted());
        assert_eq!(b.remaining(), 4);
    }

    #[test]
    fn loop_budget_tick_advances_until_exhausted() {
        let mut b = LoopBudget::new(2);
        assert!(!b.exhausted());
        b.tick();
        assert!(!b.exhausted());
        b.tick();
        assert!(b.exhausted());
        assert_eq!(b.remaining(), 0);
        // Saturating: extra ticks must not panic.
        b.tick();
        assert!(b.exhausted());
    }

    #[test]
    fn search_budget_reads_billing_policy() {
        assert_eq!(LoopBudget::search(BillingTier::Pro).max_iterations, 3);
        assert_eq!(LoopBudget::search(BillingTier::Plus).max_iterations, 3);
        assert_eq!(LoopBudget::search(BillingTier::Free).max_iterations, 2);
    }

    #[test]
    fn enterprise_plan_id_resolves_to_plus_budget() {
        assert_eq!(
            LoopBudget::rag_for_plan("enterprise").max_iterations,
            LoopBudget::rag(BillingTier::Plus).max_iterations
        );
    }

    #[test]
    fn next_step_stage_strings_are_stable() {
        // These strings are part of the telemetry contract — guard against
        // accidental rename via this test.
        assert_eq!(NextStep::Replan.activity_stage(), "replanning");
        assert_eq!(NextStep::BroadenQuery.activity_stage(), "broadening_query");
        assert_eq!(
            NextStep::EscalateVertical.activity_stage(),
            "escalating_vertical"
        );
        assert_eq!(
            NextStep::EscalateToSearch.activity_stage(),
            "escalating_to_search"
        );
        assert_eq!(
            NextStep::FetchFullPage.activity_stage(),
            "fetching_full_page"
        );
    }

    #[test]
    fn degrade_reason_stage_strings_are_stable() {
        assert_eq!(
            DegradeReason::BudgetExhausted.as_stage(),
            "budget_exhausted"
        );
        assert_eq!(
            DegradeReason::NoResultsAfterAllFallbacks.as_stage(),
            "no_results"
        );
        assert_eq!(DegradeReason::AllToolsFailed.as_stage(), "all_tools_failed");
        assert_eq!(
            DegradeReason::ProviderUnavailable.as_stage(),
            "provider_unavailable"
        );
        assert_eq!(DegradeReason::Other("x".to_string()).as_stage(), "other");
    }

    #[test]
    fn loop_decision_continue_requires_new_params() {
        // Compile-time guarantee: constructing Continue without `new_params`
        // is not possible. This test is a usage example, not a runtime check.
        #[derive(Debug, Clone)]
        struct Params {
            query: String,
        }
        let decision: LoopDecision<Params> = LoopDecision::Continue {
            next_step: NextStep::BroadenQuery,
            new_params: Params {
                query: "broader".to_string(),
            },
            reason: "zero_recall",
        };
        match decision {
            LoopDecision::Continue { new_params, .. } => {
                assert_eq!(new_params.query, "broader");
            }
            _ => panic!("expected Continue"),
        }
    }

    #[tokio::test]
    async fn check_cancelled_returns_error_when_token_fired() {
        let cancel = CancellationToken::new();
        let sink = CollectingSink::new();
        let ctx = ReactContext::new(&sink, &cancel, "trace-1");
        assert!(ctx.check_cancelled().is_ok());
        cancel.cancel();
        let err = ctx.check_cancelled().unwrap_err();
        assert_eq!(err.code(), "request_cancelled");
        assert_eq!(err.http_status(), 499);
    }

    #[tokio::test]
    async fn emit_activity_writes_to_sink() {
        let cancel = CancellationToken::new();
        let sink = CollectingSink::new();
        let ctx = ReactContext::new(&sink, &cancel, "trace-1");
        ctx.emit_activity("planning", "building plan").await;
        let events = sink.events();
        assert_eq!(events.len(), 1);
        match &events[0] {
            AgentEvent::Activity { stage, message } => {
                assert_eq!(stage, "planning");
                assert_eq!(message, "building plan");
            }
            other => panic!("expected Activity, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn emit_retry_activity_uses_next_step_stage() {
        let cancel = CancellationToken::new();
        let sink = CollectingSink::new();
        let ctx = ReactContext::new(&sink, &cancel, "trace-1");
        emit_retry_activity(&ctx, NextStep::EscalateVertical, "no_general_results").await;
        let events = sink.events();
        match &events[0] {
            AgentEvent::Activity { stage, message } => {
                assert_eq!(stage, "escalating_vertical");
                assert_eq!(message, "no_general_results");
            }
            other => panic!("expected Activity, got {other:?}"),
        }
    }
}
