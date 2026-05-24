//! RagStrategy — v5 state machine for RAG mode.
//!
//! RAG is multi-iteration with retrieval, evaluation, and optional replan:
//!   Plan → ExecuteRetrieve → Evaluate → Answer
//!                              ↓ replan/broaden, budget ok
//!                         Plan (loop)

use super::{AgentErrorKind, State, StateKind, StepOutcome, Strategy, StrategyContext};
use crate::agents::evaluator::{
    evaluate_rag_iteration, AccumulatedRagResults, EvalAdvice, EvaluationSignals,
};
use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::progressive::PromptRegistry;
use crate::agents::react_loop::{DegradeReason, LoopBudget};
use crate::agents::runtime::{AgentRequest, AgentRunResult, FinalDecision, IterationRecord};
use crate::agents::unified::helpers;
use common::{AppError, ChatRequest, ToolCall, ToolResult, ToolStatus};
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// RagState
// ---------------------------------------------------------------------------

/// States in the RAG state machine.
#[derive(Debug)]
pub enum RagState {
    /// Plan: run planner LLM to decide retrieval strategy and tool calls.
    Plan,
    /// ExecuteRetrieve: run RAG retrieval tools.
    ExecuteRetrieve,
    /// Evaluate: assess retrieval quality and decide next action.
    Evaluate,
    /// Answer: synthesize final response from accumulated evidence.
    Answer,
}

