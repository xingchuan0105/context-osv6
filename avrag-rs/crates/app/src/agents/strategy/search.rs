//! SearchStrategy — v5 state machine for Search mode.
//!
//! Search is multi-iteration with web search, evaluation, and optional broaden:
//!   Decompose → Search → Evaluate → Answer
//!                 ↑       │
//!                 └───────┘ (broaden/escalate_vertical/replan)
//!
//! Decompose runs once at the start to generate sub-queries.
//! Search executes web_search (parallel for initial plan, single for follow-ups).

use super::{State, StateKind, StepOutcome, Strategy, StrategyContext};
use crate::agents::evaluator::{evaluate_search_iteration, EvalAdvice, EvaluationSignals};
use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::progressive::PromptRegistry;
use crate::agents::react_loop::{DegradeReason, LoopBudget};
use crate::agents::runtime::{AgentRequest, AgentRunResult, FinalDecision, IterationRecord};
use crate::agents::unified::helpers;
use avrag_llm::LlmClient;
use avrag_llm::LlmUsage;
use avrag_llm::ChatMessage as LlmChatMessage;
use avrag_search::{SearchResponse, SearchResult};
use common::{AppError, DegradeTraceItem};
use std::collections::HashSet;
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// SearchState
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum SearchState {
    /// Decompose: run planner LLM to generate sub-queries and search plan.
    Decompose,
    /// Search: execute web_search (parallel sub-queries or single query).
    Search,
    /// Evaluate: assess search quality and decide next action.
    Evaluate,
    /// Answer: synthesize final response from accumulated results.
    Answer,
}

