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
pub mod verify;
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

        // L0 题型卡：按挂载能力选 profile（纯 chat 跳过；search 极简；rag/dual 全量）。
        // 该调用不占迭代预算（budget 在 run_retrieval_loop 内计）。
        // validate：未知/未挂载动作清洗，避免 L2.5 闸对不可达动作烧满轮次。
        let card_profile = query_card::query_card_profile(mode);
        let query_card = query_card::fetch_query_card(&self.llm, mode, &request.query)
            .await
            .map(|card| card.validate(mode));

        // Pure-chat L0：无题卡 LLM + 轮次封顶，避免简单问题多轮沙箱空转。
        let max_iterations = if card_profile == query_card::QueryCardProfile::Off {
            max_iterations.min(2)
        } else {
            max_iterations
        };

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
            knockout: crate::helpers::shared_knockout(),
            ews: crate::helpers::EwsState::new(),
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

        // --- synthesis (+ optional verify re-entry) ---
        let configured_max_fails = verify::verify_max_fail_rounds(&loop_exit);
        let mut verify_fails: u8 = 0;
        let mut verify_rereretrieve_iters: u8 = 0;
        let mut verify_obs = verify::VerifyObservability::default();
        let knockout_ledger = std::sync::Arc::clone(&state.knockout);
        // Hold user-bubble while verify may re-run (P0-2).
        let hold_for_verify = loop_exit.verify;
        let tier = request.metadata.get("user_tier");
        let product_max_tokens = mode.budget.resolve_max_tokens(tier);

        loop {
            if cancel.is_cancelled() {
                return Err(cancellation_error());
            }

            let run_verify = verify::should_run_verify(
                &loop_exit,
                query_card.as_ref(),
                &collected_tool_results,
            );
            let deliver_now = !hold_for_verify || !run_verify;
            let eff_max_fails = verify::effective_max_verify_fails(
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
                    &mut state.ews,
                    sink,
                    &cancel,
                    iteration,
                    budget_exhaustion,
                    deliver_now,
                )
                .await?;
            total_usage.accumulate(&synth_usage);

            if !run_verify {
                if verify_obs.bypass_reason.is_none() {
                    verify_obs.bypass_reason = verify::verify_bypass_reason(
                        &loop_exit,
                        query_card.as_ref(),
                        &collected_tool_results,
                    )
                    .map(str::to_string);
                }
                verify_obs.product_rounds_used = product_rounds_used;
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
                        Some(verify_obs),
                        Some(crate::helpers::knockout_observability(&knockout_ledger)),
                        Some(state.ews.observability_snapshot()),
                    )
                    .await;
            }

            // Always run verify when product policy requires it — even if the
            // first synthesis already sits on the token ceiling. Skipping verify
            // here left ran=false / bypass=None on long rag_fact runs (full149).
            // After a fail, budget_forces_ceiling still forces DeliverCeiling
            // (no re-entry) further below.

            let verify_outcome = verify::run_verify(
                &self.llm,
                &request.query,
                &final_answer,
                &collected_tool_results,
                &messages,
                &cancel,
            )
            .await;

            let (verdict, parse_error) = match verify_outcome {
                Ok((o, ju)) => {
                    total_usage.accumulate(&ju);
                    (o.verdict, o.parse_error)
                }
                Err(e) => {
                    if cancel.is_cancelled() {
                        return Err(e);
                    }
                    tracing::warn!(error = %e, "verify LLM failed → deliver draft");
                    let _ = sink
                        .emit(crate::events::AgentEvent::Activity {
                            stage: "verify_error".to_string(),
                            message: format!("verify call failed: {e}"),
                            detail: None,
                            counts: Default::default(),
                            sources_preview: Vec::new(),
                        })
                        .await;
                    verify_obs.ran = true;
                    verify_obs.calls.push(verify::VerifyCallObs {
                        verdict: "error".to_string(),
                        route: None,
                        parse_error: false,
                        advice_summary: verify::advice_summary(&e.to_string(), 160),
                    });
                    verify_obs.fail_count = verify_fails;
                    verify_obs.rereretrieve_iters = verify_rereretrieve_iters;
                    verify_obs.product_rounds_used = product_rounds_used;
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
                            Some(verify_obs),
                            Some(crate::helpers::knockout_observability(&knockout_ledger)),
                            Some(state.ews.observability_snapshot()),
                        )
                        .await;
                }
            };

            verify_obs.ran = true;
            if parse_error {
                verify_obs.parse_error_count = verify_obs.parse_error_count.saturating_add(1);
                let _ = sink
                    .emit(crate::events::AgentEvent::Activity {
                        stage: "verify_parse_error".to_string(),
                        message: "verify JSON unparseable; treating as pass".to_string(),
                        detail: None,
                        counts: Default::default(),
                        sources_preview: Vec::new(),
                    })
                    .await;
            }

            match verdict {
                verify::VerifyVerdict::Pass => {
                    verify_obs.calls.push(verify::VerifyCallObs {
                        verdict: "pass".to_string(),
                        route: None,
                        parse_error,
                        advice_summary: String::new(),
                    });
                    verify_obs.fail_count = verify_fails;
                    verify_obs.rereretrieve_iters = verify_rereretrieve_iters;
                    verify_obs.product_rounds_used = product_rounds_used;
                    emit_verify_report(
                        sink,
                        "pass",
                        None,
                        "",
                        verify_fails,
                        false,
                        parse_error,
                        product_rounds_used,
                        verify_rereretrieve_iters,
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
                            Some(verify_obs),
                            Some(crate::helpers::knockout_observability(&knockout_ledger)),
                            Some(state.ews.observability_snapshot()),
                        )
                        .await;
                }
                verify::VerifyVerdict::Fail { route, advice } => {
                    verify_fails = verify_fails.saturating_add(1);
                    let force_ceiling = verify::budget_forces_ceiling(
                        total_usage.billable_tokens(),
                        product_max_tokens,
                    );
                    let remaining_rounds =
                        max_iterations.saturating_sub(product_rounds_used);
                    let rereretrieve_cap = remaining_rounds
                        .min(run_retrieval::VERIFY_RERETRIEVE_MAX_ROUNDS);
                    let follow = if force_ceiling {
                        verify::VerifyFailFollowUp::DeliverCeiling
                    } else {
                        let mut f = verify::follow_up_after_verify_fail(
                            route,
                            &advice,
                            verify_fails,
                            eff_max_fails,
                        );
                        if matches!(f, verify::VerifyFailFollowUp::Reretrieve { .. })
                            && rereretrieve_cap == 0
                        {
                            f = verify::VerifyFailFollowUp::DeliverCeiling;
                        }
                        f
                    };
                    let (route_label, ceiling) = match &follow {
                        verify::VerifyFailFollowUp::DeliverCeiling => ("ceiling", true),
                        verify::VerifyFailFollowUp::Resynthesis { .. } => ("synthesis", false),
                        verify::VerifyFailFollowUp::Reretrieve { .. } => ("retrieve", false),
                    };
                    verify_obs.calls.push(verify::VerifyCallObs {
                        verdict: "fail".to_string(),
                        route: Some(route_label.to_string()),
                        parse_error,
                        advice_summary: verify::advice_summary(&advice, 240),
                    });
                    if ceiling {
                        verify_obs.ceiling = true;
                    }
                    verify_obs.fail_count = verify_fails;
                    emit_verify_report(
                        sink,
                        "fail",
                        Some(route_label),
                        &advice,
                        verify_fails,
                        ceiling,
                        parse_error,
                        product_rounds_used,
                        verify_rereretrieve_iters,
                    )
                    .await;
                    match follow {
                        verify::VerifyFailFollowUp::DeliverCeiling => {
                            // Channel philosophy (2026-08-10): no host footnote.
                            // Token exhausted → sanitize draft / disaster prose.
                            // Rounds/fail ceiling with token left → one LLM closeout.
                            if force_ceiling {
                                final_answer = verify::finalize_delivery_without_llm(
                                    final_answer,
                                    &mode.id,
                                );
                            } else {
                                messages.push(avrag_llm::ChatMessage::user(
                                    prompt_assets::user_facing_closeout_observation(),
                                ));
                                messages.push(avrag_llm::ChatMessage::user(
                                    prompt_assets::verify_draft_under_revision(&final_answer),
                                ));
                                let (closeout, close_u) = self
                                    .produce_synthesis_answer(
                                        mode,
                                        &request,
                                        &mut disclosed_state,
                                        &messages,
                                        &collected_tool_results,
                                        &mut state.ews,
                                        sink,
                                        &cancel,
                                        iteration,
                                        budget_exhaustion,
                                        false,
                                    )
                                    .await?;
                                total_usage.accumulate(&close_u);
                                final_answer = verify::finalize_delivery_without_llm(
                                    closeout,
                                    &mode.id,
                                );
                            }
                            verify_obs.rereretrieve_iters = verify_rereretrieve_iters;
                            verify_obs.product_rounds_used = product_rounds_used;
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
                                    Some(verify_obs),
                                    Some(crate::helpers::knockout_observability(&knockout_ledger)),
                                    Some(state.ews.observability_snapshot()),
                                )
                                .await;
                        }
                        verify::VerifyFailFollowUp::Resynthesis { observation } => {
                            messages.push(avrag_llm::ChatMessage::user(observation));
                            messages.push(avrag_llm::ChatMessage::user(
                                prompt_assets::verify_draft_under_revision(&final_answer),
                            ));
                            // Keep retrieve-era BudgetExhaustion so C5 final-turn + last-Ok
                            // tool carryover still applies on the next synthesis (produce_synthesis
                            // injects it locally each call when exhaustion.any()).
                        }
                        verify::VerifyFailFollowUp::Reretrieve { observation } => {
                            messages.push(avrag_llm::ChatMessage::user(observation));
                            state.messages = messages;
                            state.disclosed = disclosed_state;
                            state.tool_results = collected_tool_results;
                            state.total_tool_calls = total_tool_calls;
                            state.reasoning_acc = reasoning_summary_acc;
                            state.query_card = query_card.clone();
                            let (_it2, rounds2, _da, tel2, usage2, be2) = self
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
                            verify_rereretrieve_iters =
                                verify_rereretrieve_iters.saturating_add(rounds2);
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
                                    stage: "verify_rereretrieve".to_string(),
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
                            // Product cumulative rounds for finish_run / telemetry — not the
                            // local re-retrieve loop counter (which restarts at 0).
                            iteration = product_rounds_used;
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
        verify_obs: Option<verify::VerifyObservability>,
        knockout_obs: Option<crate::helpers::KnockoutObservability>,
        ews_obs: Option<crate::helpers::EwsObservability>,
    ) -> Result<crate::runtime::AgentRunResult, common::AppError> {
        if !already_streamed {
            synthesis::emit_prose_delivery(sink, &final_answer, None).await;
        }
        let mut result = self
            .finish_run(
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
            .await?;
        result.verify = verify_obs;
        result.knockout = knockout_obs;
        result.ews = ews_obs;
        Ok(result)
    }
}

/// Design §8: route, advice summary, fail count, ceiling, rounds.
async fn emit_verify_report(
    sink: &dyn crate::events::AgentEventSink,
    verdict: &str,
    route: Option<&str>,
    advice: &str,
    fail_count: u8,
    ceiling: bool,
    parse_error: bool,
    product_rounds_used: u8,
    verify_rereretrieve_iters: u8,
) {
    let mut counts = std::collections::BTreeMap::new();
    counts.insert("verify_fail_count".to_string(), fail_count as usize);
    counts.insert(
        "product_rounds_used".to_string(),
        product_rounds_used as usize,
    );
    counts.insert(
        "verify_rereretrieve_iters".to_string(),
        verify_rereretrieve_iters as usize,
    );
    counts.insert("ceiling".to_string(), usize::from(ceiling));
    counts.insert("parse_error".to_string(), usize::from(parse_error));
    let summary = verify::advice_summary(advice, 240);
    let route_s = route.unwrap_or("-");
    let _ = sink
        .emit(crate::events::AgentEvent::Activity {
            stage: "verify_report".to_string(),
            message: format!(
                "verify verdict={verdict} route={route_s} fails={fail_count} ceiling={ceiling}"
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
