//! RagAgent — bounded ReAct loop with cross-iteration accumulation.
//!
//! Per `docs/CHAT_GRAPHFLOW_REMOVAL_AND_AGENT_REACT_2026-05-10.md` §4.3, the
//! RagAgent now drives a Plan → ExecuteTools → Evaluate cycle for up to 3
//! iterations (`LoopBudget::rag(UserTier::Pro)`). Across iterations it accumulates
//! retrieved chunks (deduped by `(doc_id, chunk_id)`, highest score kept) and
//! routes on the objective signals — `recall_count`, `max_score`,
//! `term_coverage` — produced by [`crate::agents::evaluator`].
//!
//! The "fallback must change input" type-system constraint (decision ⑦) is
//! honoured by constructing a fresh [`RagIterationParams`] for every continue
//! branch (`Replan`, `BroadenQuery`).

use crate::agents::evaluator::{
    AccumulatedRagResults, EvalAdvice, EvaluationSignals, evaluate_rag_iteration,
};
use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::react_loop::{
    UserTier,
    DegradeReason, LoopBudget, NextStep, ReactContext, cancellation_error, emit_retry_activity,
};
use crate::agents::runtime::{
    Agent, AgentRequest, AgentRunResult, AgentRunUsage, FinalDecision, IterationRecord,
};
use crate::rag_prompts::{
    RAG_PLAN_SYSTEM_PROMPT, RagPlanDecision, build_rag_plan_user_prompt,
    parse_rag_plan_decision,
};
use avrag_llm::{ChatMessage as LlmChatMessage, LlmClient, LlmResponse, LlmUsage};
use common::{
    AnswerContextChunk, AppError, ChatRequest, DegradeTraceItem, ToolResult, ToolStatus,
};
use std::sync::Arc;
use std::time::Instant;

/// RagAgent handles retrieval-augmented generation queries via a bounded
/// ReAct loop driven by objective recall/coverage signals.
pub struct RagAgent {
    rag_runtime: Option<Arc<avrag_rag_core::RagRuntime>>,
    llm_client: Option<LlmClient>,
    temperature: Option<f32>,
}

impl RagAgent {
    pub fn new(
        rag_runtime: Option<Arc<avrag_rag_core::RagRuntime>>,
        llm_client: Option<LlmClient>,
        temperature: Option<f32>,
    ) -> Self {
        Self {
            rag_runtime,
            llm_client,
            temperature,
        }
    }
}