impl State for SearchState {
    fn state_id(&self) -> &'static str {
        match self {
            SearchState::Decompose => "decompose",
            SearchState::Search => "search",
            SearchState::Evaluate => "evaluate",
            SearchState::Answer => "answer",
        }
    }

    fn state_kind(&self) -> StateKind {
        match self {
            SearchState::Decompose => StateKind::Plan,
            SearchState::Search => StateKind::Execute,
            SearchState::Evaluate => StateKind::Evaluate,
            SearchState::Answer => StateKind::Answer,
        }
    }

    fn to_observable(&self) -> serde_json::Value {
        match self {
            SearchState::Decompose => serde_json::json!({"state": "decompose"}),
            SearchState::Search => serde_json::json!({"state": "search"}),
            SearchState::Evaluate => serde_json::json!({"state": "evaluate"}),
            SearchState::Answer => serde_json::json!({"state": "answer"}),
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ---------------------------------------------------------------------------
// SearchContext
// ---------------------------------------------------------------------------

pub struct SearchContext {
    pub request: AgentRequest,
    pub trace_id: String,
    pub budget: LoopBudget,
    pub sink: Box<dyn AgentEventSink>,
    pub cancel: CancellationToken,
    pub auth: avrag_auth::AuthContext,

    // Search runtime state
    pub accumulated_search_results: Vec<SearchResult>,
    pub seen_urls: HashSet<String>,
    pub all_sub_queries: Vec<String>,

    // Iteration state
    pub current_query: String,
    pub current_vertical: Option<String>,
    pub current_search_response: Option<SearchResponse>,
    pub current_plan: Option<SearchPlan>,
    pub iterations: Vec<IterationRecord>,
    pub is_phase1: bool,

    // Accumulated
    pub aggregated_usage: Option<avrag_llm::LlmUsage>,
    pub request_count: u64,
    pub all_tool_results: Vec<common::ToolResult>,
    pub content_guard_trace: Vec<DegradeTraceItem>,
    /// Tool call records for white-box reporting.
    pub tool_call_records: Vec<crate::agents::runtime::ToolCallRecord>,
}

impl StrategyContext for SearchContext {
    fn trace_id(&self) -> &str {
        &self.trace_id
    }

    fn budget(&self) -> &LoopBudget {
        &self.budget
    }

    fn budget_mut(&mut self) -> &mut LoopBudget {
        &mut self.budget
    }

    fn sink(&self) -> &dyn AgentEventSink {
        self.sink.as_ref()
    }

    fn cancel(&self) -> &CancellationToken {
        &self.cancel
    }

    fn org_id(&self) -> Option<String> {
        Some(self.auth.org_id().to_string())
    }

    fn actor_id(&self) -> Option<String> {
        self.auth.actor_id().map(|id| id.uuid().to_string())
    }

    fn request(&self) -> Option<&crate::agents::runtime::AgentRequest> {
        Some(&self.request)
    }
}

impl SearchContext {
    /// Build a SearchContext from an AgentRequest and runtime dependencies.
    pub fn from_request(
        request: AgentRequest,
        trace_id: String,
        budget: LoopBudget,
        sink: Box<dyn AgentEventSink>,
        cancel: CancellationToken,
    ) -> Result<Self, common::AppError> {
        let auth: avrag_auth::AuthContext =
            serde_json::from_value(request.auth_context.clone()).map_err(|error| {
                common::AppError::internal(format!("Failed to deserialize auth context: {error}"))
            })?;
        Ok(Self {
            request: request.clone(),
            trace_id,
            budget,
            sink,
            cancel,
            auth,
            accumulated_search_results: Vec::new(),
            seen_urls: HashSet::new(),
            all_sub_queries: Vec::new(),
            current_query: request.query,
            current_vertical: None,
            current_search_response: None,
            current_plan: None,
            iterations: Vec::new(),
            is_phase1: true,
            aggregated_usage: None,
            request_count: 0,
            all_tool_results: Vec::new(),
            content_guard_trace: Vec::new(),
            tool_call_records: Vec::new(),
        })
    }
}

// ---------------------------------------------------------------------------
// SearchStrategy
// ---------------------------------------------------------------------------

pub struct SearchStrategy {
    pub llm: avrag_llm::LlmClient,
    pub temperature: Option<f32>,
    pub search_executor: std::sync::Arc<dyn avrag_search::SearchProvider>,
    pub search_synthesizer: Option<std::sync::Arc<dyn SearchAnswerSynthesizer>>,
}

#[async_trait::async_trait]
impl Strategy for SearchStrategy {
    type Context = SearchContext;

    async fn init(
        &self,
        _ctx: &mut SearchContext,
    ) -> Result<Box<dyn State>, AppError> {
        Ok(Box::new(SearchState::Decompose))
    }

    async fn step(
        &self,
        state: Box<dyn State>,
        ctx: &mut SearchContext,
    ) -> Result<StepOutcome, AppError> {
        let search_state = state
            .as_any()
            .downcast_ref::<SearchState>()
            .ok_or_else(|| AppError::internal("invalid state type for SearchStrategy"))?;

        match search_state {
            SearchState::Decompose => self.step_decompose(ctx).await,
            SearchState::Search => self.step_search(ctx).await,
            SearchState::Evaluate => self.step_evaluate(ctx).await,
            SearchState::Answer => self.step_answer(ctx).await,
        }
    }
}

impl SearchStrategy {
    // --- Decompose step ---

    async fn step_decompose(
        &self,
        ctx: &mut SearchContext,
    ) -> Result<StepOutcome, AppError> {
        ctx.check_cancelled()?;

        let registry = PromptRegistry::standard_cached();
        let system_prompt = registry
            .skill("search-plan")
            .map(|s| s.system_prompt().to_string())
            .unwrap_or_default();

        let plan = plan_search(
            &self.llm,
            &ctx.request.query,
            self.temperature,
            &system_prompt,
        )
        .await;

        if let Some(ref p) = plan {
            let _ = ctx
                .sink
                .emit(AgentEvent::PlanDecision {
                    selected_tools: p.atomic_calls.clone(),
                    selected_skills: p.sub_queries.clone(),
                    reasoning: p.intent_summary.clone(),
                })
                .await;
        }
        ctx.current_plan = plan;
        ctx.is_phase1 = true;

        Ok(StepOutcome::Next(Box::new(SearchState::Search)))
    }

    // --- Search step ---

    async fn step_search(
        &self,
        ctx: &mut SearchContext,
    ) -> Result<StepOutcome, AppError> {
        ctx.check_cancelled()?;

        // Phase 1: parallel execution of planner sub-queries (first call only).
        if let Some(plan) = ctx.current_plan.take() {
            let _ = ctx
                .sink
                .emit(AgentEvent::Activity {
                    stage: "planning".to_string(),
                    message: format!(
                        "Plan: {} | Sub-queries: {} | Atomic calls: {}",
                        plan.intent_summary,
                        plan.sub_queries.join(", "),
                        plan.atomic_calls.len(),
                    ),
                })
                .await;

            let mut all_calls: Vec<common::ToolCall> = plan
                .sub_queries
                .iter()
                .map(|q| common::ToolCall {
                    tool: "web_search".to_string(),
                    version: "1.0".to_string(),
                    args: serde_json::json!({
                        "query": q,
                        "vertical": plan.preferred_vertical,
                    }),
                })
                .collect();
            all_calls.extend(plan.atomic_calls);

            // Save calls for white-box reporting before they are consumed.
            let calls_for_records = all_calls.clone();

            let tool_results = crate::agents::unified::atomic_tools::dispatch_atomic_tools_with_enforcement(
                all_calls,
                Some(self.search_executor.as_ref()),
                Some(&ctx.auth),
            )
            .await;

            let mut all_new_results: Vec<SearchResult> = Vec::new();
            let mut all_sub_queries: Vec<String> = Vec::new();

            // Record tool call details for white-box reporting.
            let iteration_idx = ctx.iterations.len() as u8;
            for (call, result) in calls_for_records.iter().zip(tool_results.iter()) {
                let elapsed_ms = result.trace.as_ref().and_then(|t| t.elapsed_ms).unwrap_or(0);
                ctx.tool_call_records.push(crate::agents::runtime::ToolCallRecord {
                    tool: call.tool.clone(),
                    iteration: iteration_idx,
                    args: call.args.clone(),
                    status: result.status,
                    elapsed_ms,
                });
                let _ = ctx
                    .sink
                    .emit(AgentEvent::ToolResult {
                        tool: call.tool.clone(),
                        status: result.status,
                        data: result.data.clone(),
                        elapsed_ms,
                    })
                    .await;
            }

            for result in tool_results {
                match result.tool.as_str() {
                    "web_search" => {
                        if result.status == common::ToolStatus::Ok {
                            if let Some(data) = result.data
                                && let Ok(response) = serde_json::from_value::<SearchResponse>(data) {
                                    for sub in &response.sub_queries {
                                        if !all_sub_queries.contains(sub) {
                                            all_sub_queries.push(sub.clone());
                                        }
                                    }
                                    for r in &response.results {
                                        if ctx.seen_urls.insert(r.url.clone()) {
                                            let cloned = r.clone();
                                            all_new_results.push(cloned.clone());
                                            ctx.accumulated_search_results.push(cloned);
                                        }
                                    }
                                    if let Some(usage) = response.llm_usage.as_ref() {
                                        ctx.aggregated_usage = Some(helpers::merge_usage(
                                            ctx.aggregated_usage.as_ref(),
                                            usage,
                                        ));
                                        ctx.request_count += 1;
                                    }
                                }
                        } else if let Some(data) = result.data
                            && let Some(error) = data.get("error").and_then(|v| v.as_str()) {
                                tracing::warn!(error = %error, "web_search tool failed");
                            }
                    }
                    _ => {
                        ctx.all_tool_results.push(result);
                    }
                }
            }

            let _ = ctx
                .sink
                .emit(AgentEvent::Activity {
                    stage: "reading_sources".to_string(),
                    message: format!(
                        "Planner collected {} sources from {} sub-queries",
                        all_new_results.len(),
                        plan.sub_queries.len()
                    ),
                })
                .await;

            ctx.all_sub_queries.extend(all_sub_queries);

            // Store synthetic response for evaluate phase.
            ctx.current_search_response = Some(SearchResponse {
                query_type: "planner".to_string(),
                sub_queries: ctx.all_sub_queries.clone(),
                results: ctx.accumulated_search_results.clone(),
                synthesized_answer: String::new(),
                llm_usage: ctx.aggregated_usage.clone(),
            });

            return Ok(StepOutcome::Next(Box::new(SearchState::Evaluate)));
        }

        // ------------------------------------------------------------------
        // Phase 2: single search execution (follow-up iterations).
        // ------------------------------------------------------------------
        let iteration_idx = ctx.budget.current;
        let iter_started = std::time::Instant::now();

        let _ = ctx
            .sink
            .emit(AgentEvent::Activity {
                stage: "searching".to_string(),
                message: format!(
                    "Searching (iteration {}{})",
                    iteration_idx + 1,
                    ctx.current_vertical
                        .as_deref()
                        .map(|v| format!(", vertical={v}"))
                        .unwrap_or_default(),
                ),
            })
            .await;

        let search_call = common::ToolCall {
            tool: "web_search".to_string(),
            version: "1.0".to_string(),
            args: serde_json::json!({
                "query": ctx.current_query,
                "vertical": ctx.current_vertical,
            }),
        };
        let search_call_for_record = search_call.clone();

        let response = tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => {
                return Err(AppError::internal("request cancelled"));
            }
            results = crate::agents::unified::atomic_tools::dispatch_atomic_tools_with_enforcement(
                vec![search_call],
                Some(self.search_executor.as_ref()),
                Some(&ctx.auth),
            ) => {
                let iteration_idx = ctx.iterations.len() as u8;
                let mut results_iter = results.into_iter();
                let first_result = results_iter.next();
                if let Some(ref result) = first_result {
                    let elapsed_ms = result.trace.as_ref().and_then(|t| t.elapsed_ms).unwrap_or(0);
                    ctx.tool_call_records.push(crate::agents::runtime::ToolCallRecord {
                        tool: search_call_for_record.tool.clone(),
                        iteration: iteration_idx,
                        args: search_call_for_record.args.clone(),
                        status: result.status,
                        elapsed_ms,
                    });
                    let _ = ctx
                        .sink
                        .emit(AgentEvent::ToolResult {
                            tool: search_call_for_record.tool.clone(),
                            status: result.status,
                            data: result.data.clone(),
                            elapsed_ms,
                        })
                        .await;
                }
                match first_result {
                    Some(result) => {
                        if result.status == common::ToolStatus::Ok {
                            if let Some(data) = result.data {
                                match serde_json::from_value::<SearchResponse>(data) {
                                    Ok(response) => response,
                                    Err(error) => {
                                        tracing::warn!(error = %error, "failed to deserialize web_search response");
                                        ctx.budget.tick();
                                        let elapsed_ms = iter_started.elapsed().as_millis() as u64;
                                        ctx.iterations.push(IterationRecord {
                                            iteration: iteration_idx,
                                            plan: serde_json::json!({
                                                "query": ctx.current_query,
                                                "vertical": ctx.current_vertical,
                                                "error": error.to_string(),
                                            }),
                                            signals: EvaluationSignals::default(),
                                            decision: "degrade".to_string(),
                                            elapsed_ms,
                                            llm_evaluation: None,
                                            usage: None,
                                        });
                                        return self.finalize_degrade(ctx, DegradeReason::AllToolsFailed)
                                            .await
                                            .map(StepOutcome::Terminate);
                                    }
                                }
                            } else {
                                tracing::warn!("web_search returned Ok but no data");
                                ctx.budget.tick();
                                return self.finalize_degrade(ctx, DegradeReason::AllToolsFailed)
                                    .await
                                    .map(StepOutcome::Terminate);
                            }
                        } else {
                            let error_msg = result.data.as_ref()
                                .and_then(|d| d.get("error").and_then(|v| v.as_str()))
                                .unwrap_or("web_search failed");
                            tracing::warn!(error = %error_msg, "web_search tool failed");
                            ctx.budget.tick();
                            let elapsed_ms = iter_started.elapsed().as_millis() as u64;
                            ctx.iterations.push(IterationRecord {
                                iteration: iteration_idx,
                                plan: serde_json::json!({
                                    "query": ctx.current_query,
                                    "vertical": ctx.current_vertical,
                                    "error": error_msg,
                                }),
                                signals: EvaluationSignals::default(),
                                decision: "degrade".to_string(),
                                elapsed_ms,
                                llm_evaluation: None,
                                usage: None,
                            });
                            return self.finalize_degrade(ctx, DegradeReason::AllToolsFailed)
                                .await
                                .map(StepOutcome::Terminate);
                        }
                    }
                    None => {
                        tracing::warn!("web_search returned empty results");
                        ctx.budget.tick();
                        return self.finalize_degrade(ctx, DegradeReason::AllToolsFailed)
                            .await
                            .map(StepOutcome::Terminate);
                    }
                }
            }
        };

        for sub in &response.sub_queries {
            if !ctx.all_sub_queries.contains(sub) {
                ctx.all_sub_queries.push(sub.clone());
            }
        }

        if let Some(provider_usage) = response.llm_usage.as_ref() {
            ctx.aggregated_usage =
                Some(helpers::merge_usage(ctx.aggregated_usage.as_ref(), provider_usage));
            ctx.request_count += 1;
        }

        for result in &response.results {
            if ctx.seen_urls.insert(result.url.clone()) {
                let cloned = result.clone();
                ctx.accumulated_search_results.push(cloned);
            }
        }

        let _ = ctx
            .sink
            .emit(AgentEvent::Activity {
                stage: "reading_sources".to_string(),
                message: format!(
                    "Collected {} new sources",
                    response.results.len()
                ),
            })
            .await;

        ctx.current_search_response = Some(response);
        ctx.budget.tick();
        let _ = ctx
            .sink
            .emit(AgentEvent::BudgetTick {
                current: ctx.budget.current,
                max: ctx.budget.max_iterations,
            })
            .await;
        ctx.is_phase1 = false;

        Ok(StepOutcome::Next(Box::new(SearchState::Evaluate)))
    }

    // --- Evaluate step ---

    async fn step_evaluate(
        &self,
        ctx: &mut SearchContext,
    ) -> Result<StepOutcome, AppError> {
        ctx.check_cancelled()?;

        let original_query = ctx.request.query.clone();
        let iteration_idx = ctx.budget.current;

        let response = ctx.current_search_response.clone().unwrap_or_else(|| SearchResponse {
            query_type: "brave".to_string(),
            sub_queries: vec![ctx.current_query.clone()],
            results: Vec::new(),
            synthesized_answer: String::new(),
            llm_usage: None,
        });

        let snippet_texts: Vec<&str> = response.results.iter().map(|r| r.snippet.as_str()).collect();
        let signals = EvaluationSignals {
            recall_count: response.results.len(),
            max_score: 0.0,
            term_coverage: EvaluationSignals::compute_term_coverage(
                &original_query,
                &snippet_texts,
            ),
            zero_hits_per_subquery: Vec::new(),
        };

        // Hard constraint: budget exhausted (only for Phase 2+).
        if !ctx.is_phase1 && ctx.budget.exhausted() {
            telemetry::prometheus::observe_agent_budget_exhausted("SearchStrategy");
            let decision = if ctx.accumulated_search_results.is_empty() {
                "degrade".to_string()
            } else {
                "synthesize".to_string()
            };
            ctx.iterations.push(IterationRecord {
                iteration: iteration_idx,
                plan: serde_json::json!({
                    "query": ctx.current_query,
                    "vertical": ctx.current_vertical,
                    "sub_queries": response.sub_queries,
                    "query_type": response.query_type,
                    "result_count": response.results.len(),
                }),
                signals: signals.clone(),
                decision: decision.clone(),
                elapsed_ms: 0,
                llm_evaluation: None,
                usage: helpers::build_run_usage(ctx.aggregated_usage.as_ref(), ctx.request_count),
            });
            let eval_signals = serde_json::to_value(&signals).ok();
            let _ = ctx
                .sink
                .emit(AgentEvent::Evaluation {
                    signals: eval_signals,
                    decision: decision.clone(),
                    reasoning: "budget exhausted — forced decision".to_string(),
                })
                .await;
            if ctx.accumulated_search_results.is_empty() {
                return self.finalize_degrade(ctx, DegradeReason::NoResultsAfterAllFallbacks)
                    .await
                    .map(StepOutcome::Terminate);
            }
            return Ok(StepOutcome::Next(Box::new(SearchState::Answer)));
        }

        // LLM strategy evaluation.
        let eval_system = build_eval_system_prompt();
        let strategy_eval = self
            .evaluate_search_strategy(
                ctx,
                &original_query,
                &response,
                if ctx.is_phase1 { 0 } else { iteration_idx },
                &eval_system,
            )
            .await;
        let llm_suggested = strategy_eval
            .as_ref()
            .map(|(e, _)| e.suggested_followup_queries.clone())
            .unwrap_or_default();

        if let Some((_, eval_usage)) = &strategy_eval {
            ctx.aggregated_usage = Some(helpers::merge_usage(
                ctx.aggregated_usage.as_ref(),
                eval_usage,
            ));
            ctx.request_count += 1;
        }

        let (advice, llm_eval_json) = match &strategy_eval {
            Some((eval, _)) => {
                let mapped = map_search_strategy_to_advice(
                    eval,
                    ctx.current_vertical.as_deref(),
                );
                let json = serde_json::to_value(eval).ok();
                (mapped, json)
            }
            None => {
                let code_advice = evaluate_search_iteration(
                    &signals,
                    &ctx.budget,
                    &response.results,
                );
                (code_advice, None)
            }
        };

        let decision_str = decision_label(&advice).to_string();

        let mut iter_usage = response.llm_usage.clone();
        if let Some((_, eval_u)) = &strategy_eval {
            iter_usage = Some(helpers::merge_usage(iter_usage.as_ref(), eval_u));
        }
        let iter_agent_usage = helpers::build_run_usage(iter_usage.as_ref(), 0);

        ctx.iterations.push(IterationRecord {
            iteration: if ctx.is_phase1 { 0 } else { iteration_idx },
            plan: serde_json::json!({
                "query": ctx.current_query,
                "vertical": ctx.current_vertical,
                "sub_queries": response.sub_queries,
                "query_type": response.query_type,
                "result_count": response.results.len(),
            }),
            signals: signals.clone(),
            decision: decision_str.clone(),
            elapsed_ms: 0,
            llm_evaluation: llm_eval_json.clone(),
            usage: iter_agent_usage,
        });

        let eval_signals = serde_json::to_value(&signals).ok();
        let _ = ctx
            .sink
            .emit(AgentEvent::Evaluation {
                signals: eval_signals,
                decision: decision_str,
                reasoning: llm_eval_json
                    .and_then(|v| v.get("reason").and_then(|r| r.as_str().map(|s| s.to_string())))
                    .unwrap_or_default(),
            })
            .await;

        match advice {
            EvalAdvice::Synthesize => Ok(StepOutcome::Next(Box::new(SearchState::Answer))),
            EvalAdvice::Clarify { .. } => {
                Ok(StepOutcome::Next(Box::new(SearchState::Answer)))
            }
            EvalAdvice::Degrade { reason } => {
                self.finalize_degrade(ctx, reason)
                    .await
                    .map(StepOutcome::Terminate)
            }
            EvalAdvice::EscalateVertical { reason: _ } => {
                let Some(next_vertical) = next_vertical_step(ctx.current_vertical.as_deref()) else {
                    return self.finalize_degrade(ctx, DegradeReason::NoResultsAfterAllFallbacks)
                        .await
                        .map(StepOutcome::Terminate);
                };
                ctx.current_vertical = Some(next_vertical);
                ctx.current_query = if llm_suggested.is_empty() {
                    original_query.clone()
                } else {
                    llm_suggested[0].clone()
                };
                Ok(StepOutcome::Next(Box::new(SearchState::Search)))
            }
            EvalAdvice::BroadenQuery { reason } => {
                ctx.current_query = if llm_suggested.is_empty() {
                    helpers::broaden_query(&ctx.current_query)
                } else {
                    llm_suggested[0].clone()
                };
                let _ = ctx
                    .sink
                    .emit(AgentEvent::Activity {
                        stage: "search".to_string(),
                        message: format!("Broaden: {reason}"),
                    })
                    .await;
                Ok(StepOutcome::Next(Box::new(SearchState::Search)))
            }
            EvalAdvice::Replan { reason } => {
                ctx.current_query = if llm_suggested.is_empty() {
                    helpers::broaden_query(&original_query)
                } else {
                    llm_suggested[0].clone()
                };
                ctx.current_vertical = None;
                let _ = ctx
                    .sink
                    .emit(AgentEvent::Activity {
                        stage: "search".to_string(),
                        message: format!("Replan: {reason}"),
                    })
                    .await;
                Ok(StepOutcome::Next(Box::new(SearchState::Search)))
            }
            EvalAdvice::FetchFullPage { .. } | EvalAdvice::EscalateToSearch { .. } => {
                Ok(StepOutcome::Next(Box::new(SearchState::Answer)))
            }
        }
    }

    // --- Answer step ---

    async fn step_answer(
        &self,
        ctx: &mut SearchContext,
    ) -> Result<StepOutcome, AppError> {
        ctx.check_cancelled()?;

        let system_prompt = build_answer_system_prompt();

        if ctx.accumulated_search_results.is_empty() {
            return self.finalize_degrade(ctx, DegradeReason::NoResultsAfterAllFallbacks)
                .await
                .map(StepOutcome::Terminate);
        }

        self.finalize_synthesize(ctx, &system_prompt)
            .await
            .map(StepOutcome::Terminate)
    }

    // --- Helpers ---

    async fn evaluate_search_strategy(
        &self,
        ctx: &SearchContext,
        original_query: &str,
        response: &SearchResponse,
        iteration_idx: u8,
        system_prompt: &str,
    ) -> Option<(crate::rag_prompts::SearchStrategyEvaluation, avrag_llm::LlmUsage)> {
        let prompt = crate::rag_prompts::build_search_strategy_evaluation_prompt(
            original_query,
            ctx.current_vertical.as_deref(),
            &response.sub_queries,
            response.results.len(),
            ctx.accumulated_search_results.len(),
            iteration_idx,
        );
        let messages = vec![
            LlmChatMessage::system(system_prompt),
            LlmChatMessage::user(prompt),
        ];
        let llm_response = self.llm.complete(&messages, self.temperature).await.ok()?;
        let eval = crate::rag_prompts::parse_search_strategy_evaluation(&llm_response.content)?;
        Some((eval, llm_response.usage))
    }

    async fn finalize_synthesize(
        &self,
        ctx: &mut SearchContext,
        system_prompt: &str,
    ) -> Result<AgentRunResult, AppError> {
        let sink = ctx.sink.as_ref();

        let _ = sink.emit(AgentEvent::Activity {
            stage: "synthesizing".to_string(),
            message: format!(
                "Synthesizing answer from {} sources",
                ctx.accumulated_search_results.len()
            ),
        })
        .await;

        let renumbered = renumber_citation_indexes(&ctx.accumulated_search_results);
        let last_query_type = ctx.current_search_response
            .as_ref()
            .map(|r| r.query_type.clone())
            .unwrap_or_else(|| "brave_llm_context".to_string());
        let provider_synth_answer = ctx.current_search_response
            .as_ref()
            .map(|r| r.synthesized_answer.clone())
            .unwrap_or_default();

        // Sanitize web search snippets against prompt injection.
        let (sanitized_results, sanitize_trace) =
            if let Some(ref guard) = ctx.request.guard_pipeline {
                crate::agents::content_guard::sanitize_search_results(
                    &renumbered,
                    guard.as_ref(),
                    Some("web-search".to_string()),
                )
            } else {
                (renumbered.clone(), Vec::new())
            };

        let synth_response = SearchResponse {
            query_type: last_query_type.clone(),
            sub_queries: ctx.all_sub_queries.clone(),
            results: sanitized_results,
            synthesized_answer: provider_synth_answer.clone(),
            llm_usage: None,
        };

        let mut degrade_trace = sanitize_trace;

        // v5: Apply UntrustedInputProcessor to all tool results before
        // they enter the Answer-phase LLM prompt.
        let mut rejected_reasons = Vec::new();
        for result in &mut ctx.all_tool_results {
            if result.status == common::ToolStatus::Ok {
                let reasons = crate::agents::untrusted_input::UntrustedInputProcessor::sanitize_tool_result_data(result, 0.8);
                rejected_reasons.extend(reasons);
            }
        }
        if !rejected_reasons.is_empty() {
            degrade_trace.extend(rejected_reasons.iter().map(|reason| common::DegradeTraceItem {
                stage: "untrusted_input".to_string(),
                reason: reason.clone(),
                impact: format!("{} item(s) rejected before Answer phase", rejected_reasons.len()),
            }));
            let _ = sink.emit(crate::agents::events::AgentEvent::DebugTrace {
                kind: "untrusted_input.rejected".to_string(),
                payload: serde_json::json!({
                    "tool": "web_search",
                    "rejected_count": rejected_reasons.len(),
                    "reasons": rejected_reasons,
                }),
            }).await;
        }
        let stream = ctx.request.stream;
        let cancel = ctx.cancel.clone();

        let (answer, synth_usage): (String, Option<avrag_llm::LlmUsage>) =
            match self.synthesize_brave_answer(
                SynthesizeBraveParams {
                    query: &ctx.request.query,
                    search_response: &synth_response,
                    stream,
                    session_summary: ctx.request.session_summary.as_deref(),
                    user_preferences: ctx.request.user_preferences.as_ref(),
                    system_prompt,
                    tool_results: if ctx.all_tool_results.is_empty() {
                        None
                    } else {
                        Some(&ctx.all_tool_results)
                    },
                },
                cancel,
                sink,
            )
            .await
            {
                Ok((answer, usage)) => (answer, usage),
                Err(error) => {
                    degrade_trace.push(DegradeTraceItem {
                        stage: "search.synthesize_answer".to_string(),
                        reason: error.to_string(),
                        impact: "Returning provider evidence without final answer synthesis".to_string(),
                    });
                    if stream && !provider_synth_answer.is_empty() {
                        let _ = sink.emit(AgentEvent::MessageDelta {
                            text: provider_synth_answer.clone(),
                        })
                        .await;
                    }
                    (provider_synth_answer.clone(), None)
                }
            };

        if let Some(synth) = synth_usage.as_ref() {
            ctx.aggregated_usage = Some(helpers::merge_usage(ctx.aggregated_usage.as_ref(), synth));
            ctx.request_count += 1;
        }

        let citations = build_citations(&renumbered);
        if !citations.is_empty() {
            let _ = sink.emit(AgentEvent::Citations {
                citations: citations.clone(),
            })
            .await;
        }

        let run_usage = helpers::build_run_usage(ctx.aggregated_usage.as_ref(), ctx.request_count);
        helpers::emit_usage(sink, run_usage.as_ref()).await;

        let debug_payload = build_debug_payload(ctx, &last_query_type);
        emit_search_debug_trace_if_requested(ctx.request.debug, sink, debug_payload.clone()).await;

        let _ = sink.emit(AgentEvent::Done {
            final_message: Some(answer.clone()),
            usage: run_usage.as_ref().map(helpers::run_usage_to_agent_usage),
        })
        .await;

        let sources = build_sources(&renumbered);

        let mut result = AgentRunResult {
            answer,
            citations,
            sources,
            degrade_trace,
            usage: run_usage,
            tool_results: std::mem::take(&mut ctx.all_tool_results),
            debug_payload: Some(debug_payload),
            iterations: std::mem::take(&mut ctx.iterations),
            total_tool_calls: 0,
            final_decision: Some(FinalDecision::Synthesized),
            ..Default::default()
        };
        result.decisions = result
            .iterations
            .iter()
            .map(|it| crate::agents::runtime::DecisionRecord {
                phase: "evaluate".to_string(),
                iteration: it.iteration,
                decision: it.decision.clone(),
                reasoning: format!(
                    "recall={}; max_score={:.2}; coverage={:.2}; zero_hits_subqueries={}",
                    it.signals.recall_count,
                    it.signals.max_score,
                    it.signals.term_coverage,
                    it.signals.zero_hits_per_subquery.len()
                ),
                selected_tools: vec![],
            })
            .collect();
        result.tool_calls = std::mem::take(&mut ctx.tool_call_records);
        Ok(result)
    }

    async fn finalize_degrade(
        &self,
        ctx: &mut SearchContext,
        reason: DegradeReason,
    ) -> Result<AgentRunResult, AppError> {
        let sink = ctx.sink.as_ref();
        let fallback = ctx.current_search_response
            .as_ref()
            .map(|r| r.synthesized_answer.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                crate::chat::i18n::fallback::no_valid_retrieval_results(
                    ctx.request.language.as_deref(),
                )
                .to_string()
            });

        let _ = sink.emit(AgentEvent::MessageDelta {
            text: fallback.clone(),
        })
        .await;

        let run_usage = helpers::build_run_usage(ctx.aggregated_usage.as_ref(), ctx.request_count);
        helpers::emit_usage(sink, run_usage.as_ref()).await;

        let last_query_type = ctx.current_search_response
            .as_ref()
            .map(|r| r.query_type.clone())
            .unwrap_or_else(|| "brave_llm_context".to_string());
        let debug_payload = build_debug_payload(ctx, &last_query_type);
        emit_search_debug_trace_if_requested(ctx.request.debug, sink, debug_payload.clone()).await;

        let _ = sink.emit(AgentEvent::Done {
            final_message: Some(fallback.clone()),
            usage: run_usage.as_ref().map(helpers::run_usage_to_agent_usage),
        })
        .await;

        let degrade_trace = vec![DegradeTraceItem {
            stage: reason.as_stage().to_string(),
            reason: reason.message(),
            impact: "returned partial / fallback message — no full synthesis".to_string(),
        }];

        let mut result = AgentRunResult {
            answer: fallback,
            degrade_trace,
            usage: run_usage,
            tool_results: std::mem::take(&mut ctx.all_tool_results),
            debug_payload: Some(debug_payload),
            iterations: std::mem::take(&mut ctx.iterations),
            total_tool_calls: 0,
            final_decision: Some(FinalDecision::Degraded { reason }),
            ..Default::default()
        };
        result.decisions = result
            .iterations
            .iter()
            .map(|it| crate::agents::runtime::DecisionRecord {
                phase: "evaluate".to_string(),
                iteration: it.iteration,
                decision: it.decision.clone(),
                reasoning: format!(
                    "recall={}; max_score={:.2}; coverage={:.2}; zero_hits_subqueries={}",
                    it.signals.recall_count,
                    it.signals.max_score,
                    it.signals.term_coverage,
                    it.signals.zero_hits_per_subquery.len()
                ),
                selected_tools: vec![],
            })
            .collect();
        result.tool_calls = std::mem::take(&mut ctx.tool_call_records);
        Ok(result)
    }

    async fn synthesize_brave_answer(
        &self,
        params: SynthesizeBraveParams<'_>,
        token: CancellationToken,
        sink: &dyn AgentEventSink,
    ) -> anyhow::Result<(String, Option<avrag_llm::LlmUsage>)> {
        let Some(synthesizer) = self.search_synthesizer.as_ref() else {
            anyhow::bail!("search answer synthesizer is not configured");
        };
        if params.search_response.results.is_empty() {
            anyhow::bail!("Brave LLM Context returned no sources");
        }

        let messages = build_search_answer_messages(
            params.query,
            &params.search_response.results,
            params.session_summary,
            params.user_preferences,
            params.system_prompt,
            params.tool_results,
        );

        if params.stream {
            let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
            let mut on_delta = move |delta: String| {
                let _ = delta_tx.send(delta);
            };
            let answer_stream =
                synthesizer.synthesize_stream(&messages, self.temperature, token.clone(), &mut on_delta);
            tokio::pin!(answer_stream);

            let answer = loop {
                tokio::select! {
                    biased;
                    _ = token.cancelled() => {
                        return Err(anyhow::anyhow!("request cancelled"));
                    }
                    delta = delta_rx.recv() => {
                        if let Some(delta) = delta {
                            let _ = sink.emit(AgentEvent::MessageDelta { text: delta }).await;
                        }
                    }
                    result = &mut answer_stream => {
                        break result?;
                    }
                }
            };

            while let Ok(delta) = delta_rx.try_recv() {
                let _ = sink.emit(AgentEvent::MessageDelta { text: delta }).await;
            }

            Ok((answer.answer, answer.usage))
        } else {
            let answer = synthesizer.synthesize(&messages, self.temperature).await?;
            let _ = sink.emit(AgentEvent::MessageDelta {
                text: answer.answer.clone(),
            })
            .await;
            Ok((answer.answer, answer.usage))
        }
    }

}

