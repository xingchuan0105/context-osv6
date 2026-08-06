use std::sync::Arc;

pub mod answer_contract;
pub mod assembler;
pub mod fallback;
pub mod host_markers;
pub mod policy;
pub use policy::LoopPolicy;
pub use policy::config;
pub use policy::disclosure_plan;
pub use policy::exit_policy;
pub mod cancellation;
pub use cancellation::DegradeReason;
pub(crate) use cancellation::cancellation_error;
pub mod deps;
pub mod hooks;
pub mod iteration;
mod iteration_codegen;
mod iteration_tools;
pub mod json_fence;
mod message_format;
mod context_visibility;
mod claim_notes;
pub(crate) mod evidence_pool;
mod model_visible;
pub mod message_queue;
pub mod parse;
pub mod prompt_assets;
pub mod query_card;
pub mod sdk_gate;
pub mod session_fs;
// rag_bridge moved to agent-tools (TN Wave 6)
pub use agent_tools::rag_bridge;
pub mod reasoning_emit;
mod run_fallback;
mod run_prepare;
mod run_result;
mod run_retrieval;
mod run_synthesis;
pub mod short_judge;
pub mod skill_request;
pub mod skills;
pub mod synthesis;
pub mod telemetry;

use crate::events::AgentEventSink;
use crate::runtime::{AgentRequest, AgentRunResult};
use agent_tools::capability::CapabilityRegistry;
use app_core::ChatPersistencePort;
use assembler::DisclosedState;
use avrag_llm::LlmClient;
use common::AppError;
use config::ModeConfig;
use iteration::IterationState;
pub(crate) use message_format::{
    build_assistant_message_with_tool_calls, build_tool_message, truncate_observation,
    truncate_preview,
};

pub use deps::{BridgeCallObs, LoopRuntimeDeps};
pub use hooks::{BeforeToolCallOutcome, LoopContext, LoopHooks, StandardLoopHooks};
pub use policy::derive_mandatory_retrieve;
pub use sdk_gate::{method_allowed, sdk_primitives_for_caps};

/// ReAct retrieve → gate → synthesis engine.
///
/// Runtime side-effects (rag/search/codegen) live in [`LoopRuntimeDeps`]; prefer
/// `with_*` builders over reaching into `deps` from product code.
pub struct ReActLoop {
    llm: Arc<LlmClient>,
    skill_registry: Arc<CapabilityRegistry>,
    deps: LoopRuntimeDeps,
}

impl ReActLoop {
    pub fn new(llm: Arc<LlmClient>, skill_registry: Arc<CapabilityRegistry>) -> Self {
        Self {
            llm,
            skill_registry,
            deps: LoopRuntimeDeps::default(),
        }
    }

    pub fn with_chat_persistence(
        mut self,
        chat_persistence: Option<Arc<dyn ChatPersistencePort>>,
    ) -> Self {
        self.deps.chat_persistence = chat_persistence;
        self
    }

    pub fn with_rag_runtime(mut self, runtime: Option<Arc<avrag_rag_core::RagRuntime>>) -> Self {
        self.deps.rag_runtime = runtime;
        self
    }

    pub fn with_search_executor(
        mut self,
        executor: Option<Arc<dyn avrag_search::SearchProvider>>,
    ) -> Self {
        self.deps.search_executor = executor;
        self
    }

    /// Replace the whole deps bag (tests / advanced composition).
    pub fn with_runtime_deps(mut self, deps: LoopRuntimeDeps) -> Self {
        self.deps = deps;
        self
    }

    pub fn runtime_deps(&self) -> &LoopRuntimeDeps {
        &self.deps
    }

    /// Run with default [`StandardLoopHooks`] (two-tier compact: high=32, low=20).
    pub async fn run(
        &self,
        mode: &ModeConfig,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
    ) -> Result<AgentRunResult, AppError> {
        self.run_with_hooks(mode, request, sink, &StandardLoopHooks::default())
            .await
    }