#[async_trait::async_trait]
impl Agent for RagAgent {
    #[tracing::instrument(skip(self, sink), fields(agent_kind = ?request.kind))]
    async fn run(
        &self,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, AppError> {
        if request.doc_scope.is_empty() {
            let _ = sink.emit(AgentEvent::Error {
                code: "missing_doc_scope".to_string(),
                message: "RAG mode requires a non-empty doc_scope".to_string(),
            })
            .await;
            return Err(AppError::validation(
                "missing_doc_scope",
                "RAG mode requires a non-empty doc_scope",
            ));
        }

        let Some(rag) = self.rag_runtime.clone() else {
            let _ = sink.emit(AgentEvent::Error {
                code: "rag_unavailable".to_string(),
                message: "RAG runtime is not configured".to_string(),
            })
            .await;
            return Err(AppError::internal("RAG runtime is not configured"));
        };

        let llm = self
            .llm_client
            .clone()
            .ok_or_else(|| AppError::internal("LLM client is not configured for RAG"))?;

        let auth: avrag_auth::AuthContext =
            serde_json::from_value(request.auth_context.clone()).map_err(|error| {
                AppError::internal(format!("Failed to deserialize auth context: {error}"))
            })?;

        let mut history: Vec<LlmChatMessage> = Vec::new();
        if let Some(summary) = request.session_summary.as_deref().filter(|s| !s.trim().is_empty()) {
            let mut system = String::from("Retrieval context:");
            system.push_str("\n\nSession summary:\n");
            system.push_str(summary.trim());
            if let Some(prefs) = request.user_preferences.as_ref() {
                system.push_str("\n\nUser preferences:\n");
                system.push_str(&prefs.to_string());
            }
            history.push(LlmChatMessage::system(system));
        }
        history.extend(request.messages.iter().map(|turn| match turn.role.as_str() {
            "assistant" => LlmChatMessage::assistant(&turn.content),
            _ => LlmChatMessage::user(&turn.content),
        }));

        let chat_req = ChatRequest {
            query: request.query.clone(),
            notebook_id: request.notebook_id.clone(),
            session_id: request.session_id.clone(),
            agent_type: "rag".to_string(),
            source_type: None,
            source_token: None,
            doc_scope: request.doc_scope.clone(),
            messages: request.messages.clone(),
            stream: request.stream,
            language: request.language.clone(),
        };

        let cancellation = request.cancellation_token.clone().unwrap_or_default();
        let trace_id = request
            .session_id
            .clone()
            .unwrap_or_else(|| "rag-agent".to_string());

        let mut state = RagRunState {
            rag,
            llm,
            auth,
            chat_req,
            history,
            request,
            temperature: self.temperature,
            budget: LoopBudget::rag(UserTier::Pro),
            accumulated: AccumulatedRagResults::new(),
            all_tool_results: Vec::new(),
            iterations: Vec::new(),
            total_tool_calls: 0,
            aggregated_usage: None,
            request_count: 0,
        };

        let ctx = ReactContext::new(sink, &cancellation, &trace_id);
        let outcome = run_react_loop(&mut state, &ctx).await?;

        match outcome {
            RagLoopOutcome::Synthesize => finalize_synthesize(state, sink).await,
            RagLoopOutcome::Clarify(message) => finalize_clarify(state, sink, message).await,
            RagLoopOutcome::Degrade(reason) => finalize_degrade(state, sink, reason).await,
        }
    }
}

/// Mutable per-run state owned by `RagAgent::run`. Held by reference inside the
/// loop driver so each iteration can append a record and merge retrieved
/// chunks into the cross-iteration accumulator.
struct RagRunState {
    rag: Arc<avrag_rag_core::RagRuntime>,
    llm: LlmClient,
    auth: avrag_auth::AuthContext,
    chat_req: ChatRequest,
    history: Vec<LlmChatMessage>,
    request: AgentRequest,
    temperature: Option<f32>,
    budget: LoopBudget,
    accumulated: AccumulatedRagResults,
    all_tool_results: Vec<ToolResult>,
    iterations: Vec<IterationRecord>,
    total_tool_calls: u32,
    aggregated_usage: Option<LlmUsage>,
    request_count: u64,
}

/// Iteration-scoped params. Constructing `LoopDecision::Continue` requires a
/// fresh value of this type — the type-system enforcement of decision ⑦.
#[derive(Debug, Clone)]
struct RagIterationParams {
    /// Query string used by the planner this iteration; may be broadened
    /// from the original.
    query: String,
    /// Annotation woven into the planner prompt; `None` for iteration 0.
    directive: Option<String>,
    /// Follow-up queries suggested by the LLM strategy evaluator.
    /// Injected into the planner prompt when non-empty.
    suggested_queries: Vec<String>,
}

enum RagLoopOutcome {
    Synthesize,
    Clarify(String),
    Degrade(DegradeReason),
}

async fn run_react_loop(
    state: &mut RagRunState,
    ctx: &ReactContext<'_>,
) -> Result<RagLoopOutcome, AppError> {
    let original_query = state.chat_req.query.clone();
    let mut params = RagIterationParams {
        query: original_query.clone(),
        directive: None,
        suggested_queries: Vec::new(),
    };

    loop {
        ctx.check_cancelled()?;

        let iteration_idx = state.budget.current;
        let iteration_started = Instant::now();

        ctx.emit_activity(
            "rag",
            format!("Planning retrieval (iteration {})", iteration_idx + 1),
        )
        .await;

        let plan_response = tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => {
                return Err(cancellation_error());
            }
            result = call_planner(state, &params) => {
                result?
            }
        };
        let planner_usage = plan_response.usage.clone();
        state.aggregated_usage = Some(merge_usage(state.aggregated_usage.as_ref(), &planner_usage));
        state.request_count = state.request_count.saturating_add(1);

        let plan_calls = match parse_rag_plan_decision(&plan_response.content, &state.chat_req) {
            Some(RagPlanDecision::Clarify(message)) => {
                state.budget.tick();
                state.iterations.push(IterationRecord {
                    iteration: iteration_idx,
                    plan: serde_json::json!({"action": "clarify"}),
                    signals: EvaluationSignals::default(),
                    decision: "clarify".to_string(),
                    elapsed_ms: iteration_started.elapsed().as_millis() as u64,
                    llm_evaluation: None,
                    usage: None,
                });
                return Ok(RagLoopOutcome::Clarify(message));
            }
            Some(RagPlanDecision::ToolCalls(calls)) => calls,
            None => {
                return Err(AppError::internal(
                    "RAG planner produced an invalid plan output",
                ));
            }
        };

        ctx.emit_activity(
            "rag",
            format!("Retrieving evidence (iteration {})", iteration_idx + 1),
        )
        .await;

        let plan_snapshot = serde_json::to_value(&plan_calls).unwrap_or(serde_json::Value::Null);
        let n_calls = plan_calls.len() as u32;
        let tool_results = tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => {
                return Err(cancellation_error());
            }
            results = state.rag.execute_tools(&state.auth, plan_calls.clone()) => {
                results
            }
        };
        state.total_tool_calls = state.total_tool_calls.saturating_add(n_calls);
        state.all_tool_results.extend(tool_results.iter().cloned());

