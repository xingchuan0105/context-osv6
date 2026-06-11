use std::sync::Arc;

pub mod answer_contract;
pub mod assembler;
pub mod config;
pub mod disclosure_plan;
pub mod exit_policy;
pub mod fallback;
pub mod hooks;
pub mod iteration;
pub mod message_queue;
pub mod optimizer;
pub mod parse;
pub mod query_normalize;
pub mod reasoning_emit;
pub mod skill_request;
pub mod skills;
pub mod synthesis;
pub mod telemetry;

use crate::agents::capability::CapabilityRegistry;
use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::react_loop::DegradeReason;
use crate::agents::r#loop::optimizer::{IterationProgress, LoopOptimizer};
use crate::agents::runtime::{
    AgentRequest, AgentRunResult, AgentRunUsage, BudgetUsage, EvaluationSignals, FinalDecision,
    IterationRecord,
};
use iteration::{IterationControl, IterationState};
use assembler::{ContextAssembler, DisclosedState};
use avrag_llm::{ChatMessage, LlmClient, LlmUsage};
use common::{AppError, ToolResult};
use config::ModeConfig;
use exit_policy::{
    PostLoopAction, SynthesisGate, decide_synthesis_gate, degraded_no_evidence_answer,
    has_retrieval_observation, post_fallback_gate,
};
use hooks::{LoopContext, LoopHooks, StandardLoopHooks};
use query_normalize::normalize_query;
use synthesis::SynthesisPhase;
use telemetry::ReActIterationRecord;

pub(crate) fn merge_request_doc_scope(call: &mut common::ToolCall, doc_scope: &[String]) {
    if doc_scope.is_empty() {
        return;
    }
    let Some(args) = call.args.as_object_mut() else {
        return;
    };
    let scope_empty = args
        .get("doc_scope")
        .and_then(|value| value.as_array())
        .is_none_or(|items| items.is_empty());
    if scope_empty {
        args.insert("doc_scope".to_string(), serde_json::json!(doc_scope));
    }
}

pub(crate) async fn dispatch_rag_tool(
    runtime: &avrag_rag_core::RagRuntime,
    auth: &avrag_auth::AuthContext,
    call: &common::ToolCall,
    doc_scope: &[String],
) -> ToolResult {
    let mut call = call.clone();
    if call.tool == "dense_retrieval" || call.tool == "lexical_retrieval" {
        merge_request_doc_scope(&mut call, doc_scope);
    }
    avrag_rag_core::runtime::tools::dispatch(runtime, auth, &call).await
}

pub struct ReActLoop {
    llm: Arc<LlmClient>,
    skill_registry: Arc<CapabilityRegistry>,
    rag_runtime: Option<Arc<avrag_rag_core::RagRuntime>>,
    search_executor: Option<Arc<dyn avrag_search::SearchProvider>>,
    pg_repo: Option<Arc<avrag_storage_pg::PgAppRepository>>,
    code_interpreter: Arc<std::sync::Mutex<Option<avrag_code_interpreter::CodeInterpreter>>>,
}