    /// Run with an injected [`LoopHooks`] implementation.
    ///
    /// Prefer this over forking `run` for context transforms. Hooks are **not**
    /// the tool-policy truth source — see `EXTENDING.md` / `hooks` module docs.
    pub async fn run_with_hooks(
        &self,
        mode: &ModeConfig,
        request: AgentRequest,
        sink: &dyn AgentEventSink,
        hooks: &dyn LoopHooks,
    ) -> Result<AgentRunResult, AppError> {
        let start_time = std::time::Instant::now();
        let cancel = request.cancellation_token.clone().unwrap_or_default();
        if cancel.is_cancelled() {
            return Err(cancellation_error());
        }
        let loop_exit = mode.loop_exit_for_mode();

        let (request, base_message_count, max_iterations, auth, loop_user_query) =
            self.prepare_run_request(mode, request, sink).await?;

        // L0 题型卡（2026-08-03）：pre-loop 一次 json_mode 调用做结构化分类 +
        // 必做动作声明。失败 → None（卡缺省 = 埋点不激活，通用证据闸仍在）。
        // 该调用不占迭代预算（budget 在 run_retrieval_loop 内计）。
        // validate：声明了未知/未挂载动作的卡必须清洗——否则 L2.5 闸对一个
        // 沙箱里根本不可达的动作永远弹回，烧满轮次预算（2026-08-03 评审 P1）。
        let query_card = query_card::fetch_query_card(&self.llm, mode, &request.query)
            .await
            .map(|card| card.validate(mode));

        // W1 (2026-07-28, channel-persistent worker): a resumed worker session
        // passes its alias cursor so retrieval-log aliases stay unique across
        // briefs of the same turn (see worker_contract::RETRIEVAL_ALIAS_START_METADATA).
        let alias_start = request
            .metadata
            .get(crate::worker_contract::RETRIEVAL_ALIAS_START_METADATA)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let mut state = IterationState {
            messages: self.build_initial_messages(mode, &request, &loop_user_query),
            disclosed: DisclosedState::default(),
            tool_results: Vec::new(),
            total_tool_calls: 0,
            consecutive_sandbox_errors: 0,
            reasoning_acc: String::new(),
            answer_deltas_streamed: false,
            compile_continuations: 0,
            retrieval_aliases: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(alias_start)),
            evidence: evidence_pool::EvidencePool::new(),
            session_fs: std::sync::Arc::new(session_fs::SessionFs::new()),
            sdk_allowed: std::sync::Arc::new(mode.sdk_primitives.iter().cloned().collect()),
            query_card,
            max_iterations,
        };
        let (
            mut iteration,
            mut product_rounds_used,
            direct_answer,
            mut telemetry_records,
            mut total_usage,
            mut budget_exhaustion,
        ) = self
            .run_retrieval_loop(
                mode,
                &request,
                &auth,
                &loop_exit,
                hooks,
                base_message_count,
                max_iterations,
                &cancel,
                &mut state,
                sink,
                run_retrieval::RetrievalBudgetSeed::default(),
            )
            .await?;

        let mut messages = state.messages;
        let mut disclosed_state = state.disclosed;
        let mut collected_tool_results = state.tool_results;
        let mut total_tool_calls = state.total_tool_calls;
        let mut reasoning_summary_acc = state.reasoning_acc;
        let answer_deltas_streamed = state.answer_deltas_streamed;
        let query_card = state.query_card.clone();

        if cancel.is_cancelled() {
            return Err(cancellation_error());
        }