// ---------------------------------------------------------------------------
// System prompt builders
// ---------------------------------------------------------------------------

fn build_eval_system_prompt() -> String {
    let registry = PromptRegistry::standard_cached();
    registry
        .skill("search-eval")
        .map(|s| s.system_prompt().to_string())
        .unwrap_or_default()
}

fn build_answer_system_prompt() -> String {
    crate::agents::strategy::prompts::build_answer_system_prompt(
        crate::agents::strategy::prompts::search::ANSWER_SKILL_ID,
    )
}

// ---------------------------------------------------------------------------
// Helpers (migrated from mode_search.rs)
// ---------------------------------------------------------------------------

async fn plan_search(
    llm: &avrag_llm::LlmClient,
    query: &str,
    temperature: Option<f32>,
    system_prompt: &str,
) -> Option<SearchPlan> {
    let messages = vec![
        LlmChatMessage::system(system_prompt),
        LlmChatMessage::user(format!(
            "User query: \"{}\"\n\nGenerate a search plan.",
            query
        )),
    ];
    let response = llm.complete(&messages, temperature).await.ok()?;
    parse_search_plan(&response.content)
}

fn parse_search_plan(raw: &str) -> Option<SearchPlan> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    let json_str = if start <= end {
        &raw[start..=end]
    } else {
        raw.trim()
    };
    let value: serde_json::Value = serde_json::from_str(json_str).ok()?;

    let sub_queries: Vec<String> = value
        .get("sub_queries")?
        .as_array()?
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
        .collect();

    if sub_queries.is_empty() {
        return None;
    }

    let intent_summary = value
        .get("intent_summary")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let needs_clarification = value
        .get("needs_clarification")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let preferred_vertical = value
        .get("preferred_vertical")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let atomic_calls: Vec<common::ToolCall> = value
        .get("calls")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let tool = item.get("tool")?.as_str()?;
                    let args = item.get("args").cloned().unwrap_or(serde_json::json!({}));
                    Some(common::ToolCall {
                        tool: tool.to_string(),
                        version: "1.0".to_string(),
                        args,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Some(SearchPlan {
        sub_queries,
        intent_summary,
        needs_clarification,
        preferred_vertical,
        atomic_calls,
    })
}

fn map_search_strategy_to_advice(
    eval: &crate::rag_prompts::SearchStrategyEvaluation,
    current_vertical: Option<&str>,
) -> EvalAdvice {
    use crate::rag_prompts::SearchStrategyRecommendation;
    match eval.recommendation {
        SearchStrategyRecommendation::Synthesize => EvalAdvice::Synthesize,
        SearchStrategyRecommendation::Broaden => EvalAdvice::BroadenQuery {
            reason: "llm_strategy_broaden",
        },
        SearchStrategyRecommendation::EscalateVertical => {
            if next_vertical_step(current_vertical).is_some() {
                EvalAdvice::EscalateVertical {
                    reason: "llm_strategy_escalate_vertical",
                }
            } else {
                EvalAdvice::Degrade {
                    reason: DegradeReason::NoResultsAfterAllFallbacks,
                }
            }
        }
    }
}

fn next_vertical_step(current: Option<&str>) -> Option<String> {
    match current {
        None => Some("news".to_string()),
        Some(_) => None,
    }
}

fn decision_label(advice: &EvalAdvice) -> &'static str {
    match advice {
        EvalAdvice::Synthesize => "synthesize",
        EvalAdvice::Clarify { .. } => "clarify",
        EvalAdvice::Degrade { .. } => "degrade",
        EvalAdvice::Replan { .. } => "replan",
        EvalAdvice::BroadenQuery { .. } => "broaden_query",
        EvalAdvice::EscalateVertical { .. } => "escalate_vertical",
        EvalAdvice::EscalateToSearch { .. } => "escalate_to_search",
        EvalAdvice::FetchFullPage { .. } => "fetch_full_page",
    }
}