impl ReActLoop {
    pub fn new(llm: Arc<LlmClient>, skill_registry: Arc<CapabilityRegistry>) -> Self {
        Self {
            llm,
            skill_registry,
            rag_runtime: None,
            search_executor: None,
            pg_repo: None,
            code_interpreter: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub fn with_pg_repo(
        mut self,
        pg_repo: Option<Arc<avrag_storage_pg::PgAppRepository>>,
    ) -> Self {
        self.pg_repo = pg_repo;
        self
    }

    fn effective_pg_repo(&self) -> Option<Arc<avrag_storage_pg::PgAppRepository>> {
        self.pg_repo
            .clone()
            .or_else(|| self.rag_runtime.as_ref().and_then(|r| r.pg_repo()))
    }

    pub fn with_rag_runtime(mut self, runtime: Option<Arc<avrag_rag_core::RagRuntime>>) -> Self {
        self.rag_runtime = runtime;
        self
    }

    pub fn with_search_executor(
        mut self,
        executor: Option<Arc<dyn avrag_search::SearchProvider>>,
    ) -> Self {
        self.search_executor = executor;
        self
    }

    pub async fn run(
        &self,
        mode: &ModeConfig,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, AppError> {
        let start_time = std::time::Instant::now();
        let cancel = request.cancellation_token.clone().unwrap_or_default();
        if cancel.is_cancelled() {
            return Err(crate::agents::react_loop::cancellation_error());
        }
        let loop_exit = mode.loop_exit_for_mode();
        let hooks = StandardLoopHooks::default();

        let norm = normalize_query(&self.llm, mode, &request).await?;
        if let Some(clarify) = norm.clarify_answer {
            let _ = sink
                .emit(AgentEvent::MessageDelta {
                    text: clarify.clone(),
                })
                .await;
            let _ = sink
                .emit(AgentEvent::Done {
                    final_message: Some(clarify.clone()),
                    usage: None,
                })
                .await;
            return Ok(AgentRunResult {
                answer: clarify.clone(),
                final_decision: Some(FinalDecision::Clarified { question: clarify }),
                ..AgentRunResult::default()
            });
        }

        let request = request.with_resolved_query(norm.resolved_query.clone(), norm.meta);
        let slots: Vec<String> = request
            .query_resolution
            .as_ref()
            .map(|meta| {
                meta.slots
                    .iter()
                    .map(|s| {
                        serde_json::to_string(s)
                            .unwrap_or_default()
                            .trim_matches('"')
                            .to_string()
                    })
                    .collect()
            })
            .unwrap_or_default();
        let _ = sink
            .emit(AgentEvent::QueryResolved {
                raw: request.query.clone(),
                resolved: request.effective_query().to_string(),
                slots,
            })
            .await;

        let mut messages: Vec<ChatMessage> = Vec::new();

        // Cross-mode history injection: ADR-0006 requires prior user queries
        // to be injected with a [prior_user_query] prefix so the ReAct loop
        // can see conversational context without leaking assistant/tool turns
        // from a different mode.
        for turn in &request.messages {
            if turn.role == "user" {
                let content = format!("[prior_user_query] {}", turn.content);
                messages.push(ChatMessage::user(&content));
            }
        }

        let loop_user_query = if mode.id == "rag" || mode.id == "search" {
            request.effective_query().to_string()
        } else {
            request.query.clone()
        };
        messages.push(ChatMessage::user(&loop_user_query));

        // base_message_count = conversation history + user query.
        // ReAct steps are appended after this. Truncation must never touch
        // the base conversation — only intermediate ReAct rounds.
        let base_message_count = messages.len();

        let max_iterations = request
            .max_iterations
            .unwrap_or_else(|| {
                mode.budget
                    .resolve_max_iterations(request.metadata.get("user_tier"))
            })
            .max(1);

        let auth: avrag_auth::AuthContext = serde_json::from_value(request.auth_context.clone())
            .map_err(|e| AppError::internal(format!("invalid auth context: {e}")))?;

        let mut state = IterationState {
            messages,
            disclosed: DisclosedState::default(),
            tool_results: Vec::new(),
            progress: IterationProgress::new(),
            total_tool_calls: 0,
            consecutive_sandbox_errors: 0,
            reasoning_acc: String::new(),
        };
        let mut iteration: u8 = 0;
        let mut telemetry_records: Vec<ReActIterationRecord> = vec![];
        let mut total_usage = LlmUsage::zeroed();
        let mut direct_answer: Option<String> = None;
        let optimizer = LoopOptimizer::new();

        loop {
            if cancel.is_cancelled() {
                break;
            }

            if iteration >= max_iterations {
                let disclosed_skills: Vec<String> = state
                    .disclosed
                    .disclosed_skill_ids
                    .iter()
                    .cloned()
                    .collect();
                reasoning_emit::emit_evaluation_telemetry(
                    sink,
                    iteration,
                    "budget_exhausted",
                    "iteration budget exhausted, evaluating exit policy",
                    &disclosed_skills,
                    "budget_exhausted",
                )
                .await;
                let _ = sink
                    .emit(AgentEvent::Activity {
                        stage: "budget_exhausted".to_string(),
                        message: "iteration budget exhausted, evaluating exit policy".to_string(),
                    })
                    .await;
                break;
            }

            let has_evidence =
                has_retrieval_observation(&state.messages, &state.tool_results, mode);

            let _ = sink
                .emit(AgentEvent::TurnStart {
                    iteration,
                    phase: "retrieve".to_string(),
                })
                .await;

            let outcome = self
                .run_iteration(
                    iteration,
                    max_iterations,
                    mode,
                    &request,
                    &auth,
                    &loop_exit,
                    &mut state,
                    &mut total_usage,
                    &optimizer,
                    sink,
                )
                .await?;

            if !outcome.sandbox_break {
                if let Some(record) = outcome.record {
                    let exit_reason = record.exit_reason.clone();
                    let observation_preview = record.observation_preview.clone();
                    let disclosed_skills = record.disclosed_skills.clone();
                    reasoning_emit::emit_evaluation_telemetry(
                        sink,
                        iteration,
                        &exit_reason,
                        &observation_preview,
                        &disclosed_skills,
                        &exit_reason,
                    )
                    .await;
                    let _ = sink
                        .emit(AgentEvent::TurnEnd {
                            iteration,
                            exit_reason,
                        })
                        .await;
                    telemetry_records.push(record);
                }
            }

            match outcome.control {
                IterationControl::Continue => {
                    iteration += 1;
                }
                IterationControl::BreakToSynthesis { .. } => break,
                IterationControl::DirectAnswer { content } => {
                    direct_answer = Some(content);
                    break;
                }
            }

            hooks.transform_context(
                &mut state.messages,
                &LoopContext {
                    mode,
                    request: &request,
                    iteration,
                    phase: assembler::LoopPhase::Retrieve,
                    has_retrieval_observation: has_evidence,
                    base_message_count,
                },
            );

            let _ = sink
                .emit(AgentEvent::BudgetTick {
                    current: iteration,
                    max: max_iterations,
                })
                .await;
        }

        let mut messages = state.messages;
        let mut disclosed_state = state.disclosed;
        let mut collected_tool_results = state.tool_results;
        let total_tool_calls = state.total_tool_calls;
        let reasoning_summary_acc = state.reasoning_acc;

        if cancel.is_cancelled() {
            return Err(crate::agents::react_loop::cancellation_error());
        }

        let mut has_evidence = has_retrieval_observation(&messages, &collected_tool_results, mode);
        let retrieval_query = request.effective_query().to_string();

        match decide_synthesis_gate(&loop_exit, has_evidence, direct_answer.as_deref(), &collected_tool_results, &retrieval_query) {
            SynthesisGate::SkipSynthesisUseDirect(answer) => {
                let disclosed_skills: Vec<String> = disclosed_state
                    .disclosed_skill_ids
                    .iter()
                    .cloned()
                    .collect();
                let observation_preview = truncate_preview(&answer, 200);
                reasoning_emit::emit_evaluation_telemetry(
                    sink,
                    iteration,
                    "skip_synthesis_direct",
                    &observation_preview,
                    &disclosed_skills,
                    "skip_synthesis_direct",
                )
                .await;
                let _ = sink
                    .emit(AgentEvent::MessageDelta {
                        text: answer.clone(),
                    })
                    .await;
                let _ = sink
                    .emit(AgentEvent::Done {
                        final_message: Some(answer.clone()),
                        usage: None,
                    })
                    .await;
                return self
                    .finish_run(
                        sink,
                        answer,
                        &request,
                        &collected_tool_results,
                        &telemetry_records,
                        &total_usage,
                        &reasoning_summary_acc,
                        iteration,
                        max_iterations,
                        total_tool_calls,
                        start_time,
                        Some(FinalDecision::DirectAnswer),
                    )
                    .await;
            }
            SynthesisGate::RunFallbackThenCheck => {
                self.run_auto_fallback(
                    mode,
                    &request,
                    &auth,
                    &retrieval_query,
                    &mut messages,
                    &mut collected_tool_results,
                    sink,
                )
                .await?;
                has_evidence = has_retrieval_observation(&messages, &collected_tool_results, mode);
                if post_fallback_gate(&loop_exit, has_evidence)
                    == PostLoopAction::DegradedNoEvidence
                {
                    let answer = degraded_no_evidence_answer(&mode.id);
                    let disclosed_skills: Vec<String> = disclosed_state
                        .disclosed_skill_ids
                        .iter()
                        .cloned()
                        .collect();
                    let observation_preview = truncate_preview(&answer, 200);
                    reasoning_emit::emit_evaluation_telemetry(
                        sink,
                        iteration,
                        "degraded_no_evidence",
                        &observation_preview,
                        &disclosed_skills,
                        "degraded_no_evidence",
                    )
                    .await;
                    let _ = sink
                        .emit(AgentEvent::Activity {
                            stage: "degraded_no_evidence".to_string(),
                            message: answer.clone(),
                        })
                        .await;
                    let _ = sink
                        .emit(AgentEvent::MessageDelta {
                            text: answer.clone(),
                        })
                        .await;
                    let _ = sink
                        .emit(AgentEvent::Done {
                            final_message: Some(answer.clone()),
                            usage: None,
                        })
                        .await;
                    let mut result = self.build_run_result(
                        answer,
                        &request,
                        &collected_tool_results,
                        &telemetry_records,
                        &total_usage,
                        &reasoning_summary_acc,
                        iteration,
                        max_iterations,
                        total_tool_calls,
                        start_time,
                        Some(FinalDecision::Degraded {
                            reason: crate::agents::react_loop::DegradeReason::NoResultsAfterAllFallbacks,
                        }),
                    );
                    result.degrade_trace.push(common::DegradeTraceItem {
                        stage: "degraded_no_evidence".to_string(),
                        reason: DegradeReason::NoRetrievalEvidence,
                        impact: "Answer withheld; synthesis skipped".to_string(),
                    });
                    self.emit_run_citations(sink, &result.citations).await;
                    return Ok(result);
                }
            }
            SynthesisGate::DegradedNoEvidence => {
                let answer = degraded_no_evidence_answer(&mode.id);
                let disclosed_skills: Vec<String> = disclosed_state
                    .disclosed_skill_ids
                    .iter()
                    .cloned()
                    .collect();
                let observation_preview = truncate_preview(&answer, 200);
                reasoning_emit::emit_evaluation_telemetry(
                    sink,
                    iteration,
                    "degraded_no_evidence",
                    &observation_preview,
                    &disclosed_skills,
                    "degraded_no_evidence",
                )
                .await;
                let _ = sink
                    .emit(AgentEvent::MessageDelta {
                        text: answer.clone(),
                    })
                    .await;
                let _ = sink
                    .emit(AgentEvent::Done {
                        final_message: Some(answer.clone()),
                        usage: None,
                    })
                    .await;
                return self
                    .finish_run(
                        sink,
                        answer,
                        &request,
                        &collected_tool_results,
                        &telemetry_records,
                        &total_usage,
                        &reasoning_summary_acc,
                        iteration,
                        max_iterations,
                        total_tool_calls,
                        start_time,
                        Some(FinalDecision::Degraded {
                            reason: crate::agents::react_loop::DegradeReason::NoResultsAfterAllFallbacks,
                        }),
                    )
                    .await;
            }
            SynthesisGate::EnterSynthesis => {}
        }

        let synthesis_ctx = ContextAssembler::assemble_synthesis(
            mode,
            &request,
            &self.skill_registry,
            &mut disclosed_state,
        );
        reasoning_emit::emit_prompt_snapshot(
            sink,
            "synthesis",
            iteration,
            &synthesis_ctx,
            &disclosed_state,
        )
        .await;
        reasoning_emit::emit_plan_decision_telemetry(
            sink,
            "synthesis",
            iteration,
            &synthesis_ctx,
            &disclosed_state,
        )
        .await;

        let synthesis = SynthesisPhase;
        let final_answer = synthesis
            .run(
                &self.llm,
                &synthesis_ctx,
                mode,
                &messages,
                &collected_tool_results,
                sink,
                &cancel,
            )
            .await?;

        let disclosed_skills: Vec<String> =
            disclosed_state.disclosed_skill_ids.iter().cloned().collect();
        let observation_preview = truncate_preview(&final_answer, 200);
        reasoning_emit::emit_evaluation_telemetry(
            sink,
            iteration,
            "synthesized",
            &observation_preview,
            &disclosed_skills,
            "synthesized",
        )
        .await;

        self.finish_run(
            sink,
            final_answer,
            &request,
            &collected_tool_results,
            &telemetry_records,
            &total_usage,
            &reasoning_summary_acc,
            iteration,
            max_iterations,
            total_tool_calls,
            start_time,
            Some(FinalDecision::Synthesized),
        )
        .await
    }

    async fn run_auto_fallback(
        &self,
        mode: &ModeConfig,
        request: &AgentRequest,
        auth: &avrag_auth::AuthContext,
        retrieval_query: &str,
        messages: &mut Vec<ChatMessage>,
        collected_tool_results: &mut Vec<ToolResult>,
        sink: &dyn AgentEventSink,
    ) -> Result<(), AppError> {
        let Some(fallback) = &mode.auto_fallback else {
            return Ok(());
        };
        if !fallback.enabled {
            return Ok(());
        }

        let _ = sink
            .emit(AgentEvent::Activity {
                stage: "auto_fallback".to_string(),
                message: format!("Running fallback: {}", fallback.tool_id),
            })
            .await;

        match fallback.tool_id.as_str() {
            "dense_retrieval" => {
                if let Some(runtime) = &self.rag_runtime {
                    let args = serde_json::to_value(common::DenseRetrievalArgs {
                        queries: vec![retrieval_query.to_string()],
                        modality: common::DenseRetrievalModality::Both,
                        top_k: fallback.top_k as usize,
                        doc_scope: request.doc_scope.clone(),
                    })
                    .map_err(|e| AppError::internal(format!("serialize fallback args: {e}")))?;
                    let result = fallback::inject_fallback_observation(
                        runtime,
                        auth,
                        args,
                        &fallback.tool_id,
                        messages,
                    )
                    .await;
                    collected_tool_results.push(result);
                }
            }
            "lexical_retrieval" => {
                if let Some(runtime) = &self.rag_runtime {
                    let args = serde_json::to_value(common::LexicalRetrievalArgs {
                        terms: retrieval_query
                            .split_whitespace()
                            .map(ToOwned::to_owned)
                            .collect(),
                        top_k: fallback.top_k as usize,
                        doc_scope: request.doc_scope.clone(),
                    })
                    .map_err(|e| AppError::internal(format!("serialize fallback args: {e}")))?;
                    let result = fallback::inject_fallback_observation(
                        runtime,
                        auth,
                        args,
                        &fallback.tool_id,
                        messages,
                    )
                    .await;
                    collected_tool_results.push(result);
                }
            }
            "graph_retrieval" => {
                if let Some(runtime) = &self.rag_runtime {
                    let args = serde_json::to_value(common::GraphRetrievalArgs {
                        graph_hints: Vec::new(),
                        placeholder_triplets: Vec::new(),
                        relation_limit: 20,
                        supporting_chunk_limit: 10,
                        hop_limit: 1,
                        fan_out_limit: 10,
                        query: Some(retrieval_query.to_string()),
                        doc_scope: request.doc_scope.clone(),
                    })
                    .map_err(|e| AppError::internal(format!("serialize fallback args: {e}")))?;
                    let result = fallback::inject_fallback_observation(
                        runtime,
                        auth,
                        args,
                        &fallback.tool_id,
                        messages,
                    )
                    .await;
                    collected_tool_results.push(result);
                }
            }
            "web_search" => {
                if let Some(executor) = &self.search_executor {
                    let v = fallback.vertical.as_deref().unwrap_or("web");
                    match executor.execute_search(retrieval_query, Some(v)).await {
                        Ok(response) => {
                            let text = serde_json::to_string_pretty(&response)
                                .unwrap_or_else(|_| "search succeeded".to_string());
                            messages
                                .push(ChatMessage::system(format!("自动兜底搜索结果:\n{text}")));
                            collected_tool_results.push(ToolResult {
                                tool: "web_search".to_string(),
                                version: "1.0".to_string(),
                                status: common::ToolStatus::Ok,
                                data: Some(serde_json::to_value(&response).unwrap_or_default()),
                                trace: None,
                            });
                        }
                        Err(e) => {
                            messages.push(ChatMessage::system(format!("[fallback failed: {e}]")));
                        }
                    }
                }
            }
            other => {
                let _ = sink
                    .emit(AgentEvent::Activity {
                        stage: "fallback_skipped".to_string(),
                        message: format!("unknown fallback tool_id: {other}"),
                    })
                    .await;
            }
        }
        Ok(())
    }

    async fn emit_run_citations(&self, sink: &dyn AgentEventSink, citations: &[common::Citation]) {
        if !citations.is_empty() {
            let _ = sink
                .emit(AgentEvent::Citations {
                    citations: citations.to_vec(),
                })
                .await;
        }
    }

    async fn finish_run(
        &self,
        sink: &dyn AgentEventSink,
        final_answer: String,
        request: &AgentRequest,
        collected_tool_results: &[ToolResult],
        telemetry_records: &[ReActIterationRecord],
        total_usage: &LlmUsage,
        reasoning_summary_acc: &str,
        iteration: u8,
        max_iterations: u8,
        total_tool_calls: u32,
        start_time: std::time::Instant,
        final_decision: Option<FinalDecision>,
    ) -> Result<AgentRunResult, AppError> {
        let result = self.build_run_result(
            final_answer,
            request,
            collected_tool_results,
            telemetry_records,
            total_usage,
            reasoning_summary_acc,
            iteration,
            max_iterations,
            total_tool_calls,
            start_time,
            final_decision,
        );
        self.emit_run_citations(sink, &result.citations).await;
        Ok(result)
    }

    fn build_run_result(
        &self,
        final_answer: String,
        request: &AgentRequest,
        collected_tool_results: &[ToolResult],
        telemetry_records: &[ReActIterationRecord],
        total_usage: &LlmUsage,
        reasoning_summary_acc: &str,
        iteration: u8,
        max_iterations: u8,
        total_tool_calls: u32,
        start_time: std::time::Instant,
        final_decision: Option<FinalDecision>,
    ) -> AgentRunResult {
        let total_elapsed_ms = start_time.elapsed().as_millis() as u64;
        let citations = crate::agents::unified::helpers::build_all_citations_from_tool_results(
            collected_tool_results,
        );
        let citations = crate::agents::unified::helpers::filter_citations_for_mode(
            &request.kind.as_canonical_str(),
            &final_answer,
            citations,
        );
        let sources = crate::agents::unified::helpers::build_sources_from_tool_results(
            collected_tool_results,
        );
        let degrade_trace = crate::agents::unified::helpers::degrade_trace_from_tool_results(
            collected_tool_results,
        );

        AgentRunResult {
            answer: final_answer,
            answer_blocks: Vec::new(),
            citations,
            sources,
            reasoning_summary: if reasoning_summary_acc.is_empty() {
                None
            } else {
                Some(reasoning_summary_acc.to_string())
            },
            degrade_trace,
            usage: Some(AgentRunUsage {
                provider: if total_usage.provider.is_empty() {
                    self.llm.config.provider_name()
                } else {
                    total_usage.provider.clone()
                },
                model: if total_usage.model.is_empty() {
                    self.llm.config.model.clone()
                } else {
                    total_usage.model.clone()
                },
                prompt_tokens: total_usage.prompt_tokens as u64,
                completion_tokens: total_usage.completion_tokens as u64,
                total_tokens: total_usage.total_tokens as u64,
                request_count: telemetry_records.len() as u64,
                cached_tokens: total_usage.cached_tokens as u64,
            }),
            debug_payload: None,
            message_id: None,
            iterations: telemetry_records
                .iter()
                .map(|r| IterationRecord {
                    iteration: r.iteration,
                    plan: serde_json::json!({
                        "action_type": r.action_type,
                        "observation_preview": r.observation_preview,
                        "disclosed_skills": r.disclosed_skills,
                        "exit_reason": r.exit_reason,
                    }),
                    signals: EvaluationSignals::default(),
                    decision: r.exit_reason.clone(),
                    elapsed_ms: r.elapsed_ms,
                    llm_evaluation: None,
                    usage: r.llm_usage.clone(),
                })
                .collect(),
            total_tool_calls,
            tool_results: collected_tool_results.to_vec(),
            final_decision,
            query_resolution: request.query_resolution.clone(),
            trace_id: request.session_id.clone(),
            budget_used: Some(BudgetUsage {
                current: iteration,
                max: max_iterations,
            }),
            total_elapsed_ms: Some(total_elapsed_ms),
            trace: None,
            snapshot: None,
            decisions: Vec::new(),
            tool_calls: Vec::new(),
            routing_decision: None,
            eval_summary: None,
        }
    }
}

/// Safely truncate a string to at most `max_chars` characters (not bytes).
pub(crate) fn truncate_preview(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect::<String>() + "..."
    }
}

/// Build an OpenAI-format `assistant` message carrying `tool_calls`.
/// `call_ids` must be parallel to `calls` (e.g. `call_0`, `call_1`, ...).
/// If the LLM also emitted reasoning text in `content`, it is preserved so
/// the next iteration can see the model's chain-of-thought.
pub(crate) fn build_assistant_message_with_tool_calls(
    calls: &[common::ToolCall],
    call_ids: &[String],
    content: &str,
    reasoning_content: Option<String>,
) -> ChatMessage {
    let openai_calls: Vec<serde_json::Value> = calls
        .iter()
        .zip(call_ids.iter())
        .map(|(call, id)| {
            serde_json::json!({
                "id": id,
                "type": "function",
                "function": {
                    "name": call.tool,
                    "arguments": serde_json::to_string(&call.args)
                        .unwrap_or_else(|_| "{}".to_string()),
                }
            })
        })
        .collect();

    ChatMessage {
        role: "assistant".to_string(),
        content: content.to_string(),
        name: None,
        tool_call_id: None,
        tool_calls: Some(serde_json::json!(openai_calls)),
        reasoning_content,
    }
}

/// Build a `tool` role message from a native tool result, keyed by the
/// synthetic call id used in the assistant message.
pub(crate) fn build_tool_message(call_id: &str, tool_name: &str, result: &common::ToolResult) -> ChatMessage {
    let body = serde_json::json!({
        "tool": tool_name,
        "status": result.status,
        "data": result.data,
    });
    ChatMessage {
        role: "tool".to_string(),
        content: body.to_string(),
        name: Some(tool_name.to_string()),
        tool_call_id: Some(call_id.to_string()),
        tool_calls: None,
        reasoning_content: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::r#loop::config::BudgetConfig;
    use std::collections::HashMap;

    #[test]
    fn assistant_tool_calls_use_openai_format() {
        let calls = vec![common::ToolCall {
            tool: "dense_retrieval".to_string(),
            version: "1".to_string(),
            args: serde_json::json!({"query": "rust"}),
        }];
        let call_ids = vec!["call_0".to_string()];
        let msg = build_assistant_message_with_tool_calls(
            &calls,
            &call_ids,
            "thinking...",
            Some("internal reasoning".to_string()),
        );

        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content, "thinking...");
        assert_eq!(msg.reasoning_content.as_deref(), Some("internal reasoning"));
        let tc = msg.tool_calls.unwrap();
        let arr = tc.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], "call_0");
        assert_eq!(arr[0]["type"], "function");
        assert_eq!(arr[0]["function"]["name"], "dense_retrieval");
        assert_eq!(
            arr[0]["function"]["arguments"].as_str().unwrap(),
            r#"{"query":"rust"}"#
        );
    }

