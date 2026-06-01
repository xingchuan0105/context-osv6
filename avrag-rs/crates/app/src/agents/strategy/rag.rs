//! RagStrategy — v6 state machine for RAG mode.
//!
//! RAG is multi-iteration with retrieval and (optional) replan:
//!   Plan → ExecuteRetrieve → Answer
//!            ↓ budget left, retriable
//!         Plan (loop)
//!
//! The legacy standalone `Evaluate` state has been merged into
//! `Answer` (now done by `Evidence Gate` as a pure-code check +
//! `grounded-answer` as a single LLM call that internally assesses
//! sufficiency). This avoids reading the same evidence twice and
//! removes the LLM-evaluate / LLM-answer split-brain problem.

use super::{AgentErrorKind, State, StateKind, StepOutcome, Strategy, StrategyContext};
use crate::agents::evaluator::{
    evaluate_rag_iteration, AccumulatedRagResults, EvalAdvice, EvaluationSignals,
};
use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::progressive::PromptRegistry;
use crate::agents::react_loop::{DegradeReason, LoopBudget};
use crate::agents::runtime::{AgentRequest, AgentRunResult, FinalDecision, IterationRecord};
use avrag_rag_core::{
    DefaultEvidenceGate, DegradeKind, EvidenceGate, EvidenceGateInput, EvidenceGateOutcome,
    FocusMode, ScoreBasedFocusMode,
};
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
    /// Answer: synthesize final response from accumulated evidence
    /// (with internal sufficiency check via `Evidence Gate` +
    /// `grounded-answer` self-assessment).
    Answer,
}