fn renumber_citation_indexes(results: &[SearchResult]) -> Vec<SearchResult> {
    results
        .iter()
        .enumerate()
        .map(|(idx, result)| {
            let mut renumbered = result.clone();
            renumbered.citation_index = Some(idx + 1);
            renumbered
        })
        .collect()
}

fn build_citations(results: &[SearchResult]) -> Vec<common::Citation> {
    results
        .iter()
        .enumerate()
        .map(|(index, result)| common::Citation {
            citation_id: result.citation_index.unwrap_or(index + 1) as i64,
            doc_id: result.url.clone(),
            chunk_id: Some(format!(
                "search:{}",
                result.citation_index.unwrap_or(index + 1)
            )),
            page: None,
            doc_name: result.title.clone(),
            preview: Some(result.snippet.clone()),
            content: None,
            score: 1.0,
            layer: Some("search".to_string()),
            chunk_type: None,
            asset_id: None,
            caption: None,
            image_url: None,
            parser_backend: None,
            source_locator: Some(serde_json::json!({
                "url": result.url.clone(),
                "citation_index": result.citation_index.unwrap_or(index + 1),
            })),
            parse_run_id: None,
        })
        .collect()
}

fn build_sources(results: &[SearchResult]) -> Vec<common::SourceRef> {
    results
        .iter()
        .map(|result| common::SourceRef {
            id: result.url.clone(),
            title: result.title.clone(),
            snippet: Some(result.snippet.clone()),
            doc_id: None,
            page: None,
        })
        .collect()
}