        let retrieval_query = request.query.clone();
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
                answer_deltas_streamed,
                query_card.as_ref(),
            )
            .await?
        {
            return Ok(result);
        }

        // --- synthesis (+ optional short Judge re-entry) ---
        let configured_max_fails = short_judge::judge_max_fail_rounds(&loop_exit);
        let mut judge_fails: u8 = 0;
        let mut judge_rereretrieve_iters: u8 = 0;
        // Hold user-bubble while short_judge may re-run (P0-2).
        let hold_for_judge = loop_exit.short_judge;
        let tier = request.metadata.get("user_tier");
        let product_max_tokens = mode.budget.resolve_max_tokens(tier);

        loop {
            if cancel.is_cancelled() {
                return Err(cancellation_error());
            }

            let run_judge = short_judge::should_run_short_judge(
                &loop_exit,
                query_card.as_ref(),
                &collected_tool_results,
            );
            let deliver_now = !hold_for_judge || !run_judge;
            let eff_max_fails = short_judge::effective_max_judge_fails(
                configured_max_fails,
                total_usage.billable_tokens(),
                product_max_tokens,
            );

            let (mut final_answer, synth_usage) = self
                .produce_synthesis_answer(
                    mode,
                    &request,
                    &mut disclosed_state,
                    &messages,
                    &collected_tool_results,
                    sink,
                    &cancel,
                    iteration,
                    budget_exhaustion,
                    deliver_now,
                )
                .await?;
            total_usage.accumulate(&synth_usage);

            if !run_judge {
                return self
                    .deliver_synthesized(
                        sink,
                        final_answer,
                        deliver_now,
                        &request,
                        &collected_tool_results,
                        &telemetry_records,
                        &total_usage,
                        &reasoning_summary_acc,
                        iteration,
                        max_iterations,
                        total_tool_calls,
                        start_time,
                        query_card.as_ref(),
                    )
                    .await;
            }

            // Token ceiling after synth: no more fail re-entries; deliver draft.
            if short_judge::budget_forces_ceiling(
                total_usage.billable_tokens(),
                product_max_tokens,
            ) && judge_fails > 0
            {
                final_answer = short_judge::append_judge_ceiling_disclosure(final_answer);
                return self
                    .deliver_synthesized(
                        sink,
                        final_answer,
                        false,
                        &request,
                        &collected_tool_results,
                        &telemetry_records,
                        &total_usage,
                        &reasoning_summary_acc,
                        iteration,
                        max_iterations,
                        total_tool_calls,
                        start_time,
                        query_card.as_ref(),
                    )
                    .await;
            }

            let judge_outcome = short_judge::run_short_judge(
                &self.llm,
                &request.query,
                &final_answer,
                &collected_tool_results,
                &messages,
                &cancel,
            )
            .await;

            let (verdict, parse_error) = match judge_outcome {
                Ok((o, ju)) => {
                    total_usage.accumulate(&ju);
                    (o.verdict, o.parse_error)
                }
                Err(e) => {
                    if cancel.is_cancelled() {
                        return Err(e);
                    }
                    tracing::warn!(error = %e, "short_judge LLM failed → deliver draft");
                    let _ = sink
                        .emit(crate::events::AgentEvent::Activity {
                            stage: "short_judge_error".to_string(),
                            message: format!("short Judge call failed: {e}"),
                            detail: None,
                            counts: Default::default(),
                            sources_preview: Vec::new(),
                        })
                        .await;
                    return self
                        .deliver_synthesized(
                            sink,
                            final_answer,
                            false,
                            &request,
                            &collected_tool_results,
                            &telemetry_records,
                            &total_usage,
                            &reasoning_summary_acc,
                            iteration,
                            max_iterations,
                            total_tool_calls,
                            start_time,
                            query_card.as_ref(),
                        )
                        .await;
                }
            };

            if parse_error {
                let _ = sink
                    .emit(crate::events::AgentEvent::Activity {
                        stage: "short_judge_parse_error".to_string(),
                        message: "short Judge JSON unparseable; treating as pass".to_string(),
                        detail: None,
                        counts: Default::default(),
                        sources_preview: Vec::new(),
                    })
                    .await;
            }

            match verdict {
                short_judge::JudgeVerdict::Pass => {
                    emit_judge_report(
                        sink,
                        "pass",
                        None,
                        "",
                        judge_fails,
                        false,
                        parse_error,
                        product_rounds_used,
                        judge_rereretrieve_iters,
                    )
                    .await;
                    return self
                        .deliver_synthesized(
                            sink,
                            final_answer,
                            false,
                            &request,
                            &collected_tool_results,
                            &telemetry_records,
                            &total_usage,
                            &reasoning_summary_acc,
                            iteration,
                            max_iterations,
                            total_tool_calls,
                            start_time,
                            query_card.as_ref(),
                        )
                        .await;
                }
                short_judge::JudgeVerdict::Fail { route, advice } => {
                    judge_fails = judge_fails.saturating_add(1);
                    let force_ceiling = short_judge::budget_forces_ceiling(
                        total_usage.billable_tokens(),
                        product_max_tokens,
                    );
                    let remaining_rounds =
                        max_iterations.saturating_sub(product_rounds_used);
                    let rereretrieve_cap = remaining_rounds
                        .min(run_retrieval::JUDGE_RERETRIEVE_MAX_ROUNDS);
                    let follow = if force_ceiling {
                        short_judge::JudgeFailFollowUp::DeliverCeiling
                    } else {
                        let mut f = short_judge::follow_up_after_judge_fail(
                            route,
                            &advice,
                            judge_fails,
                            eff_max_fails,
                        );
                        if matches!(f, short_judge::JudgeFailFollowUp::Reretrieve { .. })
                            && rereretrieve_cap == 0
                        {
                            f = short_judge::JudgeFailFollowUp::DeliverCeiling;
                        }
                        f
                    };
                    let (route_label, ceiling) = match &follow {
                        short_judge::JudgeFailFollowUp::DeliverCeiling => ("ceiling", true),
                        short_judge::JudgeFailFollowUp::Resynthesis { .. } => ("synthesis", false),
                        short_judge::JudgeFailFollowUp::Reretrieve { .. } => ("retrieve", false),
                    };
                    emit_judge_report(
                        sink,
                        "fail",
                        Some(route_label),
                        &advice,
                        judge_fails,
                        ceiling,
                        parse_error,
                        product_rounds_used,
                        judge_rereretrieve_iters,
                    )
                    .await;
                    match follow {
                        short_judge::JudgeFailFollowUp::DeliverCeiling => {
                            final_answer =
                                short_judge::append_judge_ceiling_disclosure(final_answer);
                            return self
                                .deliver_synthesized(
                                    sink,
                                    final_answer,
                                    false,
                                    &request,
                                    &collected_tool_results,
                                    &telemetry_records,
                                    &total_usage,
                                    &reasoning_summary_acc,
                                    iteration,
                                    max_iterations,
                                    total_tool_calls,
                                    start_time,
                                    query_card.as_ref(),
                                )
                                .await;
                        }
                        short_judge::JudgeFailFollowUp::Resynthesis { observation } => {
                            messages.push(avrag_llm::ChatMessage::user(observation));
                            messages.push(avrag_llm::ChatMessage::user(
                                prompt_assets::judge_draft_under_revision(&final_answer),
                            ));
                            budget_exhaustion = run_retrieval::BudgetExhaustion::default();
                        }
                        short_judge::JudgeFailFollowUp::Reretrieve { observation } => {
                            messages.push(avrag_llm::ChatMessage::user(observation));
                            state.messages = messages;
                            state.disclosed = disclosed_state;
                            state.tool_results = collected_tool_results;
                            state.total_tool_calls = total_tool_calls;
                            state.reasoning_acc = reasoning_summary_acc;
                            state.query_card = query_card.clone();
                            let (it2, rounds2, _da, tel2, usage2, be2) = self
                                .run_retrieval_loop(
                                    mode,
                                    &request,
                                    &auth,
                                    &loop_exit,
                                    hooks,
                                    base_message_count,
                                    max_iterations,
                                    &cancel,
                                    &mut state,
                                    sink,
                                    run_retrieval::RetrievalBudgetSeed {
                                        prior_usage: total_usage.clone(),
                                        max_additional_rounds: Some(rereretrieve_cap),
                                    },
                                )
                                .await?;
                            product_rounds_used =
                                product_rounds_used.saturating_add(rounds2);
                            judge_rereretrieve_iters =
                                judge_rereretrieve_iters.saturating_add(rounds2);
                            let mut counts = std::collections::BTreeMap::new();
                            counts.insert(
                                "product_rounds_used".to_string(),
                                product_rounds_used as usize,
                            );
                            counts.insert(
                                "this_rereretrieve_rounds".to_string(),
                                rounds2 as usize,
                            );
                            counts.insert(
                                "rereretrieve_cap".to_string(),
                                rereretrieve_cap as usize,
                            );
                            let _ = sink
                                .emit(crate::events::AgentEvent::Activity {
                                    stage: "judge_rereretrieve".to_string(),
                                    message: format!(
                                        "re-retrieve +{rounds2} rounds (product {product_rounds_used}/{max_iterations}, cap {rereretrieve_cap})"
                                    ),
                                    detail: None,
                                    counts,
                                    sources_preview: Vec::new(),
                                })
                                .await;
                            telemetry_records.extend(tel2);
                            total_usage.accumulate(&usage2);
                            budget_exhaustion = be2;
                            messages = state.messages;
                            disclosed_state = state.disclosed;
                            collected_tool_results = state.tool_results;
                            total_tool_calls = state.total_tool_calls;
                            reasoning_summary_acc = state.reasoning_acc;
                            iteration = it2;
                        }
                    }
                }
            }
        }
    }

    /// Emit user prose if not already streamed, then build the run result.
    async fn deliver_synthesized(
        &self,
        sink: &dyn AgentEventSink,
        final_answer: String,
        already_streamed: bool,
        request: &crate::runtime::AgentRequest,
        collected_tool_results: &[contracts::ToolResult],
        telemetry_records: &[telemetry::ReActIterationRecord],
        total_usage: &avrag_llm::LlmUsage,
        reasoning_summary_acc: &str,
        iteration: u8,
        max_iterations: u8,
        total_tool_calls: u32,
        start_time: std::time::Instant,
        query_card: Option<&query_card::QueryCard>,
    ) -> Result<crate::runtime::AgentRunResult, common::AppError> {
        if !already_streamed {
            synthesis::emit_prose_delivery(sink, &final_answer, None).await;
        }
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
            Some(crate::runtime::FinalDecision::Synthesized),
            query_card,
        )
        .await
    }
}

