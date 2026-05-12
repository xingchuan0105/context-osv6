//! WebSearchAgent — bounded ReAct loop with vertical / query escalation.
//!
//! Per `docs/CHAT_GRAPHFLOW_REMOVAL_AND_AGENT_REACT_2026-05-10.md` §4.4, the
//! WebSearchAgent now drives a Search → Evaluate cycle for up to 2 iterations
//! (`LoopBudget::search(UserTier::Pro)`). Across iterations it accumulates web results
//! (deduped by URL, first-seen wins) and routes on objective signals —
//! `recall_count` and `term_coverage` — produced by
//! [`crate::agents::evaluator`].
//!
//! Continue branches:
//! - `EscalateVertical`: switch Brave general → news on zero-recall iterations.
//! - `BroadenQuery`: drop trailing modifier token from the query.
//! - `FetchFullPage`: stub (decision ⑤) — for now treated as `Synthesize`.

use crate::agents::evaluator::{
    EvalAdvice, EvaluationSignals, evaluate_search_iteration,
};
use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::react_loop::{
    UserTier,
    DegradeReason, LoopBudget, NextStep, ReactContext, cancellation_error, emit_retry_activity,
};
use crate::agents::runtime::{
    Agent, AgentRequest, AgentRunResult, AgentRunUsage, FinalDecision, IterationRecord,
};
use avrag_llm::{ChatMessage as LlmChatMessage, LlmClient, LlmUsage};
use avrag_search::{SearchProvider, SearchResponse, SearchResult};
use common::{AppError, DegradeTraceItem};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

struct SynthesizedSearchAnswer {
    answer: String,
    usage: Option<LlmUsage>,
}

#[async_trait::async_trait]
trait SearchAnswerSynthesizer: Send + Sync {
    async fn synthesize(
        &self,
        messages: &[LlmChatMessage],
        temperature: Option<f32>,
    ) -> anyhow::Result<SynthesizedSearchAnswer>;

    async fn synthesize_stream(
        &self,
        messages: &[LlmChatMessage],
        temperature: Option<f32>,
        token: CancellationToken,
        on_delta: &mut (dyn FnMut(String) + Send),
    ) -> anyhow::Result<SynthesizedSearchAnswer>;
}

struct LlmSearchAnswerSynthesizer {
    llm: LlmClient,
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
        token: CancellationToken,
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

/// WebSearchAgent handles external web search queries via a bounded ReAct
/// loop driven by objective recall / coverage signals.
pub struct WebSearchAgent {
    executor: Option<Arc<dyn SearchProvider>>,
    answer_synthesizer: Option<Arc<dyn SearchAnswerSynthesizer>>,
    llm_client: Option<LlmClient>,
    temperature: Option<f32>,
}

impl WebSearchAgent {
    pub fn new(executor: Option<Arc<dyn SearchProvider>>) -> Self {
        Self {
            executor,
            answer_synthesizer: None,
            llm_client: None,
            temperature: None,
        }
    }