        let chunks = extract_chunks_with_scores(&tool_results);
        let texts: Vec<&str> = chunks.iter().map(|(c, _)| c.text.as_str()).collect();
        let signals = EvaluationSignals {
            recall_count: chunks.len(),
            max_score: chunks.iter().map(|(_, s)| *s).fold(0.0_f32, f32::max),
            term_coverage: EvaluationSignals::compute_term_coverage(&original_query, &texts),
            zero_hits_per_subquery: Vec::new(),
        };

        state
            .accumulated
            .merge_iteration(chunks.into_iter(), iteration_idx);

        state.budget.tick();
        let elapsed_ms = iteration_started.elapsed().as_millis() as u64;

        // --- Hard constraint: budget exhausted ---
        if state.budget.exhausted() {
            let decision = if state.accumulated.is_empty() {
                "degrade".to_string()
            } else {
                "synthesize".to_string()
            };
            state.iterations.push(IterationRecord {
                iteration: iteration_idx,
                plan: plan_snapshot,
                signals: signals.clone(),
                decision,
                elapsed_ms,
                llm_evaluation: None,
                usage: None,
            });
            if state.accumulated.is_empty() {
                return Ok(RagLoopOutcome::Degrade(
                    DegradeReason::NoResultsAfterAllFallbacks,
                ));
            }
            return Ok(RagLoopOutcome::Synthesize);
        }

        // --- Default: LLM strategy evaluation ---
        let strategy_advice =
            evaluate_retrieval_strategy(state, &original_query, &plan_calls, &tool_results, iteration_idx).await;

        // Aggregate evaluator usage into run totals
        if let Some((_, eval_usage)) = &strategy_advice {
            state.aggregated_usage = Some(merge_usage(state.aggregated_usage.as_ref(), eval_usage));
            state.request_count = state.request_count.saturating_add(1);
        }

        let (decision_str, llm_eval_json) = match &strategy_advice {
            Some((eval, _)) => {
                let label = match eval.recommendation {
                    crate::rag_prompts::StrategyRecommendation::Synthesize => "synthesize".to_string(),
                    crate::rag_prompts::StrategyRecommendation::Replan => "replan".to_string(),
                    crate::rag_prompts::StrategyRecommendation::Broaden => "broaden_query".to_string(),
                };
                let json = serde_json::to_value(eval).ok();
                (label, json)
            }
            None => {
                // Fallback to code evaluator if LLM strategy evaluation fails
                let code_advice = evaluate_rag_iteration(&signals, &state.budget, &state.accumulated);
                (decision_label(&code_advice).to_string(), None)
            }
        };

        // Per-iteration usage = planner + evaluator
        let iter_usage = strategy_advice
            .as_ref()
            .map(|(_, eval_u)| merge_usage(Some(&planner_usage), eval_u));
        let iter_agent_usage = build_run_usage(iter_usage.as_ref(), 0);

        state.iterations.push(IterationRecord {
            iteration: iteration_idx,
            plan: plan_snapshot,
            signals: signals.clone(),
            decision: decision_str,
            elapsed_ms,
            llm_evaluation: llm_eval_json,
            usage: iter_agent_usage,
        });