impl State for RagState {
    fn state_id(&self) -> &'static str {
        match self {
            RagState::Plan => "plan",
            RagState::ExecuteRetrieve => "execute_retrieve",
            RagState::Evaluate => "evaluate",
            RagState::Answer => "answer",
        }
    }

    fn state_kind(&self) -> StateKind {
        match self {
            RagState::Plan => StateKind::Plan,
            RagState::ExecuteRetrieve => StateKind::Execute,
            RagState::Evaluate => StateKind::Evaluate,
            RagState::Answer => StateKind::Answer,
        }
    }

    fn to_observable(&self) -> serde_json::Value {
        match self {
            RagState::Plan => serde_json::json!({"state": "plan"}),
            RagState::ExecuteRetrieve => serde_json::json!({"state": "execute_retrieve"}),
            RagState::Evaluate => serde_json::json!({"state": "evaluate"}),
            RagState::Answer => serde_json::json!({"state": "answer"}),
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ---------------------------------------------------------------------------
// RagIterationParams
// ---------------------------------------------------------------------------

/// Per-iteration parameters for RAG planning.
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct RagIterationParams {
    pub query: String,
    pub directive: Option<String>,
    pub suggested_queries: Vec<String>,
}


// ---------------------------------------------------------------------------
// RagContext
// ---------------------------------------------------------------------------

/// Runtime context for RagStrategy.
pub struct RagContext {
    pub request: AgentRequest,
    pub trace_id: String,
    pub budget: LoopBudget,
    pub sink: Box<dyn AgentEventSink>,
    pub cancel: CancellationToken,

    // RAG-specific runtime
    pub rag_runtime: std::sync::Arc<avrag_rag_core::RagRuntime>,
    pub auth: avrag_auth::AuthContext,
    pub chat_req: ChatRequest,
    pub history: Vec<avrag_llm::ChatMessage>,
    pub accumulated: AccumulatedRagResults,
    pub all_tool_results: Vec<ToolResult>,
    pub total_tool_calls: u32,
    pub content_guard_trace: Vec<common::DegradeTraceItem>,
    /// Tool call records for white-box reporting.
    pub tool_call_records: Vec<crate::agents::runtime::ToolCallRecord>,

    // Iteration state
    pub iteration_params: RagIterationParams,
    pub current_plan_calls: Option<Vec<ToolCall>>,
    pub current_plan_strategy: Option<crate::rag_prompts::PlanStrategy>,
    pub selected_skills: Vec<String>,
    pub iterations: Vec<IterationRecord>,

    // Accumulated
    pub aggregated_usage: Option<avrag_llm::LlmUsage>,
    pub request_count: u64,
}

impl StrategyContext for RagContext {
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

impl RagContext {
    /// Build a RagContext from an AgentRequest and runtime dependencies.
    pub fn from_request(
        request: AgentRequest,
        trace_id: String,
        budget: LoopBudget,
        sink: Box<dyn AgentEventSink>,
        cancel: CancellationToken,
        rag_runtime: std::sync::Arc<avrag_rag_core::RagRuntime>,
    ) -> Result<Self, AppError> {
        let auth: avrag_auth::AuthContext =
            serde_json::from_value(request.auth_context.clone()).map_err(|error| {
                AppError::internal(format!("Failed to deserialize auth context: {error}"))
            })?;

        let mut history: Vec<avrag_llm::ChatMessage> = Vec::new();
        if let Some(summary) = request
            .session_summary
            .as_deref()
            .filter(|s| !s.trim().is_empty())
        {
            let mut system = String::from("Retrieval context:");
            system.push_str("\n\nSession summary:\n");
            system.push_str(summary.trim());
            if let Some(prefs) = request.user_preferences.as_ref() {
                system.push_str("\n\nUser preferences:\n");
                system.push_str(&prefs.to_string());
            }
            history.push(avrag_llm::ChatMessage::system(system));
        }
        history.extend(request.messages.iter().map(|turn| match turn.role.as_str() {
            "assistant" => avrag_llm::ChatMessage::assistant(&turn.content),
            _ => avrag_llm::ChatMessage::user(&turn.content),
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

        Ok(Self {
            request,
            trace_id,
            budget,
            sink,
            cancel,
            rag_runtime,
            auth,
            chat_req,
            history,
            accumulated: AccumulatedRagResults::new(),
            all_tool_results: Vec::new(),
            total_tool_calls: 0,
            content_guard_trace: Vec::new(),
            tool_call_records: Vec::new(),
            iteration_params: RagIterationParams::default(),
            current_plan_calls: None,
            current_plan_strategy: None,
            selected_skills: Vec::new(),
            iterations: Vec::new(),
            aggregated_usage: None,
            request_count: 0,
        })
    }
}

// ---------------------------------------------------------------------------
// RagStrategy
// ---------------------------------------------------------------------------

/// Strategy implementation for RAG mode.
pub struct RagStrategy {
    pub llm: std::sync::Arc<dyn avrag_llm::LlmProvider>,
    pub llm_client: Option<avrag_llm::LlmClient>,
    pub temperature: Option<f32>,
}

#[async_trait::async_trait]
impl Strategy for RagStrategy {
    type Context = RagContext;

    async fn init(
        &self,
        _ctx: &mut RagContext,
    ) -> Result<Box<dyn State>, AppError> {
        Ok(Box::new(RagState::Plan))
    }

    fn schema() -> crate::agents::capability::StrategySchema {
        crate::agents::capability::StrategySchema {
            id: "rag".to_string(),
            states: vec![
                "Plan".to_string(),
                "ExecuteRetrieve".to_string(),
                "Evaluate".to_string(),
                "Answer".to_string(),
            ],
            transitions: vec![
                crate::agents::capability::TransitionSchema {
                    from: "Plan".to_string(),
                    to: "ExecuteRetrieve".to_string(),
                },
                crate::agents::capability::TransitionSchema {
                    from: "ExecuteRetrieve".to_string(),
                    to: "Evaluate".to_string(),
                },
                crate::agents::capability::TransitionSchema {
                    from: "Evaluate".to_string(),
                    to: "Answer".to_string(),
                },
                crate::agents::capability::TransitionSchema {
                    from: "Evaluate".to_string(),
                    to: "ExecuteRetrieve".to_string(),
                },
                crate::agents::capability::TransitionSchema {
                    from: "Evaluate".to_string(),
                    to: "Plan".to_string(),
                },
            ],
            external_tools_used: vec![],
            requires_internet: false,
            max_budget: 4,
        }
    }

    async fn step(
        &self,
        state: Box<dyn State>,
        ctx: &mut RagContext,
    ) -> Result<StepOutcome, AgentErrorKind> {
        let rag_state = state
            .as_any()
            .downcast_ref::<RagState>()
            .ok_or_else(|| AgentErrorKind::ModelOutputInvalid {
                expected_schema: "RagState".to_string(),
                got: "unknown state type".to_string(),
            })?;

        match rag_state {
            RagState::Plan => self.step_plan(ctx).await,
            RagState::ExecuteRetrieve => self.step_execute(ctx).await,
            RagState::Evaluate => self.step_evaluate(ctx).await,
            RagState::Answer => self.step_answer(ctx).await,
        }
    }
}

impl RagStrategy {
    // --- Plan step ---

    async fn step_plan(&self, ctx: &mut RagContext) -> Result<StepOutcome, AgentErrorKind> {
        ctx.check_cancelled()?;

        let iteration_idx = ctx.budget.current;

        let _ = ctx
            .sink
            .emit(AgentEvent::Activity {
                stage: "rag".to_string(),
                message: format!("Planning retrieval (iteration {})", iteration_idx + 1),
            })
            .await;

        let mut plan_system = crate::agents::strategy::prompts::build_plan_system_prompt(
            crate::agents::strategy::prompts::rag::PLANNER_SKILL_ID,
            "rag",
        );
        inject_memory_context(ctx, &mut plan_system);

        let plan_response = tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => {
                return Err(AgentErrorKind::Unknown("cancelled".to_string()));
            }
            result = self.call_planner(ctx, &plan_system) => {
                result?
            }
        };

        ctx.aggregated_usage = Some(helpers::merge_usage(
            ctx.aggregated_usage.as_ref(),
            &plan_response.usage,
        ));
        ctx.request_count += 1;

        match crate::rag_prompts::parse_rag_plan_decision(&plan_response.content, &ctx.chat_req) {
            Some((crate::rag_prompts::RagPlanDecision::Clarify(message), _)) => {
                Ok(StepOutcome::Terminate(AgentRunResult {
                    answer: message.clone(),
                    final_decision: Some(FinalDecision::Clarified { question: message }),
                    ..Default::default()
                }))
            }
            Some((crate::rag_prompts::RagPlanDecision::Strategy(strategy), skills)) => {
                ctx.current_plan_strategy = Some(strategy.clone());
                ctx.current_plan_calls = None;
                ctx.selected_skills = skills.clone();
                let _ = ctx
                    .sink
                    .emit(AgentEvent::PlanDecision {
                        selected_tools: vec![],
                        selected_skills: skills,
                        reasoning: format!("plan strategy: {:?}", strategy),
                    })
                    .await;
                Ok(StepOutcome::Next(Box::new(RagState::ExecuteRetrieve)))
            }
            Some((crate::rag_prompts::RagPlanDecision::ToolCalls(calls), skills)) => {
                ctx.current_plan_strategy = None;
                ctx.current_plan_calls = Some(calls.clone());
                ctx.selected_skills = skills.clone();
                let _ = ctx
                    .sink
                    .emit(AgentEvent::PlanDecision {
                        selected_tools: calls.clone(),
                        selected_skills: skills,
                        reasoning: format!("plan selected {} tool call(s)", calls.len()),
                    })
                    .await;
                Ok(StepOutcome::Next(Box::new(RagState::ExecuteRetrieve)))
            }
            None => Err(AgentErrorKind::ModelOutputInvalid {
                expected_schema: "RagPlanDecision".to_string(),
                got: "RAG planner produced an invalid plan output".to_string(),
            }),
        }
    }

    // --- Execute step ---

    async fn step_execute(&self, ctx: &mut RagContext) -> Result<StepOutcome, AgentErrorKind> {
        ctx.check_cancelled()?;

        let iteration_idx = ctx.budget.current;
        let iteration_started = std::time::Instant::now();

        // Convert strategy to tool calls if needed.
        if ctx.current_plan_calls.is_none() && ctx.current_plan_strategy.is_some() {
            let strategy = ctx.current_plan_strategy.take().unwrap();
            let tool_calls = crate::rag_prompts::plan_strategy_to_tool_calls(&strategy);
            ctx.current_plan_calls = Some(tool_calls);
        }

        let plan_calls = ctx.current_plan_calls.clone().unwrap_or_default();

        // Filter to RAG tools only + PolicyEnforcer check.
        let enforcer = crate::agents::capability::PolicyEnforcer::new(
            crate::agents::capability::standard_rules(),
        );
        let registry = crate::agents::capability::CapabilityRegistry::standard_cached();
        let mut denied_results: Vec<ToolResult> = Vec::new();
        let rag_calls: Vec<ToolCall> = plan_calls
            .into_iter()
            .filter(|call| {
                let ok = is_rag_tool(&call.tool);
                if !ok {
                    tracing::warn!(
                        tool = %call.tool,
                        "ignoring non-RAG tool in RAG execute"
                    );
                    return false;
                }
                // v5: PolicyEnforcer runtime check
                if let Some(meta) = registry.tool(&call.tool)
                    && let crate::agents::capability::EnforcementAction::Deny { reason } =
                        enforcer.evaluate(meta, Some(&ctx.auth))
                    {
                        tracing::warn!(
                            tool = %call.tool,
                            reason = %reason,
                            "PolicyEnforcer denied RAG tool call"
                        );
                        denied_results.push(ToolResult {
                            tool: call.tool.clone(),
                            version: call.version.clone(),
                            status: common::ToolStatus::Error,
                            data: Some(serde_json::json!({ "error": reason })),
                            trace: None,
                        });
                        return false;
                    }
                true
            })
            .collect();

        let n_calls = rag_calls.len() as u32;
        let plan_snapshot =
            serde_json::to_value(&rag_calls).unwrap_or(serde_json::Value::Null);

        let _ = ctx
            .sink
            .emit(AgentEvent::Activity {
                stage: "rag".to_string(),
                message: format!(
                    "Retrieving evidence (iteration {}) — {} tool call(s)",
                    iteration_idx + 1,
                    rag_calls.len(),
                ),
            })
            .await;

        // Save calls for white-box reporting before they are consumed.
        let calls_for_records = rag_calls.clone();

        let mut tool_results = tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => {
                return Err(AgentErrorKind::Unknown("cancelled".to_string()));
            }
            results = async {
                if rag_calls.is_empty() {
                    Vec::<ToolResult>::new()
                } else {
                    ctx.rag_runtime.execute_tools(&ctx.auth, rag_calls).await
                }
            } => results
        };

        // Merge any PolicyEnforcer-denied results.
        tool_results.extend(denied_results);

        ctx.total_tool_calls += n_calls;

        // Sanitize retrieved chunks against prompt injection.
        let (mut tool_results, sanitize_trace) =
            if let Some(ref guard) = ctx.request.guard_pipeline {
                crate::agents::content_guard::sanitize_tool_results(
                    &tool_results,
                    guard.as_ref(),
                    Some(ctx.trace_id.clone()),
                )
            } else {
                (tool_results, Vec::new())
            };
        ctx.content_guard_trace.extend(sanitize_trace);

        // v5: Apply UntrustedInputProcessor to all retrieved content before
        // it enters the Answer-phase LLM prompt.
        let mut rejected_reasons = Vec::new();
        for result in &mut tool_results {
            if result.status == common::ToolStatus::Ok {
                let reasons = crate::agents::untrusted_input::UntrustedInputProcessor::sanitize_tool_result_data(result, 0.8);
                rejected_reasons.extend(reasons);
            }
        }
        if !rejected_reasons.is_empty() {
            ctx.content_guard_trace.extend(rejected_reasons.iter().map(|reason| common::DegradeTraceItem {
                stage: "untrusted_input".to_string(),
                reason: reason.clone(),
                impact: format!("{} item(s) rejected before Answer phase", rejected_reasons.len()),
            }));
            let _ = ctx.sink.emit(crate::agents::events::AgentEvent::DebugTrace {
                kind: "untrusted_input.rejected".to_string(),
                payload: serde_json::json!({
                    "tool": "retrieval",
                    "rejected_count": rejected_reasons.len(),
                    "reasons": rejected_reasons,
                }),
            }).await;
        }

        // Record tool call details for white-box reporting.
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

        ctx.all_tool_results.extend(tool_results.iter().cloned());

        let chunks = helpers::extract_chunks_with_scores(&tool_results);
        let texts: Vec<&str> = chunks.iter().map(|(c, _)| c.text.as_str()).collect();
        let signals = EvaluationSignals {
            recall_count: chunks.len(),
            max_score: chunks.iter().map(|(_, s)| *s).fold(0.0_f32, f32::max),
            term_coverage: EvaluationSignals::compute_term_coverage(&ctx.iteration_params.query, &texts),
            zero_hits_per_subquery: Vec::new(),
        };

        ctx.accumulated.merge_iteration(chunks.into_iter(), iteration_idx);
        ctx.budget.tick();
        let _ = ctx
            .sink
            .emit(AgentEvent::BudgetTick {
                current: ctx.budget.current,
                max: ctx.budget.max_iterations,
            })
            .await;
        let elapsed_ms = iteration_started.elapsed().as_millis() as u64;

        // Budget exhausted: short-circuit to Answer.
        if ctx.budget.exhausted() {
            telemetry::prometheus::observe_agent_budget_exhausted("RagStrategy");
            let decision = if ctx.accumulated.is_empty() {
                "degrade".to_string()
            } else {
                "synthesize".to_string()
            };
            ctx.iterations.push(IterationRecord {
                iteration: iteration_idx,
                plan: plan_snapshot,
                signals: signals.clone(),
                decision: decision.clone(),
                elapsed_ms,
                llm_evaluation: None,
                usage: None,
            });
            let eval_signals = serde_json::to_value(&signals).ok();
            let _ = ctx
                .sink
                .emit(AgentEvent::Evaluation {
                    signals: eval_signals,
                    decision,
                    reasoning: "budget exhausted — forced decision".to_string(),
                })
                .await;
            if ctx.accumulated.is_empty() {
                return self.finalize_degrade(ctx, DegradeReason::NoResultsAfterAllFallbacks).await
                    .map(StepOutcome::Terminate);
            }
            return Ok(StepOutcome::Next(Box::new(RagState::Answer)));
        }

        // Store partial iteration record for evaluate.
        ctx.iterations.push(IterationRecord {
            iteration: iteration_idx,
            plan: plan_snapshot,
            signals: signals.clone(),
            decision: "pending".to_string(),
            elapsed_ms,
            llm_evaluation: None,
            usage: None,
        });

        Ok(StepOutcome::Next(Box::new(RagState::Evaluate)))
    }

    // --- Evaluate step ---

    async fn step_evaluate(&self, ctx: &mut RagContext) -> Result<StepOutcome, AgentErrorKind> {
        ctx.check_cancelled()?;

        let iteration_idx = ctx.budget.current;
        let original_query = ctx.chat_req.query.clone();
        let plan_calls = ctx.current_plan_calls.clone().unwrap_or_default();
        let tool_results = ctx.all_tool_results.clone();

        // Short-circuit: zero recall with no accumulated results → degrade immediately.
        // Prevents LLM from entering pointless replan loops on empty collections.
        if let Some(last_record) = ctx.iterations.last() {
            if last_record.signals.recall_count == 0 && ctx.accumulated.is_empty() {
                let _ = ctx
                    .sink
                    .emit(AgentEvent::Evaluation {
                        signals: serde_json::to_value(&last_record.signals).ok(),
                        decision: "degrade".to_string(),
                        reasoning: "zero recall and no accumulated results — skipping LLM evaluation".to_string(),
                    })
                    .await;
                if let Some(last) = ctx.iterations.last_mut() {
                    last.decision = "degrade".to_string();
                }
                return self.finalize_degrade(ctx, DegradeReason::NoResultsAfterAllFallbacks)
                    .await
                    .map(StepOutcome::Terminate);
            }
        }

        let eval_system = build_eval_system_prompt("rag");
        let strategy_advice = self
            .evaluate_retrieval_strategy(
                ctx,
                &original_query,
                &plan_calls,
                &tool_results,
                iteration_idx,
                &eval_system,
            )
            .await;

        if let Some((_, eval_usage)) = &strategy_advice {
            ctx.aggregated_usage = Some(helpers::merge_usage(
                ctx.aggregated_usage.as_ref(),
                eval_usage,
            ));
            ctx.request_count += 1;
        }

        let (decision_str, llm_eval_json) = match &strategy_advice {
            Some((eval, _)) => {
                let label = match eval.decision {
                    crate::rag_prompts::EvalDecision::Sufficient => "sufficient".to_string(),
                    crate::rag_prompts::EvalDecision::Insufficient => "insufficient".to_string(),
                    crate::rag_prompts::EvalDecision::GiveUp => "give_up".to_string(),
                };
                let json = serde_json::to_value(eval).ok();
                (label, json)
            }
            None => {
                let last_record = ctx.iterations.last().unwrap();
                let advice = evaluate_rag_iteration(
                    &last_record.signals,
                    &ctx.budget,
                    &ctx.accumulated,
                );
                (decision_label(&advice).to_string(), None)
            }
        };

        if let Some(last) = ctx.iterations.last_mut() {
            last.decision = decision_str.clone();
            last.llm_evaluation = llm_eval_json.clone();
        }

        let eval_signals = ctx.iterations.last().and_then(|it| serde_json::to_value(&it.signals).ok());
        let _ = ctx
            .sink
            .emit(AgentEvent::Evaluation {
                signals: eval_signals,
                decision: decision_str.clone(),
                reasoning: llm_eval_json.as_ref()
                    .and_then(|v| v.get("reasoning").and_then(|r| r.as_str().map(|s| s.to_string())))
                    .or_else(|| llm_eval_json.as_ref().and_then(|v| v.get("reason").and_then(|r| r.as_str().map(|s| s.to_string()))))
                    .unwrap_or_default(),
            })
            .await;

        match strategy_advice {
            Some((eval, _)) => match eval.decision {
                crate::rag_prompts::EvalDecision::Sufficient => {
                    Ok(StepOutcome::Next(Box::new(RagState::Answer)))
                }
                crate::rag_prompts::EvalDecision::GiveUp => {
                    self.finalize_degrade(ctx, DegradeReason::NoResultsAfterAllFallbacks)
                        .await
                        .map(StepOutcome::Terminate)
                }
                crate::rag_prompts::EvalDecision::Insufficient => {
                    // Convert next_actions directly to tool calls — skip Plan LLM.
                    let mut calls: Vec<ToolCall> = Vec::new();
                    for action in &eval.next_actions {
                        match action {
                            crate::rag_prompts::NextAction::SubQuery { query } => {
                                calls.push(ToolCall {
                                    tool: "dense_retrieval".to_string(),
                                    version: "1.0".to_string(),
                                    args: serde_json::json!({
                                        "queries": [query],
                                        "modality": "text",
                                        "top_k": 10,
                                    }),
                                });
                            }
                            crate::rag_prompts::NextAction::ToolCall { tool, args, reason: _ } => {
                                calls.push(ToolCall {
                                    tool: tool.clone(),
                                    version: "1.0".to_string(),
                                    args: args.clone(),
                                });
                            }
                        }
                    }

                    if calls.is_empty() {
                        // No actionable next actions — broaden as fallback
                        ctx.iteration_params = RagIterationParams {
                            query: helpers::broaden_query(&original_query),
                            directive: Some(format!("broaden: {}", eval.reasoning)),
                            suggested_queries: Vec::new(),
                        };
                        return Ok(StepOutcome::Next(Box::new(RagState::Plan)));
                    }

                    // Store calls for step_execute to dispatch directly
                    ctx.current_plan_calls = Some(calls);
                    ctx.iteration_params = RagIterationParams {
                        query: original_query.clone(),
                        directive: Some(format!("evaluate: {}", eval.reasoning)),
                        suggested_queries: Vec::new(),
                    };
                    Ok(StepOutcome::Next(Box::new(RagState::ExecuteRetrieve)))
                }
            },
            None => {
                let last_record = ctx.iterations.last().unwrap();
                let advice = evaluate_rag_iteration(
                    &last_record.signals,
                    &ctx.budget,
                    &ctx.accumulated,
                );
                match advice {
                    EvalAdvice::Synthesize => {
                        Ok(StepOutcome::Next(Box::new(RagState::Answer)))
                    }
                    EvalAdvice::Clarify { question } => {
                        Ok(StepOutcome::Terminate(AgentRunResult {
                            answer: question.clone(),
                            final_decision: Some(FinalDecision::Clarified { question }),
                            ..Default::default()
                        }))
                    }
                    EvalAdvice::Degrade { reason } => {
                        self.finalize_degrade(ctx, reason).await
                            .map(StepOutcome::Terminate)
                    }
                    EvalAdvice::Replan { reason } => {
                        let directive = if let Some(hint) =
                            build_doc_index_directive_hint(&tool_results)
                        {
                            Some(format!("replan: {}\n\n{}", reason, hint))
                        } else {
                            Some(format!("replan: {reason}"))
                        };
                        ctx.iteration_params = RagIterationParams {
                            query: original_query.clone(),
                            directive,
                            suggested_queries: Vec::new(),
                        };
                        Ok(StepOutcome::Next(Box::new(RagState::Plan)))
                    }
                    EvalAdvice::BroadenQuery { reason } => {
                        ctx.iteration_params = RagIterationParams {
                            query: helpers::broaden_query(&ctx.iteration_params.query),
                            directive: Some(format!("broaden: {reason}")),
                            suggested_queries: Vec::new(),
                        };
                        Ok(StepOutcome::Next(Box::new(RagState::Plan)))
                    }
                    EvalAdvice::EscalateToSearch { reason } => {
                        self.finalize_degrade(
                            ctx,
                            DegradeReason::Other(format!("escalate_to_search: {reason}")),
                        )
                        .await
                        .map(StepOutcome::Terminate)
                    }
                    EvalAdvice::EscalateVertical { .. } | EvalAdvice::FetchFullPage { .. } => {
                        Ok(StepOutcome::Next(Box::new(RagState::Answer)))
                    }
                }
            }
        }
    }

    // --- Answer step ---

    async fn step_answer(&self, ctx: &mut RagContext) -> Result<StepOutcome, AgentErrorKind> {
        ctx.check_cancelled()?;

        // Compute selected format skills: prefer explicit selection, fall back to keyword detection
        let selected_format_skills: Vec<String> = if !ctx.selected_skills.is_empty() {
            ctx.selected_skills.clone()
        } else {
            detect_format_skills(&ctx.request.query).iter().map(|s| s.to_string()).collect()
        };
        let system_prompt = crate::agents::strategy::prompts::build_answer_system_prompt(
            crate::agents::strategy::prompts::rag::ANSWER_SKILL_ID,
            "rag",
            &selected_format_skills,
        );

        if !helpers::has_evidence(&ctx.all_tool_results) {
            return self.finalize_degrade(ctx, DegradeReason::NoResultsAfterAllFallbacks)
                .await
                .map(StepOutcome::Terminate);
        }

        self.finalize_synthesize(ctx, &system_prompt).await
            .map(StepOutcome::Terminate)
    }

    // --- Helpers ---

    async fn call_planner(
        &self,
        ctx: &RagContext,
        system_prompt: &str,
    ) -> Result<avrag_llm::LlmResponse, AgentErrorKind> {
        let mut iter_chat_req = ctx.chat_req.clone();
        iter_chat_req.query = match &ctx.iteration_params.directive {
            Some(directive) => format!(
                "{}\n\n[iteration_directive]: {}\n[query_for_this_iteration]: {}",
                ctx.chat_req.query, directive, ctx.iteration_params.query,
            ),
            None => ctx.iteration_params.query.clone(),
        };

        let base_plan_prompt = crate::rag_prompts::build_rag_plan_user_prompt(
            &iter_chat_req,
            ctx.request.docscope_metadata.as_ref(),
            &ctx.all_tool_results,
        );
        let plan_user_prompt = if ctx.iteration_params.suggested_queries.is_empty() {
            base_plan_prompt
        } else {
            format!(
                "{}\n\n[suggested_followup_queries]:\n{}",
                base_plan_prompt,
                ctx.iteration_params.suggested_queries.iter().enumerate()
                    .map(|(i, q)| format!("  - q{}: {}", i + 1, q))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };

        let plan_messages = vec![
            avrag_llm::ChatMessage::system(system_prompt.to_string()),
            avrag_llm::ChatMessage::user(plan_user_prompt),
        ];

        self.llm
            .complete(&plan_messages, self.temperature)
            .await
            .map_err(|_e| AgentErrorKind::ModelUnavailable {
                provider: "unknown".to_string(),
                model: "unknown".to_string(),
            })
    }

    async fn evaluate_retrieval_strategy(
        &self,
        _ctx: &RagContext,
        original_query: &str,
        plan_calls: &[ToolCall],
        tool_results: &[ToolResult],
        iteration_idx: u8,
        system_prompt: &str,
    ) -> Option<(crate::rag_prompts::RagStrategyEvaluation, avrag_llm::LlmUsage)> {
        let sub_queries = extract_sub_queries_from_plan_calls(plan_calls);
        let chunks = extract_chunks_from_tool_results(tool_results);
        let prompt = crate::rag_prompts::build_rag_strategy_evaluation_prompt(
            original_query,
            &sub_queries,
            tool_results,
            &chunks,
            iteration_idx,
            15,
        );
        let messages = vec![
            avrag_llm::ChatMessage::system(system_prompt),
            avrag_llm::ChatMessage::user(prompt),
        ];
        let response = self.llm.complete(&messages, self.temperature).await.ok()?;
        let eval = crate::rag_prompts::parse_rag_strategy_evaluation(&response.content)?;
        Some((eval, response.usage))
    }

    async fn finalize_synthesize(
        &self,
        ctx: &mut RagContext,
        system_prompt: &str,
    ) -> Result<AgentRunResult, AgentErrorKind> {
        let sink = ctx.sink.as_ref();

        let _ = sink.emit(AgentEvent::Activity {
            stage: "rag".to_string(),
            message: "Synthesizing answer".to_string(),
        })
        .await;

        let llm_client = self.llm_client.clone()
            .ok_or_else(|| AgentErrorKind::ModelUnavailable {
                provider: "unknown".to_string(),
                model: "AnswerSynthesizer requires LlmClient".to_string(),
            })?;
        let synthesizer = avrag_llm::AnswerSynthesizer::from_llm_client(llm_client)
            .with_system_prompt(system_prompt);
        let cancel = ctx.cancel.clone();

        let (answer, synth_usage): (String, Option<avrag_llm::LlmUsage>) = if ctx.request.stream {
            let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
            let synthesis_future = synthesizer.synthesize_stream_text_from_tool_results(
                &ctx.request.query,
                &ctx.all_tool_results,
                Some(&ctx.history),
                cancel,
                move |delta: &str| {
                    if !delta.is_empty() {
                        let _ = delta_tx.send(delta.to_string());
                    }
                },
            );
            tokio::pin!(synthesis_future);

            let response = loop {
                tokio::select! {
                    biased;
                    _ = ctx.cancel.cancelled() => {
                        return Err(AgentErrorKind::Unknown("cancelled".to_string()));
                    }
                    delta = delta_rx.recv() => {
                        if let Some(delta) = delta {
                            let _ = sink.emit(AgentEvent::MessageDelta { text: delta }).await;
                        }
                    }
                    result = &mut synthesis_future => {
                        break result.map_err(|_e| AgentErrorKind::ModelUnavailable {
                            provider: "unknown".to_string(),
                            model: "unknown".to_string(),
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
                    &ctx.request.query,
                    &ctx.all_tool_results,
                    Some(&ctx.history),
                )
                .await
                .map_err(|_e| AgentErrorKind::ModelUnavailable {
                    provider: "unknown".to_string(),
                    model: "unknown".to_string(),
                })?;

            let text = synthesis.answer_text.clone();
            let _ = sink.emit(AgentEvent::MessageDelta { text }).await;
            (synthesis.answer_text, synthesis.llm_usage)
        };

        if let Some(synth) = synth_usage.as_ref() {
            ctx.aggregated_usage = Some(helpers::merge_usage(ctx.aggregated_usage.as_ref(), synth));
            ctx.request_count += 1;
        }

        let run_usage = helpers::build_run_usage(ctx.aggregated_usage.as_ref(), ctx.request_count);
        helpers::emit_usage(sink, run_usage.as_ref()).await;

        let citations = helpers::build_citations_from_tool_results(&ctx.all_tool_results);
        if !citations.is_empty() {
            let _ = sink.emit(AgentEvent::Citations {
                citations: citations.clone(),
            })
            .await;
        }

        let _ = sink.emit(AgentEvent::Done {
            final_message: Some(answer.clone()),
            usage: run_usage.as_ref().map(helpers::run_usage_to_agent_usage),
        })
        .await;

        let sources = helpers::build_sources_from_tool_results(&ctx.all_tool_results);

        let mut result = AgentRunResult {
            answer,
            citations,
            sources,
            usage: run_usage,
            tool_results: std::mem::take(&mut ctx.all_tool_results),
            iterations: ctx.iterations.clone(),
            total_tool_calls: ctx.total_tool_calls,
            final_decision: Some(FinalDecision::Synthesized),
            degrade_trace: ctx.content_guard_trace.clone(),
            ..Default::default()
        };
        result.decisions = ctx
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
        ctx: &mut RagContext,
        reason: DegradeReason,
    ) -> Result<AgentRunResult, AgentErrorKind> {
        let sink = ctx.sink.as_ref();
        let fallback = crate::chat::i18n::fallback::no_valid_retrieval_results(
            ctx.request.language.as_deref(),
        )
        .to_string();

        let _ = sink.emit(AgentEvent::MessageDelta {
            text: fallback.clone(),
        })
        .await;

        let run_usage = helpers::build_run_usage(ctx.aggregated_usage.as_ref(), ctx.request_count);
        helpers::emit_usage(sink, run_usage.as_ref()).await;

        let _ = sink.emit(AgentEvent::Done {
            final_message: Some(fallback.clone()),
            usage: run_usage.as_ref().map(helpers::run_usage_to_agent_usage),
        })
        .await;

        let degrade_trace = vec![common::DegradeTraceItem {
            stage: reason.as_stage().to_string(),
            reason: reason.message(),
            impact: "returned fallback message — no synthesized answer".to_string(),
        }];

        let mut result = AgentRunResult {
            answer: fallback,
            degrade_trace,
            usage: run_usage,
            tool_results: std::mem::take(&mut ctx.all_tool_results),
            iterations: ctx.iterations.clone(),
            total_tool_calls: ctx.total_tool_calls,
            final_decision: Some(FinalDecision::Degraded { reason }),
            ..Default::default()
        };
        result.decisions = ctx
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
}

// ---------------------------------------------------------------------------
// System prompt builders
// ---------------------------------------------------------------------------

fn build_eval_system_prompt(strategy: &str) -> String {
    let registry = PromptRegistry::standard_cached();
    let skill_body = registry
        .skill("rag-eval")
        .map(|s| s.system_prompt().to_string())
        .unwrap_or_default();

    let cap_registry = crate::agents::capability::CapabilityRegistry::standard_cached();
    let plan_tools = cap_registry.plan_tools(strategy);
    let tool_catalog = plan_tools
        .iter()
        .map(|t| format!("- {} (v{}): {}", t.id, t.version, t.description))
        .collect::<Vec<_>>()
        .join("\n");

    if tool_catalog.is_empty() {
        skill_body
    } else {
        format!("{skill_body}\n\n---\n\n## Available Tools for Replanning\n\n{tool_catalog}")
    }
}


// ---------------------------------------------------------------------------
// Helpers (migrated from mode_rag.rs)
// ---------------------------------------------------------------------------

fn inject_memory_context(ctx: &RagContext, prompt: &mut String) {
    if let Some(summary) = ctx.request.session_summary.as_deref().filter(|s| !s.trim().is_empty()) {
        prompt.push_str("\n\nSession summary:\n");
        prompt.push_str(summary.trim());
    }
    if let Some(prefs) = ctx.request.user_preferences.as_ref() {
        prompt.push_str("\n\nUser preferences:\n");
        prompt.push_str(&prefs.to_string());
    }
}

fn extract_sub_queries_from_plan_calls(plan_calls: &[ToolCall]) -> Vec<crate::rag_prompts::SubQueryItem> {
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

fn build_doc_index_directive_hint(tool_results: &[ToolResult]) -> Option<String> {
    let doc_index_result = tool_results
        .iter()
        .find(|r| r.tool == "doc_index" && r.status == ToolStatus::Ok)?;
    let has_index_lookup = tool_results
        .iter()
        .any(|r| r.tool == "index_lookup" && r.status == ToolStatus::Ok);
    if has_index_lookup {
        return None;
    }
    let data = doc_index_result.data.as_ref()?;
    let entries = data.as_array()?;
    let mut lines = vec!["Document index retrieved. Available sections:".to_string()];
    for entry in entries {
        if let Some(doc) = entry.as_object()
            && let Some(index) = doc.get("index").and_then(|v| v.as_array()) {
                for section in index {
                    if let Some(obj) = section.as_object() {
                        let title = obj
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Untitled");
                        let level = obj.get("level").and_then(|v| v.as_i64()).unwrap_or(1);
                        let chunks = obj
                            .get("chunk_ids")
                            .and_then(|v| v.as_array())
                            .map(|a| {
                                a.iter()
                                    .filter_map(|v| v.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                            .unwrap_or_default();
                        let indent = "  ".repeat((level as usize).saturating_sub(1));
                        lines.push(format!("{}- {} (chunks: {})", indent, title, chunks));
                    }
                }
            }
    }
    lines.push(
        "Call index_lookup with the appropriate doc_id and chunk_ids to fetch section content."
            .to_string(),
    );
    Some(lines.join("\n"))
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

fn is_rag_tool(name: &str) -> bool {
    crate::agents::progressive::rag_tool_catalog_cached()
        .iter()
        .any(|t| t.spec().name == name)
}

fn detect_format_skills(query: &str) -> Vec<&'static str> {
    let mut skills = Vec::new();
    let lower = query.to_lowercase();
    if lower.contains("ppt") || lower.contains("slide") || lower.contains("presentation") {
        skills.push("ppt-generation");
    }
    if lower.contains("html") || lower.contains("web page") || lower.contains("网页") {
        skills.push("html-renderer");
    }
    if lower.contains("teach") || lower.contains("explain") || lower.contains("tutorial") {
        skills.push("teaching");
    }
    skills
}

fn extract_chunks_from_tool_results(
    tool_results: &[common::ToolResult],
) -> Vec<common::RetrievedChunk> {
    tool_results
        .iter()
        .filter(|r| r.status == common::ToolStatus::Ok)
        .filter_map(|r| {
            let data = r.data.as_ref()?;
            let array = data.as_array()?;
            Some(
                array
                    .iter()
                    .filter_map(|v| serde_json::from_value::<common::RetrievedChunk>(v.clone()).ok())
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rag_state_ids() {
        assert_eq!(RagState::Plan.state_id(), "plan");
        assert_eq!(RagState::ExecuteRetrieve.state_id(), "execute_retrieve");
        assert_eq!(RagState::Evaluate.state_id(), "evaluate");
        assert_eq!(RagState::Answer.state_id(), "answer");
    }

    #[test]
    fn rag_state_kinds() {
        assert_eq!(RagState::Plan.state_kind(), StateKind::Plan);
        assert_eq!(RagState::ExecuteRetrieve.state_kind(), StateKind::Execute);
        assert_eq!(RagState::Evaluate.state_kind(), StateKind::Evaluate);
        assert_eq!(RagState::Answer.state_kind(), StateKind::Answer);
    }

    #[test]
    fn decision_label_coverage() {
        use crate::agents::react_loop::DegradeReason;
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
        assert_eq!(decision_label(&EvalAdvice::EscalateToSearch { reason: "r" }), "escalate_to_search");
        assert_eq!(decision_label(&EvalAdvice::FetchFullPage { reason: "r" }), "fetch_full_page");
    }

    #[test]
    fn detect_format_skills_ppt() {
        assert!(detect_format_skills("make a ppt").contains(&"ppt-generation"));
    }

    #[test]
    fn detect_format_skills_html() {
        assert!(detect_format_skills("render html").contains(&"html-renderer"));
    }

    #[test]
    fn detect_format_skills_teaching() {
        assert!(detect_format_skills("teach me rust").contains(&"teaching"));
    }

    #[test]
    fn is_rag_tool_true_for_dense() {
        assert!(is_rag_tool("dense_retrieval"));
    }

    #[test]
    fn is_rag_tool_false_for_calculator() {
        assert!(!is_rag_tool("calculator"));
    }

    #[test]
    fn build_eval_system_prompt_contains_tool_catalog() {
        let prompt = super::build_eval_system_prompt("rag");
        assert!(prompt.contains("Available Tools for Replanning"));
        // RAG strategy should have at least one tool
        assert!(!prompt.is_empty());
    }
}