fn build_debug_payload(ctx: &SearchContext, query_type: &str) -> serde_json::Value {
    serde_json::json!({
        "query_type": query_type,
        "sub_queries": ctx.all_sub_queries,
        "result_count": ctx.accumulated_search_results.len(),
        "iterations": ctx.iterations.len(),
    })
}

async fn emit_search_debug_trace_if_requested(
    request_debug: bool,
    sink: &dyn AgentEventSink,
    payload: serde_json::Value,
) {
    if !request_debug {
        return;
    }
    let _ = sink
        .emit(AgentEvent::DebugTrace {
            kind: "search.execution".to_string(),
            payload,
        })
        .await;
}

struct SynthesizeBraveParams<'a> {
    query: &'a str,
    search_response: &'a SearchResponse,
    stream: bool,
    session_summary: Option<&'a str>,
    user_preferences: Option<&'a serde_json::Value>,
    system_prompt: &'a str,
    tool_results: Option<&'a [common::ToolResult]>,
}

fn build_search_answer_messages(
    query: &str,
    results: &[SearchResult],
    session_summary: Option<&str>,
    user_preferences: Option<&serde_json::Value>,
    system_prompt: &str,
    tool_results: Option<&[common::ToolResult]>,
) -> Vec<LlmChatMessage> {
    let mut system = String::from(system_prompt);
    if let Some(summary) = session_summary.filter(|s| !s.trim().is_empty()) {
        system.push_str("\n\nSession summary:\n");
        system.push_str(summary.trim());
    }
    if let Some(prefs) = user_preferences {
        system.push_str("\n\nUser preferences:\n");
        system.push_str(&prefs.to_string());
    }

    let mut messages = vec![LlmChatMessage::system(system)];

    let mut context = String::new();
    context.push_str("User query: ");
    context.push_str(query);

    if let Some(tools) = tool_results
        && !tools.is_empty() {
            context.push_str("\n\nTool results:\n");
            for result in tools {
                context.push_str(&format!("\n### {}\n", result.tool));
                if result.status == common::ToolStatus::Ok {
                    if let Some(data) = &result.data {
                        context.push_str(&serde_json::to_string_pretty(data).unwrap_or_default());
                    }
                } else if let Some(data) = &result.data
                    && let Some(error) = data.get("error").and_then(|v| v.as_str()) {
                        context.push_str(&format!("Error: {error}"));
                    }
            }
        }

    context.push_str("\n\nSearch results:\n");
    for (i, result) in results.iter().enumerate() {
        context.push_str(&format!(
            "[{}] {}\nURL: {}\nSnippet: {}\n\n",
            i + 1,
            result.title,
            result.url,
            result.snippet
        ));
    }

    messages.push(LlmChatMessage::user(context));
    messages
}

