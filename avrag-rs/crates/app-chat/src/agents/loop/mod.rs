use std::sync::Arc;

pub mod answer_contract;
pub mod assembler;
pub mod fallback;
pub mod policy;
pub use policy::config as config;
pub use policy::disclosure_plan as disclosure_plan;
pub use policy::exit_policy as exit_policy;
pub use policy::LoopPolicy;
pub mod hooks;
pub mod iteration;
mod iteration_codegen;
mod iteration_tools;
pub mod message_queue;
pub mod optimizer;
pub mod parse;
pub mod query_normalize;
pub mod reasoning_emit;
mod run_result;
pub mod skill_request;
pub mod skills;
pub mod synthesis;
pub mod telemetry;

use crate::agents::capability::CapabilityRegistry;
use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::react_loop::DegradeReason;
use crate::agents::r#loop::optimizer::{IterationProgress, LoopOptimizer};
use crate::agents::runtime::{AgentRequest, AgentRunResult, FinalDecision};
use run_result::build_run_result;
use iteration::{IterationControl, IterationOutcome, IterationState};
use assembler::{ContextAssembler, DisclosedState};
use app_core::ChatPersistencePort;
use avrag_llm::{ChatMessage, LlmClient, LlmUsage};
use common::{AppError};
use contracts::{ToolResult};
use config::ModeConfig;
use exit_policy::{
    PostLoopAction, SynthesisGate, decide_synthesis_gate, degraded_no_evidence_answer,
    has_retrieval_observation, post_fallback_gate,
};
use hooks::{LoopContext, LoopHooks, StandardLoopHooks};
use query_normalize::normalize_query;
use synthesis::SynthesisPhase;
use telemetry::ReActIterationRecord;

pub(crate) fn merge_request_doc_scope(call: &mut contracts::ToolCall, doc_scope: &[String]) {
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
    call: &contracts::ToolCall,
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
    chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    code_interpreter: Arc<std::sync::Mutex<Option<avrag_code_interpreter::CodeInterpreter>>>,
}

