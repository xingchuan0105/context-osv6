//! StrategyExecutor — generic driver for any Strategy state machine.
//!
//! Drives the state machine until termination, emitting StateTransition events at each
//! boundary. Replaces the v4 ProgressiveLoop fixed-phase driver.

use super::{StepOutcome, Strategy, StrategyContext};
use crate::agents::events::{AgentEvent, StateTransitionType};
use crate::agents::runtime::{AgentRunResult, AgentTrace, BudgetUsage, StateRecord, TraceSpan};
use common::AppError;

/// Extract a short strategy name from the type name for metrics.
fn strategy_name<S: Strategy>() -> String {
    let full = std::any::type_name::<S>();
    full.rsplit("::").next().unwrap_or(full).to_string()
}

/// Generic executor that drives any Strategy until termination.
pub struct StrategyExecutor;

impl StrategyExecutor {
    /// Run a strategy from init to termination.
    pub async fn run<S: Strategy>(
        &self,
        strategy: &S,
        mut ctx: S::Context,
    ) -> Result<AgentRunResult, AppError> {
        let start_time = std::time::Instant::now();
        let mut state_history: Vec<StateRecord> = Vec::new();
        let trace_id = ctx.trace_id().to_string();

        // Create root span for the entire agent.run
        let mut root_span = TraceSpan::new(&trace_id, "agent.run", None);
        root_span.set_attribute("strategy", std::any::type_name::<S>());

        let mut state = strategy.init(&mut ctx).await?;

        loop {
            ctx.check_cancelled()?;

            // Budget exhaustion guard: emit audit record and degrade gracefully.
            if ctx.budget().exhausted() {
                let audit_record = crate::agents::audit::budget_exhausted_record(
                    ctx.org_id().as_deref().unwrap_or("unknown"),
                    ctx.actor_id().as_deref(),
                    ctx.trace_id(),
                    ctx.budget().current,
                    ctx.budget().max_iterations,
                    &strategy_name::<S>(),
                );
                let _ = ctx
                    .sink()
                    .emit(crate::agents::events::AgentEvent::Audit {
                        record: audit_record,
                    })
                    .await;

                return Ok(AgentRunResult {
                    answer: "Budget exhausted — unable to complete request.".to_string(),
                    degrade_trace: vec![common::DegradeTraceItem {
                        stage: "executor".to_string(),
                        reason: "budget_exhausted".to_string(),
                        impact: "terminated early without completing state machine".to_string(),
                    }],
                    final_decision: Some(crate::agents::runtime::FinalDecision::Degraded {
                        reason: crate::agents::react_loop::DegradeReason::BudgetExhausted,
                    }),
                    state_history: Some(state_history),
                    trace_id: Some(trace_id),
                    ..Default::default()
                });
            }

            let state_id = state.state_id().to_string();
            let state_kind = format!("{:?}", state.state_kind());
            let observable = state.to_observable();

            let entered_at = start_time.elapsed().as_millis() as u64;

            // Emit StateTransition::Entered
            let _ = ctx
                .sink()
                .emit(AgentEvent::StateTransition {
                    transition_type: StateTransitionType::Entered,
                    state_id: state_id.clone(),
                    state_kind: state_kind.clone(),
                    elapsed_ms: None,
                    timestamp_ms: Some(entered_at),
                    payload: Some(observable),
                })
                .await;

            // Create child span for this state
            let mut state_span = TraceSpan::new(
                &trace_id,
                &format!("state.{}", state_id),
                Some(&root_span.id),
            );
            state_span.set_attribute("state_kind", state_kind.clone());

            let step_start = std::time::Instant::now();
            let outcome = strategy.step(state, &mut ctx).await;
            let elapsed_ms = step_start.elapsed().as_millis() as u64;
            let completed_at = start_time.elapsed().as_millis() as u64;

            // Finish state span
            state_span.set_attribute("elapsed_ms", elapsed_ms);
            state_span.finish();

            // Record state history
            state_history.push(StateRecord {
                state_id: state_id.clone(),
                state_kind: state_kind.clone(),
                entered_at_ms: entered_at,
                completed_at_ms: completed_at,
                elapsed_ms,
            });

            // Emit per-state metric
            telemetry::prometheus::observe_agent_state(
                &strategy_name::<S>(),
                &state_id,
                elapsed_ms as f64,
            );

            match outcome {
                Ok(StepOutcome::Next(next_state)) => {
                    // Emit StateTransition::Completed
                    let _ = ctx
                        .sink()
                        .emit(AgentEvent::StateTransition {
                            transition_type: StateTransitionType::Completed,
                            state_id: state_id.clone(),
                            state_kind: state_kind.clone(),
                            elapsed_ms: Some(elapsed_ms),
                            timestamp_ms: Some(completed_at),
                            payload: None,
                        })
                        .await;

                    state = next_state;
                }
                Ok(StepOutcome::Terminate(mut result)) => {
                    let total_elapsed_ms = start_time.elapsed().as_millis() as u64;
                    let strategy = strategy_name::<S>();

                    // Emit StateTransition::Terminal
                    let _ = ctx
                        .sink()
                        .emit(AgentEvent::StateTransition {
                            transition_type: StateTransitionType::Terminal,
                            state_id: state_id.clone(),
                            state_kind: state_kind.clone(),
                            elapsed_ms: Some(elapsed_ms),
                            timestamp_ms: Some(completed_at),
                            payload: None,
                        })
                        .await;

                    // Finish root span
                    root_span.set_attribute("total_elapsed_ms", total_elapsed_ms);
                    root_span.set_attribute("budget_used", ctx.budget().current);
                    root_span.finish();

                    // Build AgentTrace
                    let mut trace = AgentTrace::new(&trace_id);
                    trace.add_span(root_span);
                    trace.add_span(state_span);
                    trace.total_elapsed_ms = total_elapsed_ms;
                    trace.budget_used = ctx.budget().current;

                    // Enrich result with v5 white-box fields
                    result.trace_id = Some(trace_id.clone());
                    result.state_history = Some(state_history);
                    result.budget_used = Some(BudgetUsage {
                        current: ctx.budget().current,
                        max: ctx.budget().max_iterations,
                    });
                    result.total_elapsed_ms = Some(total_elapsed_ms);
                    result.trace = Some(trace);

                    // Build and attach replay snapshot (post-hoc capture).
                    if let Some(req) = ctx.request() {
                        let captured = crate::agents::replay::CapturedRunResult::from(&result);
                        let snapshot = crate::agents::replay::SnapshotBuilder::new()
                            .trace_id(trace_id.clone())
                            .request(req.clone())
                            .environment(crate::agents::replay::current_environment())
                            .with_captured_result(captured)
                            .build();
                        result.snapshot = snapshot;
                    }

                    // Emit Terminal event
                    let final_decision_str = result
                        .final_decision
                        .as_ref()
                        .map(|d| format!("{:?}", d))
                        .unwrap_or_else(|| "synthesized".to_string());
                    let _ = ctx
                        .sink()
                        .emit(AgentEvent::Terminal {
                            decision: final_decision_str,
                            reason: result.degrade_trace.first().map(|d| d.reason.clone()),
                        })
                        .await;

                    // Emit TraceSummary event
                    let _ = ctx
                        .sink()
                        .emit(AgentEvent::TraceSummary {
                            trace_id: trace_id.clone(),
                            total_elapsed_ms,
                        })
                        .await;

                    // Emit run-level metric
                    telemetry::prometheus::observe_agent_run(
                        &strategy,
                        total_elapsed_ms as f64,
                    );

                    return Ok(result);
                }
                Err(e) => {
                    // Error path: mark spans as failed
                    state_span.set_error(&e.display_message());
                    root_span.set_error(&e.display_message());

                    let mut trace = AgentTrace::new(&trace_id);
                    trace.add_span(root_span);
                    trace.add_span(state_span);

                    let total_elapsed_ms = start_time.elapsed().as_millis() as u64;
                    let strategy = strategy_name::<S>();

                    // Degradable errors: return Ok with degrade trace instead of failing.
                    if e.is_degradable() {
                        let degrade_trace = vec![common::DegradeTraceItem {
                            stage: "executor".to_string(),
                            reason: e.display_message(),
                            impact: "degraded due to error".to_string(),
                        }];
                        let result = AgentRunResult {
                            trace_id: Some(trace_id),
                            state_history: Some(state_history),
                            trace: Some(trace),
                            total_elapsed_ms: Some(total_elapsed_ms),
                            degrade_trace,
                            final_decision: Some(crate::agents::runtime::FinalDecision::Degraded {
                                reason: crate::agents::react_loop::DegradeReason::Other(e.display_message()),
                            }),
                            ..Default::default()
                        };
                        telemetry::prometheus::observe_agent_error(&format!("{:?}", e));
                        telemetry::prometheus::observe_agent_run(
                            &strategy,
                            total_elapsed_ms as f64,
                        );
                        return Ok(result);
                    }

                    let _result = AgentRunResult {
                        trace_id: Some(trace_id),
                        state_history: Some(state_history),
                        trace: Some(trace),
                        total_elapsed_ms: Some(total_elapsed_ms),
                        ..Default::default()
                    };

                    // Emit error metric
                    telemetry::prometheus::observe_agent_error(&format!("{:?}", e));
                    // Emit run metric (failed)
                    telemetry::prometheus::observe_agent_run(
                        &strategy,
                        total_elapsed_ms as f64,
                    );

                    return Err(e.into());
                }
            }
        }
    }
}