// ---------------------------------------------------------------------------
// Search types (migrated from unified/state.rs)
// ---------------------------------------------------------------------------

/// Local search plan generated by the planner LLM.
#[derive(Debug, Clone)]
pub struct SearchPlan {
    pub sub_queries: Vec<String>,
    pub intent_summary: String,
    pub needs_clarification: bool,
    pub preferred_vertical: Option<String>,
    pub atomic_calls: Vec<common::ToolCall>,
}

/// Output of a search answer synthesis call.
pub struct SynthesizedSearchAnswer {
    pub answer: String,
    pub usage: Option<LlmUsage>,
}

/// Trait for search answer synthesis (stream + non-stream).
#[async_trait::async_trait]
pub trait SearchAnswerSynthesizer: Send + Sync {
    async fn synthesize(
        &self,
        messages: &[LlmChatMessage],
        temperature: Option<f32>,
    ) -> anyhow::Result<SynthesizedSearchAnswer>;

    async fn synthesize_stream(
        &self,
        messages: &[LlmChatMessage],
        temperature: Option<f32>,
        token: tokio_util::sync::CancellationToken,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> anyhow::Result<SynthesizedSearchAnswer>;
}

/// LLM-based implementation of [`SearchAnswerSynthesizer`].
pub struct LlmSearchAnswerSynthesizer {
    pub llm: LlmClient,
}

#[async_trait::async_trait]
impl SearchAnswerSynthesizer for LlmSearchAnswerSynthesizer {
    async fn synthesize(
        &self,
        messages: &[LlmChatMessage],
        temperature: Option<f32>,
    ) -> anyhow::Result<SynthesizedSearchAnswer> {
        let response = self.llm.complete(messages, temperature).await?;
        Ok(SynthesizedSearchAnswer {
            answer: response.content,
            usage: Some(response.usage),
        })
    }