    pub fn with_answer_synthesizer(
        mut self,
        agent_llm: Option<LlmClient>,
        temperature: Option<f32>,
    ) -> Self {
        self.answer_synthesizer = agent_llm.clone().map(|llm| {
            Arc::new(LlmSearchAnswerSynthesizer { llm }) as Arc<dyn SearchAnswerSynthesizer>
        });
        self.llm_client = agent_llm;
        self.temperature = temperature;
        self
    }
}

#[async_trait::async_trait]
impl Agent for WebSearchAgent {
    #[tracing::instrument(skip(self, sink), fields(agent_kind = ?request.kind))]
    async fn run(
        &self,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, AppError> {
        let Some(executor) = self.executor.clone() else {
            let _ = sink.emit(AgentEvent::Error {
                code: "search_unavailable".to_string(),
                message: "Search executor is not configured".to_string(),
            })
            .await;
            return Err(AppError::internal("Search executor is not configured"));
        };

        let cancellation = request.cancellation_token.clone().unwrap_or_default();
        let trace_id = request
            .session_id
            .clone()
            .unwrap_or_else(|| "web-search-agent".to_string());

        let mut state = WebSearchRunState {
            executor,
            synthesizer: self.answer_synthesizer.clone(),
            llm_client: self.llm_client.clone(),
            temperature: self.temperature,
            request,
            budget: LoopBudget::search(UserTier::Pro),
            accumulated_results: Vec::new(),
            seen_urls: HashSet::new(),
            all_sub_queries: Vec::new(),
            last_response: None,
            iterations: Vec::new(),
            aggregated_usage: None,
            request_count: 0,
            answer_synthesis_mode: "pending",
        };

        let ctx = ReactContext::new(sink, &cancellation, &trace_id);
        let outcome = run_react_loop(&mut state, &ctx).await?;

        match outcome {
            WebSearchLoopOutcome::Synthesize => finalize_synthesize(state, sink).await,
            WebSearchLoopOutcome::Degrade(reason) => finalize_degrade(state, sink, reason).await,
        }
    }
}

/// Mutable per-run state owned by `WebSearchAgent::run`.
struct WebSearchRunState {
    executor: Arc<dyn SearchProvider>,
    synthesizer: Option<Arc<dyn SearchAnswerSynthesizer>>,
    llm_client: Option<LlmClient>,
    temperature: Option<f32>,
    request: AgentRequest,
    budget: LoopBudget,
    accumulated_results: Vec<SearchResult>,
    seen_urls: HashSet<String>,
    all_sub_queries: Vec<String>,
    last_response: Option<SearchResponse>,
    iterations: Vec<IterationRecord>,
    aggregated_usage: Option<LlmUsage>,
    request_count: u64,
    answer_synthesis_mode: &'static str,
}

/// Iteration-scoped params. Constructing `LoopDecision::Continue` requires a
/// fresh value of this type — the type-system enforcement of decision ⑦.
#[derive(Debug, Clone)]
struct WebSearchIterationParams {
    /// Query string passed to the search provider for this iteration.
    query: String,
    /// Optional Brave vertical (`Some("news")` after escalation; `None` = general).
    vertical: Option<String>,
    /// Annotation describing why this iteration differs from the previous one.
    /// `None` for iteration 0.
    directive: Option<String>,
}


/// Local search plan generated by the planner LLM.
#[derive(Debug, Clone)]
struct SearchPlan {
    sub_queries: Vec<String>,
    intent_summary: String,
    needs_clarification: bool,
    preferred_vertical: Option<String>,
}

/// Generate a local search plan using the planner LLM.
/// Returns a plan with 1-3 sub-queries covering the key dimensions of the query.
async fn plan_search(
    llm: &LlmClient,
    query: &str,
    temperature: Option<f32>,
) -> Option<SearchPlan> {
    let system_prompt = include_str!("../../../../prompts/web_search_plan_system.txt");
    let user_prompt = format!(
        "User query: \"{}\"\n\nGenerate a search plan.",
        query
    );
    let messages = vec![
        avrag_llm::ChatMessage::system(system_prompt),
        avrag_llm::ChatMessage::user(user_prompt),
    ];
    
    let response = llm.complete(&messages, temperature).await.ok()?;
    parse_search_plan(&response.content)
}

fn parse_search_plan(raw: &str) -> Option<SearchPlan> {
    let json = extract_json_object(raw).unwrap_or_else(|| raw.trim().to_string());
    let value: serde_json::Value = serde_json::from_str(&json).ok()?;
    
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
    
    Some(SearchPlan {
        sub_queries,
        intent_summary,
        needs_clarification,
        preferred_vertical,
    })
}

fn extract_json_object(raw: &str) -> Option<String> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    (start <= end).then(|| raw[start..=end].to_string())
}

enum WebSearchLoopOutcome {
    Synthesize,
    Degrade(DegradeReason),
}

async fn run_react_loop(
    state: &mut WebSearchRunState,
    ctx: &ReactContext<'_>,
) -> Result<WebSearchLoopOutcome, AppError> {
    let original_query = state.request.query.clone();
    
    // --- Phase 1: Local Planner ---
    // Generate sub-queries with LLM, then execute them in parallel.
    let mut planned_sub_queries = vec![original_query.clone()];
    
    if let Some(llm) = state.llm_client.as_ref() {
        ctx.emit_activity("planning", "Analyzing query and generating search plan...").await;
        
        let plan_result = plan_search(llm, &original_query, state.temperature).await;
        if let Some(plan) = plan_result {
            if plan.needs_clarification {
                ctx.emit_activity(
                    "planning",
                    "Query needs clarification, falling back to direct search",
                )
                .await;
                planned_sub_queries = plan.sub_queries;
            } else {
                ctx.emit_activity(
                    "planning",
                    format!(
                        "Plan: {} | Sub-queries: {}",
                        plan.intent_summary,
                        plan.sub_queries.join(", ")
                    ),
                )
                .await;
                
                // Execute all sub-queries in parallel
                let mut futures = Vec::new();
                for sub_query in &plan.sub_queries {
                    let executor = state.executor.clone();
                    let query = sub_query.clone();
                    let vertical = plan.preferred_vertical.clone();
                    futures.push(async move {
                        executor.execute_search(&query, vertical.as_deref()).await
                    });
                }
                
                let results = futures::future::join_all(futures).await;
                let mut all_new_results: Vec<SearchResult> = Vec::new();
                let mut all_sub_queries: Vec<String> = Vec::new();
                
                for (idx, result) in results.into_iter().enumerate() {
                    match result {
                        Ok(response) => {
                            for sub in &response.sub_queries {
                                if !all_sub_queries.contains(sub) {
                                    all_sub_queries.push(sub.clone());
                                }
                            }
                            for r in &response.results {
                                if state.seen_urls.insert(r.url.clone()) {
                                    let cloned = r.clone();
                                    all_new_results.push(cloned.clone());
                                    state.accumulated_results.push(cloned);
                                }
                            }
                            if let Some(usage) = response.llm_usage.as_ref() {
                                state.aggregated_usage = Some(merge_usage(
                                    state.aggregated_usage.as_ref(),
                                    usage,
                                ));
                                state.request_count = state.request_count.saturating_add(1);
                            }
                        }
                        Err(error) => {
                            tracing::warn!(
                                error = %error,
                                sub_query = %plan.sub_queries[idx],
                                "sub-query search failed"
                            );
                        }
                    }
                }
                
                ctx.emit_activity(
                    "reading_sources",
                    format!(
                        "Planner collected {} sources from {} sub-queries",
                        all_new_results.len(),
                        plan.sub_queries.len()
                    ),
                )
                .await;
                
                // If planner found results, evaluate with both code and LLM
                if !state.accumulated_results.is_empty() {
                    // --- Code-based evaluation (fast) ---
                    let signals = EvaluationSignals {
                        recall_count: state.accumulated_results.len(),
                        max_score: 0.0,
                        term_coverage: EvaluationSignals::compute_term_coverage(
                            &original_query,
                            &state.accumulated_results.iter().map(|r| r.snippet.as_str()).collect::<Vec<_>>(),
                        ),
                        zero_hits_per_subquery: Vec::new(),
                    };
                    
                    let code_advice = evaluate_search_iteration(
                        &signals,
                        &state.budget,
                        &state.accumulated_results,
                    );
                    
                    // --- LLM-based evaluation (deep) ---
                    // Build a synthetic response for the LLM evaluator
                    let synthetic_response = SearchResponse {
                        query_type: "planner".to_string(),
                        sub_queries: all_sub_queries.clone(),
                        results: state.accumulated_results.clone(),
                        synthesized_answer: String::new(),
                        llm_usage: state.aggregated_usage.clone(),
                    };
                    let synthetic_params = WebSearchIterationParams {
                        query: original_query.clone(),
                        vertical: plan.preferred_vertical.clone(),
                        directive: Some("planner_phase".to_string()),
                    };
                    
                    let llm_eval = evaluate_search_strategy(
                        state,
                        &original_query,
                        &synthetic_params,
                        &synthetic_response,
                        0,
                    ).await;
                    
                    // Aggregate evaluator usage
                    if let Some((_, eval_usage)) = &llm_eval {
                        state.aggregated_usage = Some(merge_usage(
                            state.aggregated_usage.as_ref(),
                            eval_usage,
                        ));
                        state.request_count = state.request_count.saturating_add(1);
                    }
                    
                    // Use LLM evaluation if available, otherwise fall back to code
                    let (final_advice, llm_eval_json) = match &llm_eval {
                        Some((eval, _)) => {
                            let mapped = map_search_strategy_to_advice(
                                eval,
                                plan.preferred_vertical.as_deref(),
                            );
                            let json = serde_json::to_value(eval).ok();
                            (mapped, json)
                        }
                        None => (code_advice, None),
                    };
                    
                    state.iterations.push(IterationRecord {
                        iteration: 0,
                        plan: serde_json::json!({
                            "query": original_query,
                            "sub_queries": all_sub_queries,
                            "result_count": state.accumulated_results.len(),
                            "planner": true,
                        }),
                        signals,
                        decision: decision_label(&final_advice).to_string(),
                        elapsed_ms: 0,
                        llm_evaluation: llm_eval_json,
                        usage: build_run_usage(state.aggregated_usage.as_ref(), state.request_count),
                    });
                    
                    match final_advice {
                        EvalAdvice::Synthesize => {
                            return Ok(WebSearchLoopOutcome::Synthesize);
                        }
                        _ => {}
                    }
                }
                
                // If planner didn't find enough, fall through to ReAct loop
                planned_sub_queries = all_sub_queries;
            }
        } else {
            ctx.emit_activity("planning", "Planner failed, falling back to direct search").await;
        }
    }
    
    // --- Phase 2: ReAct Loop (fallback / refinement) ---
    let mut params = WebSearchIterationParams {
        query: planned_sub_queries.first().cloned().unwrap_or_else(|| original_query.clone()),
        vertical: None,
        directive: None,
    };

    loop {
        ctx.check_cancelled()?;

        let iteration_idx = state.budget.current;
        let iter_started = Instant::now();

        ctx.emit_activity(
            "searching",
            format!(
                "Searching (iteration {}{})",
                iteration_idx + 1,
                params
                    .vertical
                    .as_deref()
                    .map(|v| format!(", vertical={v}"))
                    .unwrap_or_default(),
            ),
        )
        .await;

        let response = tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => {
                return Err(cancellation_error());
            }
            result = state
                .executor
                .execute_search(&params.query, params.vertical.as_deref()) => {
                match result {
                    Ok(response) => response,
                    Err(error) => {
                        tracing::warn!(error = %error, "search execution failed");
                        state.budget.tick();
                        state.iterations.push(IterationRecord {
                            iteration: iteration_idx,
                            plan: serde_json::json!({
                                "query": params.query,
                                "vertical": params.vertical,
                                "directive": params.directive,
                                "error": error.to_string(),
                            }),
                            signals: EvaluationSignals::default(),
                            decision: "degrade".to_string(),
                            elapsed_ms: iter_started.elapsed().as_millis() as u64,
                            llm_evaluation: None,
                            usage: None,
                        });
                        return Ok(WebSearchLoopOutcome::Degrade(DegradeReason::AllToolsFailed));
                    }
                }
            }
        };

        for sub in &response.sub_queries {
            if !state.all_sub_queries.contains(sub) {
                state.all_sub_queries.push(sub.clone());
            }
        }

        if let Some(provider_usage) = response.llm_usage.as_ref() {
            state.aggregated_usage =
                Some(merge_usage(state.aggregated_usage.as_ref(), provider_usage));
            state.request_count = state.request_count.saturating_add(1);
        }

        let mut new_results: Vec<SearchResult> = Vec::new();
        for result in &response.results {
            if state.seen_urls.insert(result.url.clone()) {
                let cloned = result.clone();
                new_results.push(cloned.clone());
                state.accumulated_results.push(cloned);
            }
        }

        ctx.emit_activity(
            "reading_sources",
            format!("Collected {} new sources", new_results.len()),
        )
        .await;

        let snippet_texts: Vec<&str> = new_results.iter().map(|r| r.snippet.as_str()).collect();
        let signals = EvaluationSignals {
            recall_count: new_results.len(),
            max_score: 0.0,
            term_coverage: EvaluationSignals::compute_term_coverage(&original_query, &snippet_texts),
            zero_hits_per_subquery: Vec::new(),
        };

        state.budget.tick();
        let elapsed_ms = iter_started.elapsed().as_millis() as u64;

        // --- Hard constraint: budget exhausted ---
        if state.budget.exhausted() {
            let decision = if state.accumulated_results.is_empty() {
                "degrade".to_string()
            } else {
                "synthesize".to_string()
            };
            let iter_usage = build_run_usage(state.aggregated_usage.as_ref(), state.request_count);
            state.iterations.push(IterationRecord {
                iteration: iteration_idx,
                plan: serde_json::json!({
                    "query": params.query,
                    "vertical": params.vertical,
                    "directive": params.directive,
                    "sub_queries": response.sub_queries,
                    "query_type": response.query_type,
                    "result_count": response.results.len(),
                    "new_result_count": new_results.len(),
                }),
                signals: signals.clone(),
                decision,
                elapsed_ms,
                llm_evaluation: None,
                usage: iter_usage,
            });
            state.last_response = Some(response);
            if state.accumulated_results.is_empty() {
                return Ok(WebSearchLoopOutcome::Degrade(
                    DegradeReason::NoResultsAfterAllFallbacks,
                ));
            }
            return Ok(WebSearchLoopOutcome::Synthesize);
        }

        // --- Default: LLM strategy evaluation ---
        let strategy_eval = evaluate_search_strategy(
            state,
            &original_query,
            &params,
            &response,
            iteration_idx,
        )
        .await;
        let llm_suggested = strategy_eval
            .as_ref()
            .map(|(e, _)| e.suggested_followup_queries.clone())
            .unwrap_or_default();

        // Aggregate evaluator usage into run totals
        if let Some((_, eval_usage)) = &strategy_eval {
            state.aggregated_usage = Some(merge_usage(state.aggregated_usage.as_ref(), eval_usage));
            state.request_count = state.request_count.saturating_add(1);
        }

        let (advice, llm_eval_json) = match &strategy_eval {
            Some((eval, _)) => {
                let mapped = map_search_strategy_to_advice(
                    eval,
                    params.vertical.as_deref(),
                );
                let json = serde_json::to_value(eval).ok();
                (mapped, json)
            }
            None => {
                // Fallback to code evaluator if LLM strategy evaluation fails
                let code_advice = evaluate_search_iteration(&signals, &state.budget, &response.results);
                (code_advice, None)
            }
        };

        let decision_str = decision_label(&advice).to_string();

        let mut iter_usage = response.llm_usage.clone();
        if let Some((_, eval_u)) = &strategy_eval {
            iter_usage = Some(merge_usage(iter_usage.as_ref(), eval_u));
        }
        let iter_agent_usage = build_run_usage(iter_usage.as_ref(), 0);

        state.iterations.push(IterationRecord {
            iteration: iteration_idx,
            plan: serde_json::json!({
                "query": params.query,
                "vertical": params.vertical,
                "directive": params.directive,
                "sub_queries": response.sub_queries,
                "query_type": response.query_type,
                "result_count": response.results.len(),
                "new_result_count": new_results.len(),
            }),
            signals,
            decision: decision_str,
            elapsed_ms,
            llm_evaluation: llm_eval_json,
            usage: iter_agent_usage,
        });

        state.last_response = Some(response);

        match advice {
            EvalAdvice::Synthesize => return Ok(WebSearchLoopOutcome::Synthesize),
            EvalAdvice::Clarify { .. } => {
                return Ok(WebSearchLoopOutcome::Synthesize);
            }
            EvalAdvice::Degrade { reason } => {
                return Ok(WebSearchLoopOutcome::Degrade(reason));
            }
            EvalAdvice::EscalateVertical { reason } => {
                let Some(next_vertical) = next_vertical_step(params.vertical.as_deref()) else {
                    return Ok(WebSearchLoopOutcome::Degrade(
                        DegradeReason::NoResultsAfterAllFallbacks,
                    ));
                };
                emit_retry_activity(ctx, NextStep::EscalateVertical, reason).await;
                params = WebSearchIterationParams {
                    query: if llm_suggested.is_empty() {
                        original_query.clone()
                    } else {
                        llm_suggested[0].clone()
                    },
                    vertical: Some(next_vertical),
                    directive: Some(format!("escalate_vertical: {reason}")),
                };
            }
            EvalAdvice::BroadenQuery { reason } => {
                emit_retry_activity(ctx, NextStep::BroadenQuery, reason).await;
                params = WebSearchIterationParams {
                    // When LLM provides suggested queries, use the first one.
                    // Only mechanically broaden when LLM gave no guidance.
                    query: if llm_suggested.is_empty() {
                        broaden_query(&params.query)
                    } else {
                        llm_suggested[0].clone()
                    },
                    vertical: params.vertical,
                    directive: Some(format!("broaden: {reason}")),
                };
            }
            EvalAdvice::Replan { reason } => {
                emit_retry_activity(ctx, NextStep::Replan, reason).await;
                params = WebSearchIterationParams {
                    query: if llm_suggested.is_empty() {
                        broaden_query(&original_query)
                    } else {
                        llm_suggested[0].clone()
                    },
                    vertical: None,
                    directive: Some(format!("replan: {reason}")),
                };
            }
            EvalAdvice::FetchFullPage { reason } => {
                tracing::debug!(
                    %reason,
                    "search evaluator returned FetchFullPage — stub not implemented, synthesizing"
                );
                return Ok(WebSearchLoopOutcome::Synthesize);
            }
            EvalAdvice::EscalateToSearch { reason } => {
                tracing::debug!(
                    %reason,
                    "search evaluator returned EscalateToSearch — already in search, synthesizing"
                );
                return Ok(WebSearchLoopOutcome::Synthesize);
            }
        }
    }
}