        match strategy_advice {
            Some((eval, _)) => match eval.recommendation {
                crate::rag_prompts::StrategyRecommendation::Synthesize => {
                    return Ok(RagLoopOutcome::Synthesize);
                }
                crate::rag_prompts::StrategyRecommendation::Replan => {
                    let reason = eval.reason;
                    let suggested = eval.suggested_followup_queries.clone();
                    emit_retry_activity(ctx, NextStep::Replan, &reason).await;
                    params = RagIterationParams {
                        query: original_query.clone(),
                        directive: Some(format!("replan: {reason}")),
                        suggested_queries: suggested,
                    };
                }
                crate::rag_prompts::StrategyRecommendation::Broaden => {
                    let reason = eval.reason;
                    let suggested = eval.suggested_followup_queries.clone();
                    emit_retry_activity(ctx, NextStep::BroadenQuery, &reason).await;
                    params = RagIterationParams {
                        // When LLM provides suggested queries, keep the original query
                        // and let the planner use the suggestions. Only mechanically
                        // broaden when LLM gave no guidance.
                        query: if suggested.is_empty() {
                            broaden_query(&params.query)
                        } else {
                            params.query.clone()
                        },
                        directive: Some(format!("broaden: {reason}")),
                        suggested_queries: suggested,
                    };
                }
            },
            None => {
                // Fallback to code evaluator if LLM strategy evaluation fails
                let advice = evaluate_rag_iteration(&signals, &state.budget, &state.accumulated);
                match advice {
                    EvalAdvice::Synthesize => return Ok(RagLoopOutcome::Synthesize),
                    EvalAdvice::Clarify { question } => return Ok(RagLoopOutcome::Clarify(question)),
                    EvalAdvice::Degrade { reason } => return Ok(RagLoopOutcome::Degrade(reason)),
                    EvalAdvice::Replan { reason } => {
                        emit_retry_activity(ctx, NextStep::Replan, reason).await;
                        params = RagIterationParams {
                            query: original_query.clone(),
                            directive: Some(format!("replan: {reason}")),
                            suggested_queries: Vec::new(),
                        };
                    }
                    EvalAdvice::BroadenQuery { reason } => {
                        emit_retry_activity(ctx, NextStep::BroadenQuery, reason).await;
                        params = RagIterationParams {
                            query: broaden_query(&params.query),
                            directive: Some(format!("broaden: {reason}")),
                            suggested_queries: Vec::new(),
                        };
                    }
                    EvalAdvice::EscalateToSearch { reason } => {
                        return Ok(RagLoopOutcome::Degrade(DegradeReason::Other(format!(
                            "escalate_to_search: {reason}"
                        ))));
                    }
                    EvalAdvice::EscalateVertical { reason } | EvalAdvice::FetchFullPage { reason } => {
                        tracing::debug!(
                            %reason,
                            "rag evaluator returned search-only advice — treating as synthesize"
                        );
                        return Ok(RagLoopOutcome::Synthesize);
                    }
                }
            }
        }
    }
}