    #[test]
    fn tool_message_carries_matching_call_id() {
        let result = common::ToolResult {
            tool: "web_search".to_string(),
            version: "1".to_string(),
            status: common::ToolStatus::Ok,
            data: Some(serde_json::json!({"hits": 3})),
            trace: None,
        };
        let msg = build_tool_message("call_1", "web_search", &result);

        assert_eq!(msg.role, "tool");
        assert_eq!(msg.tool_call_id.as_deref(), Some("call_1"));
        assert_eq!(msg.name.as_deref(), Some("web_search"));
        assert!(msg.content.contains("\"hits\":3"));
    }

    #[test]
    fn budget_config_uses_tier_override_when_present() {
        let mut tiers = HashMap::new();
        tiers.insert("free".to_string(), 2);
        tiers.insert("pro".to_string(), 6);
        let cfg = BudgetConfig {
            max_iterations: 4,
            by_user_tier: Some(tiers),
        };
        assert_eq!(
            cfg.resolve_max_iterations(Some(&serde_json::json!("free"))),
            2
        );
        assert_eq!(
            cfg.resolve_max_iterations(Some(&serde_json::json!("PRO"))),
            6
        );
    }

    #[test]
    fn budget_config_falls_back_to_max_iterations_for_unknown_tier() {
        let mut tiers = HashMap::new();
        tiers.insert("free".to_string(), 2);
        let cfg = BudgetConfig {
            max_iterations: 4,
            by_user_tier: Some(tiers),
        };
        assert_eq!(
            cfg.resolve_max_iterations(Some(&serde_json::json!("enterprise"))),
            4
        );
    }