async fn evaluate_search_strategy(
    state: &WebSearchRunState,
    original_query: &str,
    params: &WebSearchIterationParams,
    response: &SearchResponse,
    iteration_idx: u8,
) -> Option<(crate::rag_prompts::SearchStrategyEvaluation, LlmUsage)> {
    let llm = state.llm_client.as_ref()?;
    let prompt = crate::rag_prompts::build_search_strategy_evaluation_prompt(
        original_query,
        params.vertical.as_deref(),
        &response.sub_queries,
        response.results.len(),
        state.accumulated_results.len(),
        iteration_idx,
    );
    let messages = vec![
        avrag_llm::ChatMessage::system(crate::rag_prompts::SEARCH_STRATEGY_EVAL_SYSTEM_PROMPT),
        avrag_llm::ChatMessage::user(prompt),
    ];
    let llm_response = llm.complete(&messages, state.temperature).await.ok()?;
    let eval = crate::rag_prompts::parse_search_strategy_evaluation(&llm_response.content)?;
    Some((eval, llm_response.usage))
}

fn map_search_strategy_to_advice(
    eval: &crate::rag_prompts::SearchStrategyEvaluation,
    current_vertical: Option<&str>,
) -> EvalAdvice {
    match eval.recommendation {
        crate::rag_prompts::SearchStrategyRecommendation::Synthesize => EvalAdvice::Synthesize,
        crate::rag_prompts::SearchStrategyRecommendation::Broaden => EvalAdvice::BroadenQuery {
            reason: "llm_strategy_broaden",
        },
        crate::rag_prompts::SearchStrategyRecommendation::EscalateVertical => {
            // Only escalate if there's a next vertical to try; otherwise degrade.
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

async fn finalize_synthesize(
    mut state: WebSearchRunState,
    sink: &dyn AgentEventSink,
) -> Result<AgentRunResult, AppError> {
    if state.accumulated_results.is_empty() {
        return finalize_degrade(state, sink, DegradeReason::NoResultsAfterAllFallbacks).await;
    }

    let _ = sink.emit(AgentEvent::Activity {
        stage: "synthesizing".to_string(),
        message: format!(
            "Synthesizing answer from {} sources",
            state.accumulated_results.len()
        ),
    })
    .await;

    let renumbered = renumber_citation_indexes(&state.accumulated_results);
    let last_query_type = state
        .last_response
        .as_ref()
        .map(|r| r.query_type.clone())
        .unwrap_or_else(|| "brave_llm_context".to_string());
    let provider_synth_answer = state
        .last_response
        .as_ref()
        .map(|r| r.synthesized_answer.clone())
        .unwrap_or_default();

    let synth_response = SearchResponse {
        query_type: last_query_type.clone(),
        sub_queries: state.all_sub_queries.clone(),
        results: renumbered.clone(),
        synthesized_answer: provider_synth_answer.clone(),
        llm_usage: None,
    };

    let mut degrade_trace = Vec::new();
    let stream = state.request.stream;
    let cancellation = state.request.cancellation_token.clone().unwrap_or_default();

    let (answer, synth_usage): (String, Option<LlmUsage>) =
        match synthesize_brave_answer(
            state.synthesizer.as_deref(),
            SynthesizeBraveParams {
                temperature: state.temperature,
                query: &state.request.query,
                search_response: &synth_response,
                stream,
                session_summary: state.request.session_summary.as_deref(),
                user_preferences: state.request.user_preferences.as_ref(),
            },
            cancellation,
            sink,
        )
        .await
        {
            Ok((answer, usage)) => {
                state.answer_synthesis_mode = if stream { "llm_stream" } else { "llm_complete" };
                (answer, usage)
            }
            Err(error) => {
                state.answer_synthesis_mode = "evidence_fallback";
                degrade_trace.push(DegradeTraceItem {
                    stage: "search.synthesize_answer".to_string(),
                    reason: error.to_string(),
                    impact:
                        "Returning provider evidence without final answer synthesis"
                            .to_string(),
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
        state.aggregated_usage = Some(merge_usage(state.aggregated_usage.as_ref(), synth));
        state.request_count = state.request_count.saturating_add(1);
    }

    let citations = build_citations(&renumbered);
    if !citations.is_empty() {
        let _ = sink.emit(AgentEvent::Citations {
            citations: citations.clone(),
        })
        .await;
    }

    let run_usage = build_run_usage(state.aggregated_usage.as_ref(), state.request_count);
    emit_usage(sink, run_usage.as_ref()).await;

    let debug_payload = build_debug_payload(&state, &last_query_type);
    emit_search_debug_trace_if_requested(state.request.debug, sink, debug_payload.clone()).await;

    let _ = sink.emit(AgentEvent::Done {
        final_message: Some(answer.clone()),
        usage: run_usage.as_ref().map(run_usage_to_agent_usage),
    })
    .await;

    let sources = build_sources(&renumbered);

    Ok(AgentRunResult {
        answer,
        citations,
        sources,
        degrade_trace,
        usage: run_usage,
        debug_payload: Some(debug_payload),
        iterations: state.iterations,
        total_tool_calls: 0,
        final_decision: Some(FinalDecision::Synthesized),
        ..Default::default()
    })
}

async fn finalize_degrade(
    mut state: WebSearchRunState,
    sink: &dyn AgentEventSink,
    reason: DegradeReason,
) -> Result<AgentRunResult, AppError> {
    let fallback = state
        .last_response
        .as_ref()
        .map(|r| r.synthesized_answer.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            crate::chat::i18n::fallback::no_valid_retrieval_results(
                state.request.language.as_deref(),
            )
            .to_string()
        });

    let _ = sink.emit(AgentEvent::MessageDelta {
        text: fallback.clone(),
    })
    .await;

    state.answer_synthesis_mode = "evidence_fallback";

    let run_usage = build_run_usage(state.aggregated_usage.as_ref(), state.request_count);
    emit_usage(sink, run_usage.as_ref()).await;

    let last_query_type = state
        .last_response
        .as_ref()
        .map(|r| r.query_type.clone())
        .unwrap_or_else(|| "brave_llm_context".to_string());
    let debug_payload = build_debug_payload(&state, &last_query_type);
    emit_search_debug_trace_if_requested(state.request.debug, sink, debug_payload.clone()).await;

    let _ = sink.emit(AgentEvent::Done {
        final_message: Some(fallback.clone()),
        usage: run_usage.as_ref().map(run_usage_to_agent_usage),
    })
    .await;

    let degrade_trace = vec![DegradeTraceItem {
        stage: reason.as_stage().to_string(),
        reason: reason.message(),
        impact: "returned partial / fallback message — no full synthesis".to_string(),
    }];

    Ok(AgentRunResult {
        answer: fallback,
        degrade_trace,
        usage: run_usage,
        debug_payload: Some(debug_payload),
        iterations: state.iterations,
        total_tool_calls: 0,
        final_decision: Some(FinalDecision::Degraded { reason }),
        ..Default::default()
    })
}

fn build_debug_payload(state: &WebSearchRunState, query_type: &str) -> serde_json::Value {
    serde_json::json!({
        "query_type": query_type,
        "sub_queries": state.all_sub_queries,
        "result_count": state.accumulated_results.len(),
        "answer_synthesis_mode": state.answer_synthesis_mode,
        "iterations": state.iterations.len(),
    })
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

/// Determine the next Brave vertical to try, or `None` if no further fallback
/// vertical is supported by the executor.
fn next_vertical_step(current: Option<&str>) -> Option<String> {
    match current {
        None => Some("news".to_string()),
        // Only "news" is currently mapped to a different provider in
        // `SearchExecutor::execute_search`; any other vertical is treated as
        // "general", so further escalation has no effect.
        Some(_) => None,
    }
}

/// Drop the trailing whitespace-separated token from `query`. If the query has
/// fewer than two tokens it is returned unchanged.
fn broaden_query(query: &str) -> String {
    let words: Vec<&str> = query.split_whitespace().collect();
    if words.len() <= 1 {
        return query.to_string();
    }
    words[..words.len() - 1].join(" ")
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

fn merge_usage(existing: Option<&LlmUsage>, new: &LlmUsage) -> LlmUsage {
    match existing {
        Some(prev) => LlmUsage {
            provider: new.provider.clone(),
            model: new.model.clone(),
            prompt_tokens: prev.prompt_tokens.saturating_add(new.prompt_tokens),
            completion_tokens: prev.completion_tokens.saturating_add(new.completion_tokens),
            total_tokens: prev.total_tokens.saturating_add(new.total_tokens),
            cached_tokens: prev.cached_tokens.saturating_add(new.cached_tokens),
        },
        None => new.clone(),
    }
}

fn build_run_usage(usage: Option<&LlmUsage>, request_count: u64) -> Option<AgentRunUsage> {
    usage.map(|u| AgentRunUsage {
        provider: u.provider.clone(),
        model: u.model.clone(),
        prompt_tokens: u.prompt_tokens as u64,
        completion_tokens: u.completion_tokens as u64,
        total_tokens: u.total_tokens as u64,
        request_count,
    })
}

fn run_usage_to_agent_usage(usage: &AgentRunUsage) -> crate::agents::events::AgentUsage {
    crate::agents::events::AgentUsage {
        provider: usage.provider.clone(),
        model: usage.model.clone(),
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
    }
}

async fn emit_usage(sink: &dyn AgentEventSink, usage: Option<&AgentRunUsage>) {
    if let Some(u) = usage {
        let _ = sink.emit(AgentEvent::Usage {
            provider: u.provider.clone(),
            model: u.model.clone(),
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
            request_count: u.request_count,
            metadata: Default::default(),
        })
        .await;
    }
}

async fn emit_search_debug_trace_if_requested(
    request_debug: bool,
    sink: &dyn AgentEventSink,
    payload: serde_json::Value,
) {
    if !request_debug {
        return;
    }
    let _ = sink.emit(AgentEvent::DebugTrace {
        kind: "search.execution".to_string(),
        payload,
    })
    .await;
}

/// Legacy helper retained because `bins/api` and earlier callers may dispatch
/// `SearchStreamUpdate` events into a sink. The ReAct loop emits its own
/// activity events directly and no longer relies on streaming updates.
#[allow(dead_code)]
async fn emit_search_update(
    update: avrag_search::SearchStreamUpdate,
    sink: &dyn AgentEventSink,
    answer: &mut String,
) {
    match update {
        avrag_search::SearchStreamUpdate::Searching { queries } => {
            let detail = if queries.is_empty() {
                None
            } else {
                Some(format!("Queries: {}", queries.join(" · ")))
            };
            let _ = sink.emit(AgentEvent::Activity {
                stage: "searching".to_string(),
                message: detail.unwrap_or_else(|| "Searching".to_string()),
            })
            .await;
        }
        avrag_search::SearchStreamUpdate::SourcesCollected { results } => {
            let _ = sink.emit(AgentEvent::Activity {
                stage: "reading_sources".to_string(),
                message: format!("Collected {} sources", results.len()),
            })
            .await;
        }
        avrag_search::SearchStreamUpdate::TextDelta { delta } => {
            answer.push_str(&delta);
            let _ = sink.emit(AgentEvent::MessageDelta { text: delta }).await;
        }
    }
}

struct SynthesizeBraveParams<'a> {
    temperature: Option<f32>,
    query: &'a str,
    search_response: &'a SearchResponse,
    stream: bool,
    session_summary: Option<&'a str>,
    user_preferences: Option<&'a serde_json::Value>,
}

async fn synthesize_brave_answer(
    synthesizer: Option<&dyn SearchAnswerSynthesizer>,
    params: SynthesizeBraveParams<'_>,
    token: CancellationToken,
    sink: &dyn AgentEventSink,
) -> anyhow::Result<(String, Option<LlmUsage>)> {
    let Some(synthesizer) = synthesizer else {
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
    );
    if params.stream {
        let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let mut on_delta = move |delta: String| {
            let _ = delta_tx.send(delta);
        };
        let answer_stream =
            synthesizer.synthesize_stream(&messages, params.temperature, token, &mut on_delta);
        tokio::pin!(answer_stream);

        let response = loop {
            tokio::select! {
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
        Ok((response.answer, response.usage))
    } else {
        let response = synthesizer.synthesize(&messages, params.temperature).await?;
        Ok((response.answer, response.usage))
    }
}

fn build_search_answer_messages(
    query: &str,
    results: &[SearchResult],
    session_summary: Option<&str>,
    user_preferences: Option<&serde_json::Value>,
) -> Vec<LlmChatMessage> {
    let mut system = String::from(include_str!(
        "../../../../prompts/web_search_system.txt"
    ));
    if let Some(summary) = session_summary.filter(|s| !s.trim().is_empty()) {
        system.push_str("\n\nSession summary:\n");
        system.push_str(summary.trim());
    }
    if let Some(prefs) = user_preferences {
        system.push_str("\n\nUser preferences:\n");
        system.push_str(&prefs.to_string());
    }

    let evidence = results
        .iter()
        .enumerate()
        .map(|(index, result)| {
            format!(
                "[[{}]] title: {}\nurl: {}\nsnippet:\n{}",
                result.citation_index.unwrap_or(index + 1),
                result.title,
                result.url,
                result.snippet
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    vec![
        LlmChatMessage::system(system),
        LlmChatMessage::user(format!(
            "Question:\n{}\n\nBrave LLM Context evidence:\n{}",
            query.trim(),
            evidence
        )),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::events::CollectingSink;

    struct FakeSearchAnswerSynthesizer;

    #[async_trait::async_trait]
    impl SearchAnswerSynthesizer for FakeSearchAnswerSynthesizer {
        async fn synthesize(
            &self,
            _messages: &[LlmChatMessage],
            _temperature: Option<f32>,
        ) -> anyhow::Result<SynthesizedSearchAnswer> {
            Ok(SynthesizedSearchAnswer {
                answer: "non-stream synthesized [[1]]".to_string(),
                usage: Some(avrag_llm::LlmUsage {
                    prompt_tokens: 10,
                    completion_tokens: 4,
                    total_tokens: 14,
                    provider: "fake".to_string(),
                    model: "fake-search-llm".to_string(),
                    cached_tokens: 0,
                }),
            })
        }

        async fn synthesize_stream(
            &self,
            _messages: &[LlmChatMessage],
            _temperature: Option<f32>,
            _token: CancellationToken,
            on_delta: &mut (dyn FnMut(String) + Send),
        ) -> anyhow::Result<SynthesizedSearchAnswer> {
            on_delta("stream ".to_string());
            on_delta("synthesized [[1]]".to_string());
            Ok(SynthesizedSearchAnswer {
                answer: "stream synthesized [[1]]".to_string(),
                usage: Some(avrag_llm::LlmUsage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                    provider: "fake".to_string(),
                    model: "fake-search-llm".to_string(),
                    cached_tokens: 0,
                }),
            })
        }
    }

    #[tokio::test]
    async fn test_web_search_agent_without_executor_returns_error() {
        let agent = WebSearchAgent::new(None);
        let sink = CollectingSink::new();
        let req = AgentRequest {
            kind: crate::agents::AgentKind::Search,
            query: "hello".to_string(),
            notebook_id: None,
            session_id: None,
            doc_scope: vec![],
            messages: vec![],
            session_summary: None,
            user_preferences: None,
            language: None,
            docscope_metadata: None,
            debug: false,
            stream: false,
            auth_context: serde_json::json!({}),
            metadata: Default::default(),
            cancellation_token: None,
        };
        let result = agent.run(req, &sink).await;
        assert!(result.is_err());
        let events = sink.events();
        assert!(events.iter().any(|e| matches!(e, AgentEvent::Error { .. })));
    }

    #[tokio::test]
    async fn search_stream_updates_are_emitted_to_sink() {
        let sink = CollectingSink::new();
        let mut answer = String::new();

        emit_search_update(
            avrag_search::SearchStreamUpdate::Searching {
                queries: vec!["atlas".to_string()],
            },
            &sink,
            &mut answer,
        )
        .await;
        emit_search_update(
            avrag_search::SearchStreamUpdate::SourcesCollected {
                results: vec![avrag_search::SearchResult {
                    title: "Atlas".to_string(),
                    url: "https://example.com".to_string(),
                    snippet: "snippet".to_string(),
                    citation_index: Some(1),
                }],
            },
            &sink,
            &mut answer,
        )
        .await;
        emit_search_update(
            avrag_search::SearchStreamUpdate::TextDelta {
                delta: "answer".to_string(),
            },
            &sink,
            &mut answer,
        )
        .await;

        let events = sink.events();
        assert_eq!(answer, "answer");
        assert!(matches!(events[0], AgentEvent::Activity { .. }));
        assert!(matches!(events[1], AgentEvent::Activity { .. }));
        assert!(matches!(events[2], AgentEvent::MessageDelta { .. }));
    }

    #[tokio::test]
    async fn brave_answer_synthesis_streams_fake_llm_deltas_in_order() {
        let sink = CollectingSink::new();
        let search_response = SearchResponse {
            query_type: "brave_llm_context".to_string(),
            sub_queries: vec!["atlas rollback".to_string()],
            results: vec![SearchResult {
                title: "Atlas Checklist".to_string(),
                url: "https://example.com/atlas".to_string(),
                snippet: "Atlas uses the rollback checklist.".to_string(),
                citation_index: Some(1),
            }],
            synthesized_answer: "evidence fallback".to_string(),
            llm_usage: None,
        };
        let fake = FakeSearchAnswerSynthesizer;

        let (answer, usage) = synthesize_brave_answer(
            Some(&fake as &dyn SearchAnswerSynthesizer),
            SynthesizeBraveParams {
                temperature: Some(0.2),
                query: "How does Atlas handle rollback?",
                search_response: &search_response,
                stream: true,
                session_summary: None,
                user_preferences: None,
            },
            CancellationToken::new(),
            &sink,
        )
        .await
        .unwrap();

        assert_eq!(answer, "stream synthesized [[1]]");
        assert_eq!(usage.as_ref().map(|usage| usage.total_tokens), Some(15));
        let deltas = sink
            .events()
            .into_iter()
            .filter_map(|event| match event {
                AgentEvent::MessageDelta { text } => Some(text),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(deltas, vec!["stream ", "synthesized [[1]]"]);
    }

    async fn emit_debug_trace_if_requested(request_debug: bool) {
        let sink = CollectingSink::new();
        emit_search_debug_trace_if_requested(
            request_debug,
            &sink,
            serde_json::json!({"internal": true}),
        )
        .await;
        let debug_events = sink
            .events()
            .into_iter()
            .filter(|event| matches!(event, AgentEvent::DebugTrace { .. }))
            .count();
        assert_eq!(debug_events, usize::from(request_debug));
    }

    #[tokio::test]
    async fn search_debug_trace_requires_debug_flag() {
        emit_debug_trace_if_requested(false).await;
    }

    #[tokio::test]
    async fn search_debug_trace_is_emitted_when_debug_flag_is_set() {
        emit_debug_trace_if_requested(true).await;
    }

    #[test]
    fn search_answer_prompt_contains_evidence_and_citation_contract() {
        let messages = build_search_answer_messages(
            "How does Atlas handle rollback?",
            &[SearchResult {
                title: "Atlas Checklist".to_string(),
                url: "https://example.com/atlas".to_string(),
                snippet: "Atlas uses the rollback checklist.".to_string(),
                citation_index: Some(1),
            }],
            None,
            None,
        );

        assert!(messages[0].content.contains("Cite sources with [[n]]"));
        assert!(
            messages[1]
                .content
                .contains("How does Atlas handle rollback?")
        );
        assert!(messages[1].content.contains("[[1]] title: Atlas Checklist"));
        assert!(messages[1].content.contains("https://example.com/atlas"));
    }

    #[test]
    fn search_answer_prompt_includes_memory_when_present() {
        let messages = build_search_answer_messages(
            "What is Rust?",
            &[],
            Some("User is learning systems programming."),
            Some(&serde_json::json!({"style": "concise"})),
        );
        assert!(messages[0].content.contains("Session summary:"));
        assert!(messages[0].content.contains("User is learning systems programming."));
        assert!(messages[0].content.contains("User preferences:"));
        assert!(messages[0].content.contains("concise"));
    }

    // ---------------- ReAct helpers ----------------

    #[test]
    fn next_vertical_step_escalates_general_to_news() {
        assert_eq!(next_vertical_step(None), Some("news".to_string()));
    }

    #[test]
    fn next_vertical_step_exhausts_after_news() {
        assert_eq!(next_vertical_step(Some("news")), None);
        assert_eq!(next_vertical_step(Some("discussions")), None);
    }

    #[test]
    fn broaden_query_drops_trailing_modifier() {
        assert_eq!(broaden_query("rust async runtime"), "rust async");
        assert_eq!(broaden_query("rust async"), "rust");
    }

    #[test]
    fn broaden_query_preserves_single_token() {
        assert_eq!(broaden_query("rust"), "rust");
        assert_eq!(broaden_query(""), "");
    }

    #[test]
    fn decision_label_covers_all_advice_variants() {
        assert_eq!(decision_label(&EvalAdvice::Synthesize), "synthesize");
        assert_eq!(
            decision_label(&EvalAdvice::Clarify {
                question: "?".to_string()
            }),
            "clarify"
        );
        assert_eq!(
            decision_label(&EvalAdvice::Degrade {
                reason: DegradeReason::BudgetExhausted
            }),
            "degrade"
        );
        assert_eq!(decision_label(&EvalAdvice::Replan { reason: "x" }), "replan");
        assert_eq!(
            decision_label(&EvalAdvice::BroadenQuery { reason: "x" }),
            "broaden_query"
        );
        assert_eq!(
            decision_label(&EvalAdvice::EscalateVertical { reason: "x" }),
            "escalate_vertical"
        );
        assert_eq!(
            decision_label(&EvalAdvice::EscalateToSearch { reason: "x" }),
            "escalate_to_search"
        );
        assert_eq!(
            decision_label(&EvalAdvice::FetchFullPage { reason: "x" }),
            "fetch_full_page"
        );
    }

    #[test]
    fn renumber_citation_indexes_assigns_one_based_indexes() {
        let inputs = vec![
            SearchResult {
                title: "a".to_string(),
                url: "u1".to_string(),
                snippet: "s1".to_string(),
                citation_index: Some(42),
            },
            SearchResult {
                title: "b".to_string(),
                url: "u2".to_string(),
                snippet: "s2".to_string(),
                citation_index: None,
            },
        ];
        let renumbered = renumber_citation_indexes(&inputs);
        assert_eq!(renumbered[0].citation_index, Some(1));
        assert_eq!(renumbered[1].citation_index, Some(2));
    }

    #[test]
    fn build_citations_uses_one_based_citation_ids() {
        let inputs = vec![
            SearchResult {
                title: "Atlas".to_string(),
                url: "https://example.com/atlas".to_string(),
                snippet: "snippet".to_string(),
                citation_index: Some(1),
            },
            SearchResult {
                title: "Beta".to_string(),
                url: "https://example.com/beta".to_string(),
                snippet: "snip2".to_string(),
                citation_index: Some(2),
            },
        ];
        let citations = build_citations(&inputs);
        assert_eq!(citations.len(), 2);
        assert_eq!(citations[0].citation_id, 1);
        assert_eq!(citations[1].citation_id, 2);
        assert_eq!(citations[0].layer.as_deref(), Some("search"));
    }

    #[test]
    fn merge_usage_sums_token_counts_across_iterations() {
        let a = LlmUsage {
            provider: "p".to_string(),
            model: "m".to_string(),
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
            cached_tokens: 0,
        };
        let b = LlmUsage {
            provider: "p".to_string(),
            model: "m".to_string(),
            prompt_tokens: 7,
            completion_tokens: 3,
            total_tokens: 10,
            cached_tokens: 0,
        };
        let merged = merge_usage(Some(&a), &b);
        assert_eq!(merged.prompt_tokens, 17);
        assert_eq!(merged.completion_tokens, 8);
        assert_eq!(merged.total_tokens, 25);

        let from_empty = merge_usage(None, &b);
        assert_eq!(from_empty.prompt_tokens, 7);
    }

    // ---------------- E2E ReAct loop tests (P2-B.6) ----------------
    //
    // Scripted `FakeSearchProvider` exercises the full ReAct loop. Three
    // scenarios from `docs/CHAT_GRAPHFLOW_REMOVAL_AND_AGENT_REACT_2026-05-10.md`
    // §8 acceptance:
    //  * zero general results → escalate to news → synthesize (iterations == 2)
    //  * both verticals empty → degrade NoResultsAfterAllFallbacks
    //  * cancellation token fired mid-loop returns within 100 ms
    //
    // The fake records each `(query, vertical)` pair so we can assert decision ⑦
    // (fallback must change inputs) holds across iterations.

    use std::sync::Mutex;
    use std::time::Duration;

    /// Scripted search provider for the ReAct e2e tests. Returns responses from
    /// `responses` in order, falling back to the last entry once exhausted; logs
    /// every `(query, vertical)` invocation for assertions; optionally sleeps
    /// `delay_per_call` before responding so we can exercise cancellation.
    struct FakeSearchProvider {
        responses: Mutex<std::collections::VecDeque<SearchResponse>>,
        calls: Mutex<Vec<(String, Option<String>)>>,
        delay_per_call: Option<Duration>,
    }

    impl FakeSearchProvider {
        fn new(responses: Vec<SearchResponse>) -> Self {
            Self {
                responses: Mutex::new(responses.into_iter().collect()),
                calls: Mutex::new(Vec::new()),
                delay_per_call: None,
            }
        }

        fn with_delay(mut self, delay: Duration) -> Self {
            self.delay_per_call = Some(delay);
            self
        }

        fn call_log(&self) -> Vec<(String, Option<String>)> {
            self.calls.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl SearchProvider for FakeSearchProvider {
        async fn execute_search(
            &self,
            query: &str,
            vertical: Option<&str>,
        ) -> anyhow::Result<SearchResponse> {
            self.calls
                .lock()
                .unwrap()
                .push((query.to_string(), vertical.map(str::to_string)));
            if let Some(delay) = self.delay_per_call {
                tokio::time::sleep(delay).await;
            }
            let mut responses = self.responses.lock().unwrap();
            let response = match responses.len() {
                0 => SearchResponse {
                    query_type: "brave_llm_context".to_string(),
                    sub_queries: vec![],
                    results: vec![],
                    synthesized_answer: String::new(),
                    llm_usage: None,
                },
                1 => responses.front().cloned().unwrap(),
                _ => responses.pop_front().unwrap(),
            };
            Ok(response)
        }
    }

    fn make_request(query: &str) -> AgentRequest {
        AgentRequest {
            kind: crate::agents::AgentKind::Search,
            query: query.to_string(),
            notebook_id: None,
            session_id: Some("test-session".to_string()),
            doc_scope: vec![],
            messages: vec![],
            session_summary: None,
            user_preferences: None,
            language: None,
            docscope_metadata: None,
            debug: false,
            stream: false,
            auth_context: serde_json::json!({}),
            metadata: Default::default(),
            cancellation_token: None,
        }
    }

    fn make_result(title: &str, url: &str, snippet: &str) -> SearchResult {
        SearchResult {
            title: title.to_string(),
            url: url.to_string(),
            snippet: snippet.to_string(),
            citation_index: None,
        }
    }

    #[tokio::test]
    async fn react_e2e_zero_general_results_escalates_to_news_and_synthesizes() {
        // Iter 0: vertical=None returns empty. Evaluator → EscalateVertical.
        // Iter 1: vertical=Some("news") returns 2 hits whose snippets cover the
        //         query terms. Evaluator → Synthesize.
        let fake = Arc::new(FakeSearchProvider::new(vec![
            SearchResponse {
                query_type: "brave_llm_context".to_string(),
                sub_queries: vec!["atlas rollback procedure".to_string()],
                results: vec![],
                synthesized_answer: String::new(),
                llm_usage: None,
            },
            SearchResponse {
                query_type: "brave_news".to_string(),
                sub_queries: vec!["atlas rollback procedure".to_string()],
                results: vec![
                    make_result(
                        "Atlas Rollback Procedure",
                        "https://example.com/atlas",
                        "the atlas rollback procedure starts with halting writes",
                    ),
                    make_result(
                        "Recent Atlas Outage Notes",
                        "https://example.com/atlas-outage",
                        "operators executed the rollback procedure within 30 minutes",
                    ),
                ],
                synthesized_answer: "Atlas rolls back via the documented procedure.".to_string(),
                llm_usage: None,
            },
        ]));
        let agent =
            WebSearchAgent::new(Some(fake.clone() as Arc<dyn SearchProvider>));
        let sink = CollectingSink::new();
        let req = make_request("atlas rollback procedure");

        let result = agent.run(req, &sink).await.expect("agent run should succeed");

        // §8.3: iterations.len() must be > 1.
        assert_eq!(result.iterations.len(), 2, "expected two iterations");
        assert_eq!(result.iterations[0].decision, "escalate_vertical");
        assert_eq!(result.iterations[1].decision, "synthesize");
        assert_eq!(result.final_decision, Some(FinalDecision::Synthesized));

        // Decision ⑦: fallback must change inputs.
        let calls = fake.call_log();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], ("atlas rollback procedure".to_string(), None));
        assert_eq!(
            calls[1],
            (
                "atlas rollback procedure".to_string(),
                Some("news".to_string())
            )
        );

        // Provider's synthesized_answer is surfaced when no LLM synthesizer is
        // configured (evidence_fallback path); citations are still emitted.
        assert!(result
            .answer
            .contains("Atlas rolls back via the documented procedure"));
        assert_eq!(result.citations.len(), 2);
    }

    #[tokio::test]
    async fn react_e2e_zero_results_across_all_iterations_degrades() {
        // Iter 0: vertical=None returns empty → EscalateVertical.
        // Iter 1: vertical=Some("news") still empty. Budget exhausted +
        //         last_results empty + recall_count==0 → Degrade.
        let fake = Arc::new(FakeSearchProvider::new(vec![
            SearchResponse {
                query_type: "brave_llm_context".to_string(),
                sub_queries: vec!["obscure long-tail query that returns nothing".to_string()],
                results: vec![],
                synthesized_answer: String::new(),
                llm_usage: None,
            },
            SearchResponse {
                query_type: "brave_news".to_string(),
                sub_queries: vec!["obscure long-tail query that returns nothing".to_string()],
                results: vec![],
                synthesized_answer: String::new(),
                llm_usage: None,
            },
        ]));
        let agent =
            WebSearchAgent::new(Some(fake.clone() as Arc<dyn SearchProvider>));
        let sink = CollectingSink::new();
        let req = make_request("obscure long-tail query that returns nothing");

        let result = agent.run(req, &sink).await.expect("agent run should succeed");

        assert_eq!(result.iterations.len(), 2);
        assert_eq!(result.iterations[0].decision, "escalate_vertical");
        assert_eq!(result.iterations[1].decision, "degrade");
        match result.final_decision {
            Some(FinalDecision::Degraded {
                reason: DegradeReason::NoResultsAfterAllFallbacks,
            }) => {}
            other => panic!("expected Degraded(NoResultsAfterAllFallbacks), got {other:?}"),
        }
        // Degrade trace records the no_results stage.
        assert_eq!(result.degrade_trace.len(), 1);
        assert_eq!(result.degrade_trace[0].stage, "no_results");
    }

    #[tokio::test]
    async fn react_e2e_cancellation_mid_loop_returns_within_100ms() {
        // Provider sleeps 5 s per call. Cancellation fired after 10 ms must
        // unwind the in-flight `tokio::select!` and return promptly.
        let fake = Arc::new(
            FakeSearchProvider::new(vec![SearchResponse {
                query_type: "brave_llm_context".to_string(),
                sub_queries: vec![],
                results: vec![],
                synthesized_answer: String::new(),
                llm_usage: None,
            }])
            .with_delay(Duration::from_secs(5)),
        );
        let agent =
            WebSearchAgent::new(Some(fake.clone() as Arc<dyn SearchProvider>));
        let token = CancellationToken::new();
        let mut req = make_request("anything");
        req.cancellation_token = Some(token.clone());

        let token_for_canceler = token.clone();
        let canceler = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            token_for_canceler.cancel();
        });

        let sink = CollectingSink::new();
        let started = std::time::Instant::now();
        let result = tokio::time::timeout(Duration::from_millis(200), agent.run(req, &sink))
            .await
            .expect("agent.run must return before the 200ms timeout");
        let elapsed = started.elapsed();

        canceler.await.ok();

        assert!(
            elapsed < Duration::from_millis(150),
            "cancellation should return within 150ms, took {elapsed:?}"
        );
        let err = result.expect_err("cancelled agent must return Err");
        assert_eq!(err.code(), "request_cancelled");
        assert_eq!(err.http_status(), 499);
    }
}