async fn call_planner(
    state: &RagRunState,
    params: &RagIterationParams,
) -> Result<LlmResponse, AppError> {
    let mut iter_chat_req = state.chat_req.clone();
    iter_chat_req.query = match &params.directive {
        Some(directive) => format!(
            "{}\n\n[iteration_directive]: {}\n[query_for_this_iteration]: {}",
            state.chat_req.query, directive, params.query,
        ),
        None => params.query.clone(),
    };
    let base_plan_prompt = build_rag_plan_user_prompt(
        &iter_chat_req,
        state.request.docscope_metadata.as_ref(),
    );
    let plan_user_prompt = if params.suggested_queries.is_empty() {
        base_plan_prompt
    } else {
        format!(
            "{}\n\n[suggested_followup_queries]:\n{}",
            base_plan_prompt,
            params.suggested_queries.iter().enumerate()
                .map(|(i, q)| format!("  - q{}: {}", i + 1, q))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    let mut plan_system = RAG_PLAN_SYSTEM_PROMPT.to_string();
    if let Some(summary) = state.request.session_summary.as_deref().filter(|s| !s.trim().is_empty()) {
        plan_system.push_str("\n\nSession summary:\n");
        plan_system.push_str(summary.trim());
    }
    if let Some(prefs) = state.request.user_preferences.as_ref() {
        plan_system.push_str("\n\nUser preferences:\n");
        plan_system.push_str(&prefs.to_string());
    }
    let plan_messages = vec![
        LlmChatMessage::system(plan_system),
        LlmChatMessage::user(plan_user_prompt),
    ];
    state
        .llm
        .complete(&plan_messages, state.temperature)
        .await
        .map_err(|error| AppError::internal(format!("RAG planning failed: {error}")))
}

async fn evaluate_retrieval_strategy(
    state: &RagRunState,
    original_query: &str,
    plan_calls: &[common::ToolCall],
    tool_results: &[common::ToolResult],
    iteration_idx: u8,
) -> Option<(crate::rag_prompts::RagStrategyEvaluation, LlmUsage)> {
    let sub_queries = extract_sub_queries_from_plan_calls(plan_calls);
    let prompt = crate::rag_prompts::build_rag_strategy_evaluation_prompt(
        original_query,
        &sub_queries,
        tool_results,
        state.accumulated.unique_chunk_count(),
        iteration_idx,
    );
    let messages = vec![
        avrag_llm::ChatMessage::system(crate::rag_prompts::RAG_STRATEGY_EVAL_SYSTEM_PROMPT),
        avrag_llm::ChatMessage::user(prompt),
    ];
    let response = state.llm.complete(&messages, state.temperature).await.ok()?;
    let eval = crate::rag_prompts::parse_rag_strategy_evaluation(&response.content)?;
    Some((eval, response.usage))
}

fn extract_sub_queries_from_plan_calls(plan_calls: &[common::ToolCall]) -> Vec<crate::rag_prompts::SubQueryItem> {
    let mut items = Vec::new();
    for (tool_idx, call) in plan_calls.iter().enumerate() {
        let mut found = false;
        if let Some(args) = call.args.as_object() {
            if let Some(qs) = args.get("queries").and_then(|v| v.as_array()) {
                for q in qs {
                    if let Some(s) = q.as_str() {
                        items.push(crate::rag_prompts::SubQueryItem {
                            id: format!("q{}", items.len() + 1),
                            text: s.to_string(),
                            tool_index: tool_idx,
                        });
                        found = true;
                    }
                }
            }
            if let Some(terms) = args.get("terms").and_then(|v| v.as_array()) {
                let term_str: Vec<String> = terms
                    .iter()
                    .filter_map(|t| t.as_str().map(|s| s.to_string()))
                    .collect();
                if !term_str.is_empty() {
                    items.push(crate::rag_prompts::SubQueryItem {
                        id: format!("q{}", items.len() + 1),
                        text: format!("BM25: {}", term_str.join(", ")),
                        tool_index: tool_idx,
                    });
                    found = true;
                }
            }
        }
        if !found {
            items.push(crate::rag_prompts::SubQueryItem {
                id: format!("q{}", items.len() + 1),
                text: call.tool.clone(),
                tool_index: tool_idx,
            });
        }
    }
    items
}

async fn finalize_synthesize(
    mut state: RagRunState,
    sink: &dyn AgentEventSink,
) -> Result<AgentRunResult, AppError> {
    if !has_evidence(&state.all_tool_results) {
        return finalize_degrade(state, sink, DegradeReason::NoResultsAfterAllFallbacks).await;
    }

    let _ = sink.emit(AgentEvent::Activity {
        stage: "rag".to_string(),
        message: "Synthesizing answer".to_string(),
    })
    .await;

    let synthesizer = avrag_llm::AnswerSynthesizer::from_llm_client(state.llm.clone());
    let cancellation = state
        .request
        .cancellation_token
        .clone()
        .unwrap_or_default();

    let (answer, synth_usage): (String, Option<LlmUsage>) = if state.request.stream {
        let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let synthesis_future = synthesizer.synthesize_stream_text_from_tool_results(
            &state.request.query,
            &state.all_tool_results,
            Some(&state.history),
            cancellation,
            move |delta: &str| {
                if !delta.is_empty() {
                    let _ = delta_tx.send(delta.to_string());
                }
            },
        );
        tokio::pin!(synthesis_future);

        let response = loop {
            tokio::select! {
                delta = delta_rx.recv() => {
                    if let Some(delta) = delta {
                        let _ = sink.emit(AgentEvent::MessageDelta { text: delta }).await;
                    }
                }
                result = &mut synthesis_future => {
                    break result.map_err(|error| {
                        AppError::internal(format!("RAG synthesis stream failed: {error}"))
                    })?;
                }
            }
        };

        while let Ok(delta) = delta_rx.try_recv() {
            let _ = sink.emit(AgentEvent::MessageDelta { text: delta }).await;
        }

        (response.content, Some(response.usage))
    } else {
        let synthesis = synthesizer
            .synthesize_from_tool_results(
                &state.request.query,
                &state.all_tool_results,
                Some(&state.history),
            )
            .await
            .map_err(|error| AppError::internal(format!("RAG synthesis failed: {error}")))?;

        let text = synthesis.answer_text.clone();
        let _ = sink.emit(AgentEvent::MessageDelta { text }).await;
        (synthesis.answer_text, synthesis.llm_usage)
    };

    if let Some(synth) = synth_usage.as_ref() {
        state.aggregated_usage = Some(merge_usage(state.aggregated_usage.as_ref(), synth));
        state.request_count = state.request_count.saturating_add(1);
    }

    let run_usage = build_run_usage(state.aggregated_usage.as_ref(), state.request_count);
    emit_usage(sink, run_usage.as_ref()).await;

    let citations = build_citations_from_tool_results(&state.all_tool_results);
    if !citations.is_empty() {
        let _ = sink.emit(AgentEvent::Citations {
            citations: citations.clone(),
        })
        .await;
    }

    let _ = sink.emit(AgentEvent::Done {
        final_message: Some(answer.clone()),
        usage: run_usage.as_ref().map(run_usage_to_agent_usage),
    })
    .await;

    let sources = build_sources_from_tool_results(&state.all_tool_results);

    Ok(AgentRunResult {
        answer,
        citations,
        sources,
        usage: run_usage,
        iterations: state.iterations,
        total_tool_calls: state.total_tool_calls,
        final_decision: Some(FinalDecision::Synthesized),
        ..Default::default()
    })
}

async fn finalize_clarify(
    state: RagRunState,
    sink: &dyn AgentEventSink,
    message: String,
) -> Result<AgentRunResult, AppError> {
    let _ = sink.emit(AgentEvent::MessageDelta {
        text: message.clone(),
    })
    .await;

    let run_usage = build_run_usage(state.aggregated_usage.as_ref(), state.request_count);
    emit_usage(sink, run_usage.as_ref()).await;

    let _ = sink.emit(AgentEvent::Done {
        final_message: Some(message.clone()),
        usage: run_usage.as_ref().map(run_usage_to_agent_usage),
    })
    .await;

    Ok(AgentRunResult {
        answer: message.clone(),
        usage: run_usage,
        iterations: state.iterations,
        total_tool_calls: state.total_tool_calls,
        final_decision: Some(FinalDecision::Clarified { question: message }),
        ..Default::default()
    })
}

async fn finalize_degrade(
    state: RagRunState,
    sink: &dyn AgentEventSink,
    reason: DegradeReason,
) -> Result<AgentRunResult, AppError> {
    let fallback = crate::chat::i18n::fallback::no_valid_retrieval_results(
        state.request.language.as_deref(),
    )
    .to_string();

    let _ = sink.emit(AgentEvent::MessageDelta {
        text: fallback.clone(),
    })
    .await;

    let run_usage = build_run_usage(state.aggregated_usage.as_ref(), state.request_count);
    emit_usage(sink, run_usage.as_ref()).await;

    let _ = sink.emit(AgentEvent::Done {
        final_message: Some(fallback.clone()),
        usage: run_usage.as_ref().map(run_usage_to_agent_usage),
    })
    .await;

    let degrade_trace = vec![DegradeTraceItem {
        stage: reason.as_stage().to_string(),
        reason: reason.message(),
        impact: "returned fallback message — no synthesized answer".to_string(),
    }];

    Ok(AgentRunResult {
        answer: fallback,
        degrade_trace,
        usage: run_usage,
        iterations: state.iterations,
        total_tool_calls: state.total_tool_calls,
        final_decision: Some(FinalDecision::Degraded { reason }),
        ..Default::default()
    })
}

fn extract_chunks_with_scores(tool_results: &[ToolResult]) -> Vec<(AnswerContextChunk, f32)> {
    let mut out = Vec::new();
    for result in tool_results {
        if result.status != ToolStatus::Ok {
            continue;
        }
        let Some(items) = result.data.as_ref().and_then(|data| data.as_array()) else {
            continue;
        };
        for item in items {
            let Some(chunk_id) = item
                .get("chunk_id")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .filter(|id| !id.is_empty())
            else {
                continue;
            };
            let doc_id = item
                .get("doc_id")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            let text = item
                .get("text")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .unwrap_or_default();
            let page = item.get("page").and_then(|v| v.as_i64());
            let score = item.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;

            out.push((
                AnswerContextChunk {
                    chunk_id,
                    doc_id,
                    chunk_type: "text".to_string(),
                    page,
                    text,
                    asset_id: None,
                    caption: None,
                    image_url: None,
                    parser_backend: None,
                    source_locator: None,
                },
                score,
            ));
        }
    }
    out
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

fn has_evidence(tool_results: &[ToolResult]) -> bool {
    tool_results.iter().any(|result| {
        result.status == ToolStatus::Ok
            && result
                .data
                .as_ref()
                .and_then(|data| data.as_array())
                .is_some_and(|array| !array.is_empty())
    })
}

fn build_citations_from_tool_results(tool_results: &[ToolResult]) -> Vec<common::Citation> {
    let mut citations = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut next_id: i64 = 1;

    for result in tool_results {
        if result.status != ToolStatus::Ok {
            continue;
        }
        let Some(items) = result.data.as_ref().and_then(|data| data.as_array()) else {
            continue;
        };
        for item in items {
            let Some(chunk_id) = item
                .get("chunk_id")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .filter(|id| !id.is_empty())
            else {
                continue;
            };
            if !seen.insert(chunk_id.clone()) {
                continue;
            }
            let doc_id = item
                .get("doc_id")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .unwrap_or_default();
            let text = item
                .get("text")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .unwrap_or_default();
            let page = item
                .get("page")
                .and_then(|v| v.as_i64())
                .map(|p| p as usize);
            let score = item.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;

            citations.push(common::Citation {
                citation_id: next_id,
                doc_id: doc_id.clone(),
                chunk_id: Some(chunk_id),
                page,
                doc_name: doc_id,
                preview: Some(text.chars().take(200).collect()),
                content: Some(text),
                score,
                layer: Some(result.tool.clone()),
                chunk_type: Some("text".to_string()),
                asset_id: None,
                caption: None,
                image_url: None,
                parser_backend: None,
                source_locator: None,
                parse_run_id: None,
            });
            next_id += 1;
        }
    }
    citations
}

fn build_sources_from_tool_results(tool_results: &[ToolResult]) -> Vec<common::SourceRef> {
    let mut sources = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for result in tool_results {
        if result.status != ToolStatus::Ok {
            continue;
        }
        let Some(items) = result.data.as_ref().and_then(|data| data.as_array()) else {
            continue;
        };
        for item in items {
            let Some(chunk_id) = item
                .get("chunk_id")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .filter(|id| !id.is_empty())
            else {
                continue;
            };
            if !seen.insert(chunk_id.clone()) {
                continue;
            }
            let doc_id = item
                .get("doc_id")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            let text = item
                .get("text")
                .and_then(|v| v.as_str())
                .map(|s| s.chars().take(200).collect::<String>());
            let page = item
                .get("page")
                .and_then(|v| v.as_i64())
                .map(|p| p as usize);

            sources.push(common::SourceRef {
                id: chunk_id.clone(),
                title: format!("Chunk {chunk_id}"),
                snippet: text,
                doc_id,
                page,
            });
        }
    }
    sources
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::events::CollectingSink;

    #[tokio::test]
    async fn test_rag_agent_rejects_empty_doc_scope() {
        let agent = RagAgent::new(None, None, None);
        let sink = CollectingSink::new();
        let req = AgentRequest {
            kind: crate::agents::AgentKind::Rag,
            query: "q".to_string(),
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
        assert!(events.iter().any(|e| matches!(
            e,
            AgentEvent::Error { code, .. } if code == "missing_doc_scope"
        )));
    }

    #[tokio::test]
    async fn test_rag_agent_without_runtime_returns_error() {
        let agent = RagAgent::new(None, None, None);
        let sink = CollectingSink::new();
        let req = AgentRequest {
            kind: crate::agents::AgentKind::Rag,
            query: "q".to_string(),
            notebook_id: None,
            session_id: None,
            doc_scope: vec!["doc1".to_string()],
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
        assert!(events.iter().any(|e| matches!(
            e,
            AgentEvent::Error { code, .. } if code == "rag_unavailable"
        )));
    }

    #[test]
    fn has_evidence_returns_true_when_a_tool_returned_chunks() {
        let results = vec![
            ToolResult {
                tool: "lexical_retrieval".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!([])),
                trace: None,
            },
            ToolResult {
                tool: "dense_retrieval".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!([
                    {"chunk_id": "c1", "doc_id": "d1", "text": "hello"}
                ])),
                trace: None,
            },
        ];
        assert!(has_evidence(&results));
    }

    #[test]
    fn has_evidence_returns_false_when_only_errors_or_empty_arrays() {
        let results = vec![
            ToolResult {
                tool: "lexical_retrieval".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({"error": "boom"})),
                trace: None,
            },
            ToolResult {
                tool: "dense_retrieval".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!([])),
                trace: None,
            },
        ];
        assert!(!has_evidence(&results));
    }

    #[test]
    fn build_citations_dedupes_by_chunk_id_and_skips_errors() {
        let results = vec![
            ToolResult {
                tool: "dense_retrieval".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!([
                    {"chunk_id": "c1", "doc_id": "d1", "text": "first", "page": 1, "score": 0.9}
                ])),
                trace: None,
            },
            ToolResult {
                tool: "lexical_retrieval".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!([
                    {"chunk_id": "c1", "doc_id": "d1", "text": "duplicate", "page": 1, "score": 0.5},
                    {"chunk_id": "c2", "doc_id": "d1", "text": "second", "score": 0.4}
                ])),
                trace: None,
            },
            ToolResult {
                tool: "dense_retrieval".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({"error": "boom"})),
                trace: None,
            },
        ];

        let citations = build_citations_from_tool_results(&results);
        assert_eq!(citations.len(), 2);
        assert_eq!(citations[0].chunk_id.as_deref(), Some("c1"));
        assert_eq!(citations[0].layer.as_deref(), Some("dense_retrieval"));
        assert_eq!(citations[1].chunk_id.as_deref(), Some("c2"));
        assert_eq!(citations[1].layer.as_deref(), Some("lexical_retrieval"));
    }

    // ---------------- ReAct helpers ----------------

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
    fn extract_chunks_with_scores_skips_errors_and_empty() {
        let results = vec![
            ToolResult {
                tool: "dense_retrieval".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!([
                    {"chunk_id": "c1", "doc_id": "d1", "text": "alpha", "score": 0.5}
                ])),
                trace: None,
            },
            ToolResult {
                tool: "lexical_retrieval".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({"error": "boom"})),
                trace: None,
            },
            ToolResult {
                tool: "lexical_retrieval".to_string(),
                version: "1.0".to_string(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!([])),
                trace: None,
            },
        ];
        let extracted = extract_chunks_with_scores(&results);
        assert_eq!(extracted.len(), 1);
        assert_eq!(extracted[0].0.chunk_id, "c1");
        assert!((extracted[0].1 - 0.5).abs() < 1e-6);
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

        // Starting from None — first call simply clones the new usage.
        let from_empty = merge_usage(None, &b);
        assert_eq!(from_empty.prompt_tokens, 7);
    }

    // ---------------- LLM strategy evaluation helpers ----------------

    #[test]
    fn extract_sub_queries_from_dense_retrieval_calls() {
        let calls = vec![
            common::ToolCall {
                tool: "dense_retrieval".to_string(),
                version: "1.0".to_string(),
                args: serde_json::json!({
                    "queries": ["rust async runtime", "tokio task scheduling"],
                    "modality": "text",
                    "top_k": 10,
                }),
            },
        ];
        let items = extract_sub_queries_from_plan_calls(&calls);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "q1");
        assert_eq!(items[0].text, "rust async runtime");
        assert_eq!(items[0].tool_index, 0);
        assert_eq!(items[1].id, "q2");
        assert_eq!(items[1].text, "tokio task scheduling");
        assert_eq!(items[1].tool_index, 0);
    }

    #[test]
    fn extract_sub_queries_from_lexical_retrieval_calls() {
        let calls = vec![
            common::ToolCall {
                tool: "lexical_retrieval".to_string(),
                version: "1.0".to_string(),
                args: serde_json::json!({
                    "terms": ["async", "runtime", "tokio"],
                    "top_k": 10,
                }),
            },
        ];
        let items = extract_sub_queries_from_plan_calls(&calls);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "q1");
        assert_eq!(items[0].text, "BM25: async, runtime, tokio");
        assert_eq!(items[0].tool_index, 0);
    }

    #[test]
    fn extract_sub_queries_falls_back_to_tool_name_when_no_args() {
        let calls = vec![
            common::ToolCall {
                tool: "graph_retrieval".to_string(),
                version: "1.0".to_string(),
                args: serde_json::json!({}),
            },
        ];
        let items = extract_sub_queries_from_plan_calls(&calls);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "q1");
        assert_eq!(items[0].text, "graph_retrieval");
        assert_eq!(items[0].tool_index, 0);
    }

    #[test]
    fn extract_sub_queries_collects_from_mixed_calls() {
        let calls = vec![
            common::ToolCall {
                tool: "dense_retrieval".to_string(),
                version: "1.0".to_string(),
                args: serde_json::json!({
                    "queries": ["query A"],
                }),
            },
            common::ToolCall {
                tool: "lexical_retrieval".to_string(),
                version: "1.0".to_string(),
                args: serde_json::json!({
                    "terms": ["term1", "term2"],
                }),
            },
            common::ToolCall {
                tool: "doc_summary".to_string(),
                version: "1.0".to_string(),
                args: serde_json::json!({ "doc_ids": ["d1"] }),
            },
        ];
        let items = extract_sub_queries_from_plan_calls(&calls);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].text, "query A");
        assert_eq!(items[0].tool_index, 0);
        assert_eq!(items[1].text, "BM25: term1, term2");
        assert_eq!(items[1].tool_index, 1);
        assert_eq!(items[2].text, "doc_summary");
        assert_eq!(items[2].tool_index, 2);
    }

    #[test]
    fn extract_sub_queries_maps_multi_query_tool_to_same_index() {
        let calls = vec![
            common::ToolCall {
                tool: "dense_retrieval".to_string(),
                version: "1.0".to_string(),
                args: serde_json::json!({
                    "queries": ["query A", "query B"],
                }),
            },
            common::ToolCall {
                tool: "lexical_retrieval".to_string(),
                version: "1.0".to_string(),
                args: serde_json::json!({
                    "terms": ["term1"],
                }),
            },
        ];
        let items = extract_sub_queries_from_plan_calls(&calls);
        assert_eq!(items.len(), 3);
        // Both dense queries map to tool_index 0
        assert_eq!(items[0].tool_index, 0);
        assert_eq!(items[1].tool_index, 0);
        // Lexical maps to tool_index 1
        assert_eq!(items[2].tool_index, 1);
    }
}