    #[test]
    fn budget_config_falls_back_when_no_tier() {
        let cfg = BudgetConfig {
            max_iterations: 4,
            by_user_tier: None,
        };
        assert_eq!(cfg.resolve_max_iterations(None), 4);
    }

    #[test]
    fn budget_config_clamps_to_at_least_one() {
        let cfg = BudgetConfig {
            max_iterations: 0,
            by_user_tier: None,
        };
        assert_eq!(cfg.resolve_max_iterations(None), 1);
    }

    #[test]
    fn fallback_dense_args_roundtrips() {
        let args = serde_json::to_value(common::DenseRetrievalArgs {
            queries: vec!["rust".to_string()],
            modality: common::DenseRetrievalModality::Text,
            top_k: 10,
            doc_scope: vec!["doc1".to_string()],
        })
        .unwrap();
        let round: common::DenseRetrievalArgs = serde_json::from_value(args).unwrap();
        assert_eq!(round.queries, vec!["rust"]);
        assert_eq!(round.top_k, 10);
    }

    #[test]
    fn fallback_lexical_args_roundtrips() {
        let args = serde_json::to_value(common::LexicalRetrievalArgs {
            terms: vec!["rust".to_string(), "lang".to_string()],
            top_k: 10,
            doc_scope: vec!["doc1".to_string()],
        })
        .unwrap();
        let round: common::LexicalRetrievalArgs = serde_json::from_value(args).unwrap();
        assert_eq!(round.terms, vec!["rust", "lang"]);
        assert_eq!(round.top_k, 10);
    }

    #[test]
    fn fallback_graph_args_roundtrips() {
        let args = serde_json::to_value(common::GraphRetrievalArgs {
            graph_hints: Vec::new(),
            placeholder_triplets: Vec::new(),
            relation_limit: 20,
            supporting_chunk_limit: 10,
            hop_limit: 1,
            fan_out_limit: 10,
            query: Some("rust".to_string()),
            doc_scope: vec!["doc1".to_string()],
        })
        .unwrap();
        let round: common::GraphRetrievalArgs = serde_json::from_value(args).unwrap();
        assert_eq!(round.query.as_deref(), Some("rust"));
        assert_eq!(round.hop_limit, 1);
    }

    #[test]
    fn auto_fallback_config_deserializes_vertical() {
        let yaml = r#"
enabled: true
tool_id: web_search
top_k: 10
vertical: news
"#;
        let cfg: super::config::AutoFallbackConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.vertical.as_deref(), Some("news"));
    }

    #[test]
    fn auto_fallback_config_default_vertical_none() {
        let yaml = r#"
enabled: true
tool_id: dense_retrieval
top_k: 10
"#;
        let cfg: super::config::AutoFallbackConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(cfg.vertical.is_none());
    }
}
