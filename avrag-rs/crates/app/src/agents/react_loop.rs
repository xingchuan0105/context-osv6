//! ReAct loop skeleton — shared primitives for `RagAgent` and `WebSearchAgent`.
//!
//! Per `docs/CHAT_GRAPHFLOW_REMOVAL_AND_AGENT_REACT_2026-05-10.md` §4.1, this
//! module supplies the type-safe scaffolding for bounded ReAct iteration:
//!
//! - [`LoopBudget`]: enforces a tier-aware iteration ceiling. Free tier:
//!   RAG = 2, Search = 2, Chat = 2. Pro/Enterprise: RAG = 4, Search = 3,
//!   Chat = 3. See [`LoopBudget::rag`] / [`LoopBudget::search`] / [`LoopBudget::chat`].
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
use common::AppError;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

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

/// Iteration budget — see decision ④. Initial defaults: RAG = 3, Search = 2.
/// Tier-based limits: free users get stricter budgets to control costs.
///
/// `max_search_rounds` / `current_search_rounds` (added Step 3) count
/// search-API round trips separately from the LLM-side `current` /
/// `max_iterations` counter. WebSearch uses this to enforce a hard
/// 2-round stop-loss before the LLM evaluator can loop on empty
/// results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopBudget {
    pub max_iterations: u8,
    pub current: u8,
    #[deprecated = "Replaced by YAML budget.max_iterations in ADR-0006"]
    #[serde(default = "default_max_search_rounds")]
    pub max_search_rounds: u8,
    #[deprecated = "Replaced by YAML budget.max_iterations in ADR-0006"]
    #[serde(default)]
    pub current_search_rounds: u8,
}

fn default_max_search_rounds() -> u8 {
    2
}

/// User tier for cost-controlled loop budgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserTier {
    Free,
    Pro,
    Enterprise,
}

impl LoopBudget {
    pub fn new(max_iterations: u8) -> Self {
        Self {
            max_iterations,
            current: 0,
            max_search_rounds: 2,
            current_search_rounds: 0,
        }
    }

    /// Tier-based RAG budget. Free = 2 (plan + 1 fallback), Pro/Enterprise = 4.
    pub fn rag(tier: UserTier) -> Self {
        Self::new(match tier {
            UserTier::Free => 2,
            UserTier::Pro | UserTier::Enterprise => 4,
        })
    }

    /// Tier-based Search budget. Free = 2 (to support action/synthesis), Pro/Enterprise = 3.
    pub fn search(tier: UserTier) -> Self {
        Self::new(match tier {
            UserTier::Free => 2,
            UserTier::Pro | UserTier::Enterprise => 3,
        })
    }

    /// Tier-based Chat budget. Free = 2 (to support action/synthesis), Pro/Enterprise = 3.
    pub fn chat(tier: UserTier) -> Self {
        Self::new(match tier {
            UserTier::Free => 2,
            UserTier::Pro | UserTier::Enterprise => 3,
        })
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

    /// Advance the search-round counter. Saturates at `u8::MAX`.
    #[deprecated = "Replaced by YAML budget.max_iterations in ADR-0006"]
    pub fn tick_search_round(&mut self) {
        self.current_search_rounds = self.current_search_rounds.saturating_add(1);
    }

    /// True once the search-round ceiling is reached. Use to
    /// short-circuit a SearchStrategy run before the LLM evaluator
    /// can loop on empty results.
    #[deprecated = "Replaced by YAML budget.max_iterations in ADR-0006"]
    pub fn search_rounds_exhausted(&self) -> bool {
        self.current_search_rounds >= self.max_search_rounds
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

/// Reason recorded on a `Degrade` outcome — surfaced via `DegradeTraceItem`
/// in `AgentRunResult` so the UI can explain partial answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "detail")]
pub enum DegradeReason {
    /// Iteration ceiling reached without success.
    BudgetExhausted,
    /// Recall remained zero across every variant attempted.
    NoResultsAfterAllFallbacks,
    /// Tool execution failed for all subqueries.
    AllToolsFailed,
    /// Provider returned a non-retryable error (rate limit, auth, outage).
    ProviderUnavailable,
    /// Custom reason carried for telemetry; prefer a concrete variant when possible.
    Other(String),
}

impl DegradeReason {
    /// Stable stage identifier used in `DegradeTraceItem.stage` and activity events.
    pub fn as_stage(&self) -> &'static str {
        match self {
            DegradeReason::BudgetExhausted => "budget_exhausted",
            DegradeReason::NoResultsAfterAllFallbacks => "no_results",
            DegradeReason::AllToolsFailed => "all_tools_failed",
            DegradeReason::ProviderUnavailable => "provider_unavailable",
            DegradeReason::Other(_) => "other",
        }
    }

    pub fn message(&self) -> String {
        match self {
            DegradeReason::BudgetExhausted => "iteration budget exhausted".to_string(),
            DegradeReason::NoResultsAfterAllFallbacks => {
                "no results after broadening query variants".to_string()
            }
            DegradeReason::AllToolsFailed => "all tool calls failed".to_string(),
            DegradeReason::ProviderUnavailable => "provider unavailable".to_string(),
            DegradeReason::Other(msg) => msg.clone(),
        }
    }
}

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
        let b = LoopBudget::rag(UserTier::Pro);
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
    fn search_budget_default_is_two() {
        assert_eq!(LoopBudget::search(UserTier::Pro).max_iterations, 3);
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