impl State for RagState {
    fn state_id(&self) -> &'static str {
        match self {
            RagState::Plan => "plan",
            RagState::ExecuteRetrieve => "execute_retrieve",
            RagState::Answer => "answer",
        }
    }

    fn state_kind(&self) -> StateKind {
        match self {
            RagState::Plan => StateKind::Plan,
            RagState::ExecuteRetrieve => StateKind::Execute,
            RagState::Answer => StateKind::Answer,
        }
    }

    fn to_observable(&self) -> serde_json::Value {
        match self {
            RagState::Plan => serde_json::json!({"state": "plan"}),
            RagState::ExecuteRetrieve => serde_json::json!({"state": "execute_retrieve"}),
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
    pub selected_writing_styles: Vec<String>,
    pub behavior_mode: Option<String>,
    pub iterations: Vec<IterationRecord>,

    // Accumulated
    pub aggregated_usage: Option<avrag_llm::LlmUsage>,
    pub request_count: u64,
    pub repository: Option<std::sync::Arc<avrag_storage_pg::PgAppRepository>>,
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
        // History is loaded on-demand via conversation_history_load tool.
        // Do not inject request.messages here.

        let query = request.query.clone();
        let chat_req = ChatRequest {
            query: query.clone(),
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
            iteration_params: RagIterationParams {
                query,
                directive: None,
                suggested_queries: Vec::new(),
            },
            current_plan_calls: None,
            current_plan_strategy: None,
            selected_skills: Vec::new(),
            selected_writing_styles: Vec::new(),
            behavior_mode: None,
            iterations: Vec::new(),
            aggregated_usage: None,
            request_count: 0,
            repository: None,
        })
    }

    pub fn with_repository(mut self, repository: Option<std::sync::Arc<avrag_storage_pg::PgAppRepository>>) -> Self {
        self.repository = repository;
        self
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
                    from: "ExecuteRetrieve".to_string(),
                    to: "Answer".to_string(),
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

        // Extract writing_styles and behavior_mode from planner output
        let (writing_styles, behavior_mode) = extract_rag_plan_metadata(&plan_response.content);

        match crate::rag_prompts::parse_rag_plan_decision(&plan_response.content, &ctx.chat_req) {
            Some((crate::rag_prompts::RagPlanDecision::Clarify(message), _)) => {
                ctx.selected_writing_styles = writing_styles;
                ctx.behavior_mode = behavior_mode;
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
                ctx.selected_writing_styles = writing_styles.clone();
                ctx.behavior_mode = behavior_mode.clone();
                let _ = ctx
                    .sink
                    .emit(AgentEvent::PlanDecision {
                        selected_tools: vec![],
                        selected_skills: skills,
                        selected_writing_styles: writing_styles.clone(),
                        behavior_mode: behavior_mode.clone(),
                        reasoning: format!("plan strategy: {:?}", strategy),
                    })
                    .await;
                Ok(StepOutcome::Next(Box::new(RagState::ExecuteRetrieve)))
            }
            Some((crate::rag_prompts::RagPlanDecision::ToolCalls(calls), skills)) => {
                ctx.current_plan_strategy = None;
                ctx.current_plan_calls = Some(calls.clone());
                ctx.selected_skills = skills.clone();
                ctx.selected_writing_styles = writing_styles.clone();
                ctx.behavior_mode = behavior_mode.clone();
                let _ = ctx
                    .sink
                    .emit(AgentEvent::PlanDecision {
                        selected_tools: calls.clone(),
                        selected_skills: skills,
                        selected_writing_styles: writing_styles.clone(),
                        behavior_mode: behavior_mode.clone(),
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

        // Fallback: if planner produced no core retrieval calls, auto-inject
        // a default dense_retrieval so the pipeline always attempts evidence
        // gathering (prevents silent degrade when pg_repo is unavailable).
        let has_core_retrieval = rag_calls.iter().any(|call| {
            matches!(
                call.tool.as_str(),
                "dense_retrieval" | "lexical_retrieval" | "graph_retrieval" | "index_lookup"
            )
        });
        let mut rag_calls = rag_calls;
        if !has_core_retrieval && !ctx.request.doc_scope.is_empty() {
            tracing::warn!(
                "planner produced no core retrieval calls — injecting default dense_retrieval"
            );
            rag_calls.push(common::ToolCall {
                tool: "dense_retrieval".to_string(),
                version: "1.0".to_string(),
                args: serde_json::json!({
                    "queries": vec![ctx.chat_req.query.clone()],
                    "modality": "text",
                    "top_k": 10,
                }),
            });
        }

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

        // Budget exhausted: short-circuit to Answer or Degrade.
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
                    decision: decision.clone(),
                    reasoning: "budget exhausted — forced decision".to_string(),
                })
                .await;
            if ctx.accumulated.is_empty() {
                return self.finalize_degrade(ctx, DegradeReason::NoResultsAfterAllFallbacks).await
                    .map(StepOutcome::Terminate);
            }
            return Ok(StepOutcome::Next(Box::new(RagState::Answer)));
        }

        // --- Evidence Gate (Step 1) ---
        // Pure-code check on retrieval metadata. Replaces the legacy
        // LLM-driven `Evaluate` state. The grounded-answer prompt will
        // perform its own internal sufficiency assessment on the same
        // evidence, but this gate is a fast first-pass for structural
        // problems (zero recall, context overflow, score floor, topic
        // mismatch).
        let top_score = signals.max_score;
        let chunk_count = signals.recall_count;
        let score_variance = compute_score_variance(&ctx.accumulated);
        let context_usage_ratio = estimate_context_usage_ratio(ctx);

        let gate_input = EvidenceGateInput {
            chunk_count,
            top_score,
            score_variance,
            context_usage_ratio,
            doc_metadata_themes: Vec::new(), // TODO: wire from doc_metadata tool
            query_themes: extract_query_themes(&ctx.iteration_params.query),
        };

        let gate = DefaultEvidenceGate::default();
        let gate_decision = gate.check(&gate_input);
        let gate_label = match &gate_decision {
            EvidenceGateOutcome::Pass => "pass",
            EvidenceGateOutcome::NeedsFocus { .. } => "needs_focus",
            EvidenceGateOutcome::Degrade(_) => "degrade",
        };
        let _ = ctx
            .sink
            .emit(AgentEvent::Evaluation {
                signals: serde_json::to_value(&signals).ok(),
                decision: gate_label.to_string(),
                reasoning: format!("evidence_gate: {:?}", gate_decision),
            })
            .await;

        // Commit iteration record with the gate decision.
        let decision = gate_label.to_string();
        ctx.iterations.push(IterationRecord {
            iteration: iteration_idx,
            plan: plan_snapshot,
            signals: signals.clone(),
            decision: decision.clone(),
            elapsed_ms,
            llm_evaluation: Some(serde_json::to_value(&gate_decision).unwrap_or(serde_json::Value::Null)),
            usage: None,
        });

        match gate_decision {
            EvidenceGateOutcome::Pass => {
                // Pass: proceed to grounded answer with current evidence.
                Ok(StepOutcome::Next(Box::new(RagState::Answer)))
            }
            EvidenceGateOutcome::NeedsFocus { .. } => {
                // Step 4: focus-mode compression. Trim/cap the accumulated
                // chunks before they enter the Answer phase.
                let mut items: Vec<(common::AnswerContextChunk, f32)> = Vec::new();
                let chunk_refs = ctx.accumulated.all_chunks();
                let scores = ctx.accumulated.all_scores();
                for (chunk, score) in chunk_refs.into_iter().zip(scores.into_iter()) {
                    items.push((chunk.clone(), score));
                }
                let focus = ScoreBasedFocusMode::default();
                if let Ok(compressed) = focus.compress(
                    &items,
                    &ctx.iteration_params.query,
                    ctx.accumulated.unique_chunk_count(),
                ) {
                    tracing::info!(
                        original = ctx.accumulated.unique_chunk_count(),
                        kept = compressed.len(),
                        trimmed = compressed.iter().filter(|c| c.trimmed).count(),
                        "focus mode compressed evidence"
                    );
                    // Replace accumulated evidence with focused version.
                    ctx.accumulated.clear();
                    for c in &compressed {
                        ctx.accumulated.merge_iteration(
                            std::iter::once((c.chunk.clone(), c.score)),
                            iteration_idx,
                        );
                    }
                } else {
                    tracing::warn!("focus mode compression failed; using raw evidence");
                }
                Ok(StepOutcome::Next(Box::new(RagState::Answer)))
            }
            EvidenceGateOutcome::Degrade(kind) => {
                let reason = evidence_gate_kind_to_degrade_reason(&kind);
                self.finalize_degrade(ctx, reason).await.map(StepOutcome::Terminate)
            }
        }
    }

    // --- Answer step ---

    async fn step_answer(&self, ctx: &mut RagContext) -> Result<StepOutcome, AgentErrorKind> {
        ctx.check_cancelled()?;

        // Compute selected format skills: prefer explicit selection, fall back to keyword detection
        let mut selected_format_skills: Vec<String> = if !ctx.selected_skills.is_empty() {
            ctx.selected_skills.clone()
        } else {
            crate::agents::strategy::prompts::detect_format_skills(&ctx.request.query)
                .iter()
                .map(|s| s.to_string())
                .collect()
        };
        // Always honor explicit format_hint regardless of planner output
        if let Some(ref hint) = ctx.request.format_hint {
            let lower = hint.to_lowercase();
            let skill = if lower.contains("html") || lower.contains("web") {
                Some("html-renderer")
            } else if lower.contains("ppt") || lower.contains("slide") || lower.contains("presentation") {
                Some("presentation-html")
            } else if lower.contains("teach") || lower.contains("tutorial") || lower.contains("step") {
                Some("step-by-step-tutor")
            } else {
                None
            };
            if let Some(s) = skill {
                let s = s.to_string();
                if !selected_format_skills.contains(&s) {
                    selected_format_skills.push(s);
                }
            }
        }
        let mut system_prompt = crate::agents::strategy::prompts::build_answer_system_prompt(
            crate::agents::strategy::prompts::rag::ANSWER_SKILL_ID,
            "rag",
            &selected_format_skills,
            &ctx.selected_writing_styles,
        );

        // Inject behavior mode skill if active
        if let Some(behavior_skill) = crate::agents::strategy::prompts::load_behavior_mode_skill(ctx.behavior_mode.as_deref()) {
            system_prompt.push_str("\n\n---\n\n");
            system_prompt.push_str(&behavior_skill);
        }

        // Emit debug trace for E2E validation of format skill injection
        let _ = ctx
            .sink
            .emit(crate::agents::events::AgentEvent::DebugTrace {
                kind: "answer.format_skills".to_string(),
                payload: serde_json::json!({
                    "selected_format_skills": selected_format_skills,
                    "system_prompt_len": system_prompt.len(),
                    "contains_html_renderer": system_prompt.contains("html-renderer"),
                }),
            })
            .await;

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

/// **Deprecated** since Step 2 (2026-06). The legacy LLM-driven
/// `evaluate` state was removed in favour of the pure-code
/// `Evidence Gate` (see `avrag_rag_core::evidence_gate`). This
/// function is retained for unit-test coverage of the
/// `retrieval-coverage-eval` skill body and the RAG tool catalog.
/// New code should not call it.
#[allow(dead_code)]
fn build_eval_system_prompt(strategy: &str) -> String {
    let registry = PromptRegistry::standard_cached();
    let (skill_body, schema_ref) = registry
        .skill("retrieval-coverage-eval")
        .map(|s| {
            let body = s.system_prompt().to_string();
            let schema = s.references().get("schema.md").cloned();
            (body, schema)
        })
        .unwrap_or_default();

    let cap_registry = crate::agents::capability::CapabilityRegistry::standard_cached();
    let plan_tools = cap_registry.plan_tools(strategy);
    let tool_catalog = plan_tools
        .iter()
        .map(|t| format!("- {} (v{}): {}", t.id, t.version, t.description))
        .collect::<Vec<_>>()
        .join("\n");

    let mut parts = vec![skill_body];
    if let Some(schema) = schema_ref {
        parts.push(format!("## Output Schema\n\n{schema}"));
    }
    if !tool_catalog.is_empty() {
        parts.push(format!("## Available Tools for Replanning\n\n{tool_catalog}"));
    }
    parts.join("\n\n---\n\n")
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

/// Map an `Evidence Gate` `DegradeKind` to the app's strong `DegradeReason` enum.
///
/// The mapping prefers existing concrete variants. `TopicMismatch` and
/// `LowRelevance` fall back to `Other(_)` for now; they will get their
/// own dedicated variants in a follow-up cleanup.
fn evidence_gate_kind_to_degrade_reason(kind: &DegradeKind) -> DegradeReason {
    match kind {
        DegradeKind::NoResults => DegradeReason::NoResultsAfterAllFallbacks,
        DegradeKind::ContextBudgetTight => DegradeReason::BudgetExhausted,
        DegradeKind::LowRelevance => DegradeReason::NoResultsAfterAllFallbacks,
        DegradeKind::TopicMismatch => {
            DegradeReason::Other("evidence_topic_mismatch".to_string())
        }
    }
}

/// Compute variance of accumulated chunk scores.
///
/// Returns 0.0 when there is fewer than two chunks (variance undefined).
pub fn compute_score_variance(accumulated: &AccumulatedRagResults) -> f32 {
    // AccumulatedRagResults keeps a flat list of (chunk, score) pairs.
    let scores = accumulated.all_scores();
    if scores.len() < 2 {
        return 0.0;
    }
    let mean: f32 = scores.iter().sum::<f32>() / scores.len() as f32;
    let var: f32 = scores
        .iter()
        .map(|s| (s - mean).powi(2))
        .sum::<f32>()
        / scores.len() as f32;
    var
}

/// Estimate context usage ratio in `[0.0, 1.0]` from accumulated evidence.
///
/// Heuristic: 1 char ≈ 0.25 token; a 200k-token context is the LLM ceiling.
pub fn estimate_context_usage_ratio(ctx: &RagContext) -> f32 {
    const CTX_CEILING_TOKENS: f32 = 200_000.0;
    let chars: usize = ctx
        .accumulated
        .all_chunks()
        .iter()
        .map(|c| c.text.len())
        .sum();
    let est_tokens = (chars as f32) * 0.25;
    (est_tokens / CTX_CEILING_TOKENS).min(1.0)
}

/// Extract query themes (lowercased, non-trivial tokens) for the
/// Evidence Gate's topic-mismatch check.
pub fn extract_query_themes(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(|s| {
            s.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|s| s.len() >= 3)
        .take(8)
        .collect()
}

fn is_rag_tool(name: &str) -> bool {
    crate::agents::progressive::rag_tool_catalog_cached()
        .iter()
        .any(|t| t.spec().name == name)
}

/// Extract writing_styles and behavior_mode from planner JSON output.
fn extract_rag_plan_metadata(raw: &str) -> (Vec<String>, Option<String>) {
    let json = raw.trim();
    let start = json.find('{');
    let end = json.rfind('}');
    let json_str = match (start, end) {
        (Some(s), Some(e)) if s <= e => &json[s..=e],
        _ => json,
    };
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) {
        let writing_styles = value
            .get("writing_styles")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let behavior_mode = value
            .get("behavior_mode")
            .and_then(|v| v.as_str())
            .map(String::from);
        (writing_styles, behavior_mode)
    } else {
        (Vec::new(), None)
    }
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
        assert_eq!(RagState::Answer.state_id(), "answer");
    }

    #[test]
    fn rag_state_kinds() {
        assert_eq!(RagState::Plan.state_kind(), StateKind::Plan);
        assert_eq!(RagState::ExecuteRetrieve.state_kind(), StateKind::Execute);
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

    #[test]
    fn rag_eval_prompt_contains_schema() {
        let prompt = super::build_eval_system_prompt("rag");
        assert!(
            prompt.contains("## Output Schema"),
            "RAG eval prompt should inject reference/schema.md"
        );
        assert!(
            prompt.contains("dimensions"),
            "Schema content should be present"
        );
    }
}