impl ReActLoop {
    pub fn new(llm: Arc<LlmClient>, skill_registry: Arc<CapabilityRegistry>) -> Self {
        Self {
            llm,
            skill_registry,
            rag_runtime: None,
            search_executor: None,
            chat_persistence: None,
            code_interpreter: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub fn with_chat_persistence(
        mut self,
        chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    ) -> Self {
        self.chat_persistence = chat_persistence;
        self
    }

    fn effective_chat_persistence(&self) -> Option<Arc<dyn ChatPersistencePort>> {
        self.chat_persistence.clone().or_else(|| {
            self.rag_runtime
                .as_ref()
                .and_then(|runtime| runtime.chat_persistence())
        })
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

        let (request, base_message_count, max_iterations, auth, loop_user_query) =
            self.prepare_run_request(mode, request, norm, sink).await?;

        let mut state = IterationState {
            messages: self.build_initial_messages(mode, &request, &loop_user_query),
            disclosed: DisclosedState::default(),
            tool_results: Vec::new(),
            progress: IterationProgress::new(),
            total_tool_calls: 0,
            consecutive_sandbox_errors: 0,
            reasoning_acc: String::new(),
        };
        let (iteration, direct_answer, telemetry_records, total_usage) = self
            .run_retrieval_loop(
                mode,
                &request,
                &auth,
                &loop_exit,
                &hooks,
                base_message_count,
                max_iterations,
                &cancel,
                &mut state,
                sink,
            )
            .await?;

        let mut messages = state.messages;
        let mut disclosed_state = state.disclosed;
        let mut collected_tool_results = state.tool_results;
        let total_tool_calls = state.total_tool_calls;
        let reasoning_summary_acc = state.reasoning_acc;

        if cancel.is_cancelled() {
            return Err(crate::agents::react_loop::cancellation_error());
        }

        let retrieval_query = request.effective_query().to_string();
        if let Some(result) = self
            .resolve_synthesis_gate(
                mode,
                &loop_exit,
                &request,
                &auth,
                &retrieval_query,
                direct_answer.as_deref(),
                &mut messages,
                &mut collected_tool_results,
                &disclosed_state,
                sink,
                iteration,
                max_iterations,
                total_tool_calls,
                &telemetry_records,
                &total_usage,
                &reasoning_summary_acc,
                start_time,
            )
            .await?
        {
            return Ok(result);
        }

        self.run_synthesis_phase(
            mode,
            &request,
            &mut disclosed_state,
            &messages,
            &collected_tool_results,
            sink,
            &cancel,
            iteration,
            max_iterations,
            total_tool_calls,
            &telemetry_records,
            &total_usage,
            &reasoning_summary_acc,
            start_time,
        )
        .await
    }

    async fn prepare_run_request(
        &self,
        mode: &ModeConfig,
        request: AgentRequest,
        norm: query_normalize::NormalizeResult,
        sink: &dyn AgentEventSink,
    ) -> Result<
        (
            AgentRequest,
            usize,
            u8,
            avrag_auth::AuthContext,
            String,
        ),
        AppError,
    > {
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

        let loop_user_query = if mode.id == "rag" || mode.id == "search" {
            request.effective_query().to_string()
        } else {
            request.query.clone()
        };
        let base_message_count = request
            .messages
            .iter()
            .filter(|turn| turn.role == "user")
            .count()
            + 1;

        let max_iterations = request
            .max_iterations
            .unwrap_or_else(|| {
                mode.budget
                    .resolve_max_iterations(request.metadata.get("user_tier"))
            })
            .max(1);

        let auth: avrag_auth::AuthContext = serde_json::from_value(request.auth_context.clone())
            .map_err(|e| AppError::internal(format!("invalid auth context: {e}")))?;

        Ok((request, base_message_count, max_iterations, auth, loop_user_query))
    }

    fn build_initial_messages(
        &self,
        mode: &ModeConfig,
        request: &AgentRequest,
        loop_user_query: &str,
    ) -> Vec<ChatMessage> {
        let _ = mode;
        let mut messages: Vec<ChatMessage> = Vec::new();
        for turn in &request.messages {
            if turn.role == "user" {
                let content = format!("[prior_user_query] {}", turn.content);
                messages.push(ChatMessage::user(&content));
            }
        }
        messages.push(ChatMessage::user(loop_user_query));
        messages
    }

    async fn run_retrieval_loop(
        &self,
        mode: &ModeConfig,
        request: &AgentRequest,
        auth: &avrag_auth::AuthContext,
        loop_exit: &config::LoopExitConfig,
        hooks: &StandardLoopHooks,
        base_message_count: usize,
        max_iterations: u8,
        cancel: &tokio_util::sync::CancellationToken,
        state: &mut IterationState,
        sink: &dyn AgentEventSink,
    ) -> Result<(u8, Option<String>, Vec<ReActIterationRecord>, LlmUsage), AppError> {
        let mut iteration: u8 = 0;
        let mut telemetry_records: Vec<ReActIterationRecord> = vec![];
        let mut total_usage = LlmUsage::zeroed();
        let mut direct_answer: Option<String> = None;
        let optimizer = LoopOptimizer::new();

        loop {
            if cancel.is_cancelled() {
                break;
            }

            if self
                .check_iteration_budget_exhausted(iteration, max_iterations, state, sink)
                .await
            {
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
                    request,
                    auth,
                    loop_exit,
                    state,
                    &mut total_usage,
                    &optimizer,
                    sink,
                )
                .await?;

            self.emit_turn_end_telemetry(iteration, &outcome, sink, &mut telemetry_records)
                .await;

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
                    request,
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

        Ok((iteration, direct_answer, telemetry_records, total_usage))
    }

    async fn resolve_synthesis_gate(
        &self,
        mode: &ModeConfig,
        loop_exit: &config::LoopExitConfig,
        request: &AgentRequest,
        auth: &avrag_auth::AuthContext,
        retrieval_query: &str,
        direct_answer: Option<&str>,
        messages: &mut Vec<ChatMessage>,
        collected_tool_results: &mut Vec<ToolResult>,
        disclosed_state: &DisclosedState,
        sink: &dyn AgentEventSink,
        iteration: u8,
        max_iterations: u8,
        total_tool_calls: u32,
        telemetry_records: &[ReActIterationRecord],
        total_usage: &LlmUsage,
        reasoning_summary_acc: &str,
        start_time: std::time::Instant,
    ) -> Result<Option<AgentRunResult>, AppError> {
        let mut has_evidence =
            has_retrieval_observation(messages, collected_tool_results, mode);

        match decide_synthesis_gate(
            loop_exit,
            has_evidence,
            direct_answer,
            collected_tool_results,
            retrieval_query,
        ) {
            SynthesisGate::SkipSynthesisUseDirect(answer) => {
                return Ok(Some(
                    self.finish_direct_answer_run(
                        answer,
                        request,
                        disclosed_state,
                        collected_tool_results,
                        sink,
                        iteration,
                        max_iterations,
                        total_tool_calls,
                        telemetry_records,
                        total_usage,
                        reasoning_summary_acc,
                        start_time,
                        "skip_synthesis_direct",
                        FinalDecision::DirectAnswer,
                    )
                    .await?,
                ));
            }
            SynthesisGate::RunFallbackThenCheck => {
                if let Some(result) = self
                    .trigger_auto_fallback_and_check_degraded(
                        mode,
                        loop_exit,
                        request,
                        auth,
                        retrieval_query,
                        messages,
                        collected_tool_results,
                        disclosed_state,
                        sink,
                        iteration,
                        max_iterations,
                        total_tool_calls,
                        telemetry_records,
                        total_usage,
                        reasoning_summary_acc,
                        start_time,
                    )
                    .await?
                {
                    return Ok(Some(result));
                }
                has_evidence =
                    has_retrieval_observation(messages, collected_tool_results, mode);
            }
            SynthesisGate::DegradedNoEvidence => {
                return Ok(Some(
                    self.finish_direct_answer_run(
                        degraded_no_evidence_answer(&mode.id),
                        request,
                        disclosed_state,
                        collected_tool_results,
                        sink,
                        iteration,
                        max_iterations,
                        total_tool_calls,
                        telemetry_records,
                        total_usage,
                        reasoning_summary_acc,
                        start_time,
                        "degraded_no_evidence",
                        FinalDecision::Degraded {
                            reason: crate::agents::react_loop::DegradeReason::NoResultsAfterAllFallbacks,
                        },
                    )
                    .await?,
                ));
            }
            SynthesisGate::EnterSynthesis => {}
        }

        let _ = has_evidence;
        Ok(None)
    }

    async fn finish_direct_answer_run(
        &self,
        answer: String,
        request: &AgentRequest,
        disclosed_state: &DisclosedState,
        collected_tool_results: &[ToolResult],
        sink: &dyn AgentEventSink,
        iteration: u8,
        max_iterations: u8,
        total_tool_calls: u32,
        telemetry_records: &[ReActIterationRecord],
        total_usage: &LlmUsage,
        reasoning_summary_acc: &str,
        start_time: std::time::Instant,
        telemetry_label: &str,
        final_decision: FinalDecision,
    ) -> Result<AgentRunResult, AppError> {
        let disclosed_skills: Vec<String> = disclosed_state
            .disclosed_skill_ids
            .iter()
            .cloned()
            .collect();
        let observation_preview = truncate_preview(&answer, 200);
        reasoning_emit::emit_evaluation_telemetry(
            sink,
            iteration,
            telemetry_label,
            &observation_preview,
            &disclosed_skills,
            telemetry_label,
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
        self.finish_run(
            sink,
            answer,
            request,
            collected_tool_results,
            telemetry_records,
            total_usage,
            reasoning_summary_acc,
            iteration,
            max_iterations,
            total_tool_calls,
            start_time,
            Some(final_decision),
        )
        .await
    }

    async fn run_synthesis_phase(
        &self,
        mode: &ModeConfig,
        request: &AgentRequest,
        disclosed_state: &mut DisclosedState,
        messages: &[ChatMessage],
        collected_tool_results: &[ToolResult],
        sink: &dyn AgentEventSink,
        cancel: &tokio_util::sync::CancellationToken,
        iteration: u8,
        max_iterations: u8,
        total_tool_calls: u32,
        telemetry_records: &[ReActIterationRecord],
        total_usage: &LlmUsage,
        reasoning_summary_acc: &str,
        start_time: std::time::Instant,
    ) -> Result<AgentRunResult, AppError> {
        let synthesis_ctx = ContextAssembler::assemble_synthesis(
            mode,
            request,
            &self.skill_registry,
            disclosed_state,
        );
        reasoning_emit::emit_prompt_snapshot(
            sink,
            "synthesis",
            iteration,
            &synthesis_ctx,
            disclosed_state,
        )
        .await;
        reasoning_emit::emit_plan_decision_telemetry(
            sink,
            "synthesis",
            iteration,
            &synthesis_ctx,
            disclosed_state,
        )
        .await;

        let synthesis = SynthesisPhase;
        let final_answer = synthesis
            .run(
                &self.llm,
                &synthesis_ctx,
                mode,
                messages,
                collected_tool_results,
                sink,
                cancel,
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
            request,
            collected_tool_results,
            telemetry_records,
            total_usage,
            reasoning_summary_acc,
            iteration,
            max_iterations,
            total_tool_calls,
            start_time,
            Some(FinalDecision::Synthesized),
        )
        .await
    }

    async fn check_iteration_budget_exhausted(
        &self,
        iteration: u8,
        max_iterations: u8,
        state: &IterationState,
        sink: &dyn AgentEventSink,
    ) -> bool {
        if iteration < max_iterations {
            return false;
        }
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
        true
    }

    async fn emit_turn_end_telemetry(
        &self,
        iteration: u8,
        outcome: &IterationOutcome,
        sink: &dyn AgentEventSink,
        telemetry_records: &mut Vec<ReActIterationRecord>,
    ) {
        if outcome.sandbox_break {
            return;
        }
        let Some(record) = outcome.record.clone() else {
            return;
        };
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

    async fn trigger_auto_fallback_and_check_degraded(
        &self,
        mode: &ModeConfig,
        loop_exit: &config::LoopExitConfig,
        request: &AgentRequest,
        auth: &avrag_auth::AuthContext,
        retrieval_query: &str,
        messages: &mut Vec<ChatMessage>,
        collected_tool_results: &mut Vec<ToolResult>,
        disclosed_state: &DisclosedState,
        sink: &dyn AgentEventSink,
        iteration: u8,
        max_iterations: u8,
        total_tool_calls: u32,
        telemetry_records: &[ReActIterationRecord],
        total_usage: &LlmUsage,
        reasoning_summary_acc: &str,
        start_time: std::time::Instant,
    ) -> Result<Option<AgentRunResult>, AppError> {
        self.run_auto_fallback(
            mode,
            request,
            auth,
            retrieval_query,
            messages,
            collected_tool_results,
            sink,
        )
        .await?;
        let has_evidence =
            has_retrieval_observation(messages, collected_tool_results, mode);
        if post_fallback_gate(loop_exit, has_evidence) != PostLoopAction::DegradedNoEvidence {
            return Ok(None);
        }
        Ok(Some(
            self.finish_degraded_no_evidence_run(
                mode,
                request,
                disclosed_state,
                collected_tool_results,
                sink,
                iteration,
                max_iterations,
                total_tool_calls,
                telemetry_records,
                total_usage,
                reasoning_summary_acc,
                start_time,
            )
            .await?,
        ))
    }

    async fn finish_degraded_no_evidence_run(
        &self,
        mode: &ModeConfig,
        request: &AgentRequest,
        disclosed_state: &DisclosedState,
        collected_tool_results: &[ToolResult],
        sink: &dyn AgentEventSink,
        iteration: u8,
        max_iterations: u8,
        total_tool_calls: u32,
        telemetry_records: &[ReActIterationRecord],
        total_usage: &LlmUsage,
        reasoning_summary_acc: &str,
        start_time: std::time::Instant,
    ) -> Result<AgentRunResult, AppError> {
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
        let mut result = build_run_result(
            &self.llm,
            answer,
            request,
            collected_tool_results,
            telemetry_records,
            total_usage,
            reasoning_summary_acc,
            iteration,
            max_iterations,
            total_tool_calls,
            start_time,
            Some(FinalDecision::Degraded {
                reason: crate::agents::react_loop::DegradeReason::NoResultsAfterAllFallbacks,
            }),
        );
        result.degrade_trace.push(contracts::chat::DegradeTraceItem {
            stage: "degraded_no_evidence".to_string(),
            reason: DegradeReason::NoRetrievalEvidence,
            impact: "Answer withheld; synthesis skipped".to_string(),
        });
        self.emit_run_citations(sink, &result.citations).await;
        Ok(result)
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
            "dense_retrieval" | "lexical_retrieval" | "graph_retrieval" => {
                self.run_rag_retrieval_fallback(
                    request,
                    auth,
                    retrieval_query,
                    fallback,
                    messages,
                    collected_tool_results,
                )
                .await?;
            }
            "web_search" => {
                self.run_web_search_fallback(
                    retrieval_query,
                    fallback,
                    messages,
                    collected_tool_results,
                )
                .await?;
            }
            other => {
                self.emit_unknown_fallback_skipped(sink, other).await;
            }
        }
        Ok(())
    }

    async fn run_rag_retrieval_fallback(
        &self,
        request: &AgentRequest,
        auth: &avrag_auth::AuthContext,
        retrieval_query: &str,
        fallback: &config::AutoFallbackConfig,
        messages: &mut Vec<ChatMessage>,
        collected_tool_results: &mut Vec<ToolResult>,
    ) -> Result<(), AppError> {
        let Some(runtime) = &self.rag_runtime else {
            return Ok(());
        };
        let args = match fallback.tool_id.as_str() {
            "dense_retrieval" => serde_json::to_value(contracts::DenseRetrievalArgs {
                queries: vec![retrieval_query.to_string()],
                modality: contracts::DenseRetrievalModality::Both,
                top_k: fallback.top_k as usize,
                doc_scope: request.doc_scope.clone(),
            }),
            "lexical_retrieval" => serde_json::to_value(contracts::LexicalRetrievalArgs {
                terms: retrieval_query
                    .split_whitespace()
                    .map(ToOwned::to_owned)
                    .collect(),
                top_k: fallback.top_k as usize,
                doc_scope: request.doc_scope.clone(),
            }),
            "graph_retrieval" => serde_json::to_value(contracts::GraphRetrievalArgs {
                graph_hints: Vec::new(),
                placeholder_triplets: Vec::new(),
                relation_limit: 20,
                supporting_chunk_limit: 10,
                hop_limit: 1,
                fan_out_limit: 10,
                query: Some(retrieval_query.to_string()),
                doc_scope: request.doc_scope.clone(),
            }),
            _ => return Ok(()),
        }
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
        Ok(())
    }

    async fn run_web_search_fallback(
        &self,
        retrieval_query: &str,
        fallback: &config::AutoFallbackConfig,
        messages: &mut Vec<ChatMessage>,
        collected_tool_results: &mut Vec<ToolResult>,
    ) -> Result<(), AppError> {
        let Some(executor) = &self.search_executor else {
            return Ok(());
        };
        let v = fallback.vertical.as_deref().unwrap_or("web");
        match executor.execute_search(retrieval_query, Some(v)).await {
            Ok(response) => {
                let text = serde_json::to_string_pretty(&response)
                    .unwrap_or_else(|_| "search succeeded".to_string());
                messages.push(ChatMessage::system(format!("自动兜底搜索结果:\n{text}")));
                collected_tool_results.push(ToolResult {
                    tool: "web_search".to_string(),
                    version: "1.0".to_string(),
                    status: contracts::ToolStatus::Ok,
                    data: Some(serde_json::to_value(&response).unwrap_or_default()),
                    trace: None,
                });
            }
            Err(e) => {
                messages.push(ChatMessage::system(format!("[fallback failed: {e}]")));
            }
        }
        Ok(())
    }

    async fn emit_unknown_fallback_skipped(&self, sink: &dyn AgentEventSink, tool_id: &str) {
        let _ = sink
            .emit(AgentEvent::Activity {
                stage: "fallback_skipped".to_string(),
                message: format!("unknown fallback tool_id: {tool_id}"),
            })
            .await;
    }

    async fn emit_run_citations(&self, sink: &dyn AgentEventSink, citations: &[contracts::chat::Citation]) {
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
        let result = build_run_result(
            &self.llm,
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
    calls: &[contracts::ToolCall],
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
        multimodal_content: None,
        name: None,
        tool_call_id: None,
        tool_calls: Some(serde_json::json!(openai_calls)),
        reasoning_content,
    }
}

/// Build a `tool` role message from a native tool result, keyed by the
/// synthetic call id used in the assistant message.
pub(crate) fn build_tool_message(call_id: &str, tool_name: &str, result: &contracts::ToolResult) -> ChatMessage {
    let body = serde_json::json!({
        "tool": tool_name,
        "status": result.status,
        "data": result.data,
    });
    ChatMessage {
        role: "tool".to_string(),
        content: body.to_string(),
        multimodal_content: None,
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
        let calls = vec![contracts::ToolCall {
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
        let result = contracts::ToolResult {
            tool: "web_search".to_string(),
            version: "1".to_string(),
            status: contracts::ToolStatus::Ok,
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
        let args = serde_json::to_value(contracts::DenseRetrievalArgs {
            queries: vec!["rust".to_string()],
            modality: contracts::DenseRetrievalModality::Text,
            top_k: 10,
            doc_scope: vec!["doc1".to_string()],
        })
        .unwrap();
        let round: contracts::DenseRetrievalArgs = serde_json::from_value(args).unwrap();
        assert_eq!(round.queries, vec!["rust"]);
        assert_eq!(round.top_k, 10);
    }

    #[test]
    fn fallback_lexical_args_roundtrips() {
        let args = serde_json::to_value(contracts::LexicalRetrievalArgs {
            terms: vec!["rust".to_string(), "lang".to_string()],
            top_k: 10,
            doc_scope: vec!["doc1".to_string()],
        })
        .unwrap();
        let round: contracts::LexicalRetrievalArgs = serde_json::from_value(args).unwrap();
        assert_eq!(round.terms, vec!["rust", "lang"]);
        assert_eq!(round.top_k, 10);
    }

    #[test]
    fn fallback_graph_args_roundtrips() {
        let args = serde_json::to_value(contracts::GraphRetrievalArgs {
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
        let round: contracts::GraphRetrievalArgs = serde_json::from_value(args).unwrap();
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