    async fn synthesize_stream(
        &self,
        messages: &[LlmChatMessage],
        temperature: Option<f32>,
        token: tokio_util::sync::CancellationToken,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> anyhow::Result<SynthesizedSearchAnswer> {
        let response = self
            .llm
            .complete_stream(messages, temperature, token, |delta| on_delta(delta.to_string()))
            .await?;
        Ok(SynthesizedSearchAnswer {
            answer: response.content,
            usage: Some(response.usage),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::react_loop::DegradeReason;

    #[test]
    fn search_state_ids() {
        assert_eq!(SearchState::Decompose.state_id(), "decompose");
        assert_eq!(SearchState::Search.state_id(), "search");
        assert_eq!(SearchState::Evaluate.state_id(), "evaluate");
        assert_eq!(SearchState::Answer.state_id(), "answer");
    }

    #[test]
    fn search_state_kinds() {
        assert_eq!(SearchState::Decompose.state_kind(), StateKind::Plan);
        assert_eq!(SearchState::Search.state_kind(), StateKind::Execute);
        assert_eq!(SearchState::Evaluate.state_kind(), StateKind::Evaluate);
        assert_eq!(SearchState::Answer.state_kind(), StateKind::Answer);
    }

    #[test]
    fn parse_search_plan_valid_json() {
        let raw = r#"{"sub_queries": ["q1", "q2"], "intent_summary": "test", "needs_clarification": false, "preferred_vertical": "news"}"#;
        let plan = parse_search_plan(raw).unwrap();
        assert_eq!(plan.sub_queries, vec!["q1", "q2"]);
        assert_eq!(plan.intent_summary, "test");
        assert!(!plan.needs_clarification);
        assert_eq!(plan.preferred_vertical, Some("news".to_string()));
    }

    #[test]
    fn parse_search_plan_missing_sub_queries_returns_none() {
        let raw = r#"{"intent_summary": "test"}"#;
        assert!(parse_search_plan(raw).is_none());
    }

    #[test]
    fn parse_search_plan_empty_sub_queries_returns_none() {
        let raw = r#"{"sub_queries": [], "intent_summary": "test"}"#;
        assert!(parse_search_plan(raw).is_none());
    }

    #[test]
    fn next_vertical_step_from_none() {
        assert_eq!(next_vertical_step(None), Some("news".to_string()));
    }

    #[test]
    fn next_vertical_step_from_news_returns_none() {
        assert_eq!(next_vertical_step(Some("news")), None);
    }

    #[test]
    fn test_renumber_citation_indexes() {
        let results = vec![
            SearchResult {
                citation_index: None,
                title: "t1".to_string(),
                url: "u1".to_string(),
                snippet: "s1".to_string(),
            },
            SearchResult {
                citation_index: Some(5),
                title: "t2".to_string(),
                url: "u2".to_string(),
                snippet: "s2".to_string(),
            },
        ];
        let renumbered = renumber_citation_indexes(&results);
        assert_eq!(renumbered[0].citation_index, Some(1));
        assert_eq!(renumbered[1].citation_index, Some(2));
    }

    #[test]
    fn build_citations_maps_search_results() {
        let results = vec![SearchResult {
            citation_index: Some(1),
            title: "Title".to_string(),
            url: "https://example.com".to_string(),
            snippet: "Snippet".to_string(),
        }];
        let citations = build_citations(&results);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].citation_id, 1);
        assert_eq!(citations[0].doc_id, "https://example.com");
    }

    #[test]
    fn decision_label_coverage() {
        assert_eq!(decision_label(&EvalAdvice::Synthesize), "synthesize");
        assert_eq!(
            decision_label(&EvalAdvice::Clarify { question: "q".to_string() }),
            "clarify"
        );
        assert_eq!(
            decision_label(&EvalAdvice::Degrade { reason: DegradeReason::NoResultsAfterAllFallbacks }),
            "degrade"
        );
        assert_eq!(decision_label(&EvalAdvice::Replan { reason: "r" }), "replan");
        assert_eq!(decision_label(&EvalAdvice::BroadenQuery { reason: "r" }), "broaden_query");
        assert_eq!(decision_label(&EvalAdvice::EscalateVertical { reason: "r" }), "escalate_vertical");
    }
}