/// Design §8: route, advice summary, fail count, ceiling, rounds.
async fn emit_judge_report(
    sink: &dyn crate::events::AgentEventSink,
    verdict: &str,
    route: Option<&str>,
    advice: &str,
    fail_count: u8,
    ceiling: bool,
    parse_error: bool,
    product_rounds_used: u8,
    judge_rereretrieve_iters: u8,
) {
    let mut counts = std::collections::BTreeMap::new();
    counts.insert("judge_fail_count".to_string(), fail_count as usize);
    counts.insert(
        "product_rounds_used".to_string(),
        product_rounds_used as usize,
    );
    counts.insert(
        "judge_rereretrieve_iters".to_string(),
        judge_rereretrieve_iters as usize,
    );
    counts.insert("ceiling".to_string(), usize::from(ceiling));
    counts.insert("parse_error".to_string(), usize::from(parse_error));
    let summary = short_judge::advice_summary(advice, 240);
    let route_s = route.unwrap_or("-");
    let _ = sink
        .emit(crate::events::AgentEvent::Activity {
            stage: "short_judge_report".to_string(),
            message: format!(
                "short Judge verdict={verdict} route={route_s} fails={fail_count} ceiling={ceiling}"
            ),
            detail: Some(format!(
                "advice_summary={summary}; parse_error={parse_error}; product_rounds={product_rounds_used}"
            )),
            counts,
            sources_preview: Vec::new(),
        })
        .await;
}

#[cfg(test)]
mod tests;
