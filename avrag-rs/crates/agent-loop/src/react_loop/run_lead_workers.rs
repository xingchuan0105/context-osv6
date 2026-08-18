//! Lead + Workers retrieve path (W1–W4).
//!
//! Design: `docs/plans/2026-08-11-lead-rag-web-workers-design.md`.
//!
//! - PlanGate: 1 retrieval Brief per channel; optional base_tools host leaf
//! - Workers: RAG short SaC; Web multi-query host search (**with CRW**); dual spawn-isolated
//! - PackGate → optional **1× re-brief** (host structural) → synthesis
//! - Budget: product **rounds** (`max_iterations`); no synthesis token reserve
//! - Telemetry: PackGate outcomes, plan usage, brief rejects

use std::panic::AssertUnwindSafe;
use std::time::Instant;

use avrag_llm::ChatMessage;
use avrag_llm::LlmUsage;
use common::AppError;
use contracts::{ToolResult, ToolStatus};
use futures::future::join_all;
use futures::FutureExt;
use serde_json::json;

use super::config::{
    mode_has_rag_primitives, mode_has_web_primitives, LoopExitConfig, ModeConfig, RetrieveStrategy,
};
use super::hooks::LoopHooks;
use super::iteration::IterationState;
use super::lead_plan;
use super::prompt_assets;
use super::sdk_gate::sdk_primitives_for_caps;
use super::session_fs;
use super::telemetry::ReActIterationRecord;
use super::{ReActLoop, truncate_preview};
use crate::events::{AgentEvent, AgentEventSink};
use crate::lead_workers::{
    apply_pack_gate, count_tool_ok, effective_web_queries, hits_to_evidence_items,
    merge_search_responses, ActivatedCaps, Coverage, DocScopeSummary, EvidenceItem, EvidencePack,
    LeadPlanContext, PackGateOutcome, PreferredSource, SubTask, TaskBrief, validate_task_brief,
};
use crate::progress::{ProgressKind, WorkFact};
use crate::runtime::AgentRequest;
use tokio_util::sync::CancellationToken;

/// Hard host cap: at most one re-brief wave (design D4).
pub(super) const MAX_REBRIEF_WAVES: u8 = 1;

impl ReActLoop {
    /// Lead+Workers retrieve (rag-only / search-only / dual).
    ///
    /// `direct_answer` is always `None` so synthesis authors user prose.
    pub(super) async fn run_lead_workers_retrieve(
        &self,
        mode: &ModeConfig,
        request: &AgentRequest,
        auth: &contracts::auth_runtime::AuthContext,
        _loop_exit: &LoopExitConfig,
        hooks: &dyn LoopHooks,
        cancel: &CancellationToken,
        state: &mut IterationState,
        sink: &dyn AgentEventSink,
    ) -> Result<
        (
            u8,
            u8,
            Option<String>,
            Vec<ReActIterationRecord>,
            LlmUsage,
            super::run_retrieval::BudgetExhaustion,
        ),
        AppError,
    > {
        let wall = Instant::now();
        let mut session_usage = LlmUsage::zeroed();
        let mut run_log = super::run_log::RunEventLog::new();

        let caps = ActivatedCaps {
            rag: mode_has_rag_primitives(mode),
            search: mode_has_web_primitives(mode),
        };

        // --- Lead plan context observation ---
        let plan_ctx = build_plan_context(request, caps);
        let doc_scope_note = if !caps.rag {
            "本轮未激活知识库".to_string()
        } else if plan_ctx.has_docs() {
            format!("已挂载 {} 篇文档（清单如下，非全文）", plan_ctx.doc_scope.len())
        } else {
            "本轮无挂载文档".to_string()
        };
        let workspace_note = plan_ctx
            .workspace_id
            .as_deref()
            .unwrap_or("（未提供 workspace_id）");
        let plan_obs = prompt_assets::lead_plan_context_observation(
            caps.rag,
            caps.search,
            workspace_note,
            &doc_scope_note,
            &plan_ctx.doc_lines(),
        );
        state.messages.push(ChatMessage::user(plan_obs.clone()));

        // Lead LLM plan → retrieval + base_tools briefs.
        // Empty retrieval = BASE-only short path. LLM fail → host_fb.
        // Budget is **rounds** (mode max_iterations); no synthesis token reserve.
        //
        // Model split (2026-08-17): the Lead plans on the **synthesis** client
        // (primary model, thinking on/max) — plan quality (decomposition,
        // routing, JSON discipline) is the Lead's job and belongs to the
        // stronger model. Only Worker SaC / retrieval-loop turns ride the
        // retrieve override (`with_retrieve_llm`, thinking off).
        let host_fb = host_default_briefs(&request.query, caps);
        let plan = lead_plan::fetch_lead_briefs(
            &self.synthesis_llm,
            request,
            caps,
            &plan_obs,
            host_fb,
            &mut run_log,
        )
        .await;
        session_usage.accumulate(&plan.usage);

        // Host BASE tools leaf (weather / calculator / user_context) when the
        // Lead plan explicitly routes a brief to `base_tools`. The concrete tool
        // and its argument come from the brief itself (LLM-decided); the host
        // does no keyword/char intent guessing.
        if !plan.base_tool_briefs.is_empty() {
            self.run_base_tool_briefs(&plan.base_tool_briefs, request, state, &mut run_log)
                .await;
        }

        let mut packs: Vec<EvidencePack> = Vec::new();
        let mut rebrief_used: u8 = 0;
        let mut pack_gate_events: Vec<serde_json::Value> = Vec::new();
        let mut brief_rejects: Vec<BriefReject> = Vec::new();

        // Wave 0 (RAG ∥ Web when both present; panic-isolated)
        // Pack alias numbering continues the delivery replay stream
        // (helpers::selected::alias_chunk_ids_in_order over state.tool_results).
        let alias_offset =
            crate::helpers::selected::alias_chunk_ids_in_order(&state.tool_results).len();
        let (wave0, u0, rejects0, gates0) = self
            .dispatch_worker_wave(
                mode,
                auth,
                request,
                &plan.retrieval_briefs,
                caps,
                hooks,
                cancel,
                sink,
                /*rebrief*/ false,
                // Dead on wave 0 (is_rebrief=false); the re-brief block below
                // recomputes the tool from actual wave-0 usage.
                "dense_retrieval",
                alias_offset,
            )
            .await;
        session_usage.accumulate(&u0);
        log_wave_outcomes(&mut run_log, 0, &plan.retrieval_briefs, &wave0, &rejects0);
        brief_rejects.extend(rejects0);
        pack_gate_events.extend(gates0);
        apply_wave_outcomes(state, &mut packs, wave0, 0, &mut run_log);

        // Host structural re-brief ≤1, facet-granular: only sub-tasks that ran
        // and came back empty/insufficient. Does not invent sub-tasks Lead
        // intentionally omitted.
        let rebrief_targets = packs_needing_rebrief(&packs);
        if !rebrief_targets.is_empty() && rebrief_used < MAX_REBRIEF_WAVES && !cancel.is_cancelled()
        {
            rebrief_used = 1;
            run_log.push(super::run_log::RunEventKind::RebriefWave {
                targets: rebrief_targets.clone(),
            });
            let rebrief_briefs =
                host_rebrief_briefs(&packs, &rebrief_targets, &plan.retrieval_briefs);
            if !rebrief_briefs.is_empty() {
                // Tool choice from wave-0 usage facts: first unused tool wins
                // (a re-run with the same dead tool is a wasted wave).
                let stats = prior_tool_stats(&state.tool_results);
                let tool_id = rebrief_tool_choice(stats);
                state.messages.push(ChatMessage::user(
                    prompt_assets::rebrief_wave_observation(
                        rebrief_used,
                        &rebrief_targets.join(","),
                        &stats.render(),
                        tool_id,
                    ),
                ));
                let alias_offset =
                    crate::helpers::selected::alias_chunk_ids_in_order(&state.tool_results).len();
                let (wave1, u1, rejects1, gates1) = self
                    .dispatch_worker_wave(
                        mode,
                        auth,
                        request,
                        &rebrief_briefs,
                        caps,
                        hooks,
                        cancel,
                        sink,
                        /*rebrief*/ true,
                        tool_id,
                        alias_offset,
                    )
                    .await;
                session_usage.accumulate(&u1);
                log_wave_outcomes(&mut run_log, 1, &rebrief_briefs, &wave1, &rejects1);
                brief_rejects.extend(rejects1);
                pack_gate_events.extend(gates1);
                apply_wave_outcomes(state, &mut packs, wave1, 1, &mut run_log);
            }
        }

        if !brief_rejects.is_empty() {
            let lines = brief_rejects
                .iter()
                .map(|r| r.line())
                .collect::<Vec<_>>()
                .join("\n");
            state.messages.push(ChatMessage::user(
                prompt_assets::brief_gate_rejects_observation(&lines),
            ));
        }

        let (cov_summary, gaps_summary) = summarize_packs(&packs);
        state.messages.push(ChatMessage::user(
            prompt_assets::retrieval_worklog_observation(request.query.trim(), &run_log),
        ));
        run_log.push(super::run_log::RunEventKind::Handoff {
            packs: packs.len(),
        });
        state.messages.push(ChatMessage::user(
            prompt_assets::lead_workers_handoff_to_synthesis(packs.len()),
        ));

        let elapsed_ms = wall.elapsed().as_millis() as u64;
        let telemetry_records = vec![ReActIterationRecord {
            iteration: 0,
            disclosed_skills: vec![],
            action_type: "lead_workers".into(),
            observation_preview: format!(
                "n_packs={} rebrief_used={} coverage={} rejects={} plan_fallback={}",
                packs.len(),
                rebrief_used,
                cov_summary,
                brief_rejects.len(),
                plan.used_host_fallback
            ),
            llm_usage: Some(llm_usage_to_agent_run(&session_usage)),
            elapsed_ms,
            exit_reason: "break_to_synthesis".into(),
        }];

        let _ = sink
            .emit(AgentEvent::Evaluation {
                signals: Some(json!({
                    "lead_workers": true,
                    "n_packs": packs.len(),
                    "rebrief_used": rebrief_used,
                    "coverage_summary": cov_summary,
                    "gaps_summary": gaps_summary,
                    "brief_rejects": brief_rejects,
                    "pack_gate": pack_gate_events,
                    "plan_used_host_fallback": plan.used_host_fallback,
                    "n_base_tool_briefs": plan.base_tool_briefs.len(),
                    "run_events": run_log.to_json(),
                    "channels": packs.iter().map(|p| json!({
                        "channel": p.channel,
                        "coverage": p.coverage.as_str(),
                        "n_evidence": p.evidence.len(),
                        "tool_ok_count": p.tool_ok_count,
                        "gaps": p.gaps,
                    })).collect::<Vec<_>>(),
                })),
                decision: "break_to_synthesis".into(),
                reasoning: format!(
                    "Lead+Workers retrieve done; rebrief_used={rebrief_used}; {cov_summary}"
                ),
            })
            .await;

        if request.debug {
            let _ = sink
                .emit(AgentEvent::DebugTrace {
                    kind: "lead_workers".into(),
                    payload: json!({
                        "n_packs": packs.len(),
                        "rebrief_used": rebrief_used,
                        "coverage_summary": cov_summary,
                        "gaps_summary": gaps_summary,
                        "brief_rejects": brief_rejects,
                        "pack_gate": pack_gate_events,
                        "elapsed_ms": elapsed_ms,
                        "packs": packs,
                        "run_events": run_log.to_json(),
                    }),
                })
                .await;
        }

        tracing::info!(
            n_packs = packs.len(),
            rebrief_used,
            coverage = %cov_summary,
            elapsed_ms,
            n_rejects = brief_rejects.len(),
            "lead_workers retrieve complete → synthesis"
        );

        Ok((
            0,
            0,
            None,
            telemetry_records,
            session_usage,
            super::run_retrieval::BudgetExhaustion::default(),
        ))
    }

    /// Host-execute Lead `base_tools` briefs (weather / calculator / user_context).
    ///
    /// The concrete tool and argument are decided by the Lead LLM and carried in
    /// the brief (`sub_task.base_tool` / `sub_task.base_tool_arg`). The host only
    /// executes; it never guesses intent from keywords or characters.
    async fn run_base_tool_briefs(
        &self,
        briefs: &[TaskBrief],
        request: &AgentRequest,
        state: &mut IterationState,
        run_log: &mut super::run_log::RunEventLog,
    ) {
        for brief in briefs {
            let tool_name = brief.sub_task.base_tool.trim().to_ascii_lowercase();
            let arg = if brief.sub_task.base_tool_arg.trim().is_empty() {
                brief.sub_task.objective.trim().to_string()
            } else {
                brief.sub_task.base_tool_arg.trim().to_string()
            };
            let (tool, result) = match tool_name.as_str() {
                "weather" | "weather_query" => {
                    let tr = self.deps.execute_weather_query(&arg).await;
                    ("weather_query", tr)
                }
                "calculator" | "calc" => {
                    let tr = self.deps.execute_calculator(&arg).await;
                    ("calculator", tr)
                }
                "user_context" | "time" | "clock" => {
                    let tr = self.deps.execute_user_context(request).await;
                    ("user_context", tr)
                }
                _ => {
                    let tr = ToolResult {
                        tool: "base_tools".into(),
                        version: "1.0".into(),
                        status: ToolStatus::Error,
                        data: Some(json!({
                            "error": "base_tools_unmapped",
                            "objective": brief.sub_task.objective,
                            "note": "brief did not specify a concrete base tool (weather | calculator | user_context)",
                        })),
                        trace: None,
                    };
                    ("base_tools", tr)
                }
            };
            let status = match result.status {
                ToolStatus::Ok => "ok",
                ToolStatus::Error => "error",
                _ => "other",
            };
            let payload = result
                .data
                .as_ref()
                .map(|d| d.to_string())
                .unwrap_or_else(|| "{}".into());
            let payload: String = payload.chars().take(2000).collect();
            run_log.push(super::run_log::RunEventKind::BaseToolExecuted {
                tool: tool.to_string(),
                ok: result.status == ToolStatus::Ok,
            });
            state.tool_results.push(result);
            state.messages.push(ChatMessage::user(
                prompt_assets::base_tools_result_observation(tool, status, &payload),
            ));
        }
    }

    async fn dispatch_worker_wave(
        &self,
        mode: &ModeConfig,
        auth: &contracts::auth_runtime::AuthContext,
        request: &AgentRequest,
        briefs: &[TaskBrief],
        caps: ActivatedCaps,
        hooks: &dyn LoopHooks,
        cancel: &CancellationToken,
        sink: &dyn AgentEventSink,
        is_rebrief: bool,
        rebrief_tool_id: &str,
        alias_offset: usize,
    ) -> (
        Vec<WorkerOutcome>,
        LlmUsage,
        Vec<BriefReject>,
        Vec<serde_json::Value>,
    ) {
        // v1: one brief per channel (PlanGate first-wins). Extra same-channel
        // briefs here are defense-in-depth drops with a warn + Lead-visible reject.
        let mut rag_brief: Option<&TaskBrief> = None;
        let mut web_brief: Option<&TaskBrief> = None;
        let mut rejects: Vec<BriefReject> = Vec::new();
        for brief in briefs {
            if let Err(e) = validate_task_brief(brief, caps) {
                tracing::warn!(error = %e, is_rebrief, "lead_workers brief gate failed; skip");
                rejects.push(BriefReject {
                    id: brief.sub_task.id.clone(),
                    source: brief.sub_task.preferred_source.as_str().to_string(),
                    reason: e.to_string(),
                });
                continue;
            }
            match brief.sub_task.preferred_source {
                PreferredSource::Rag if caps.rag => {
                    if rag_brief.is_some() {
                        tracing::warn!(
                            sub_task_id = %brief.sub_task.id,
                            is_rebrief,
                            "lead_workers: extra rag brief skipped (one per channel)"
                        );
                        rejects.push(BriefReject {
                            id: brief.sub_task.id.clone(),
                            source: "rag".into(),
                            reason: "duplicate_channel_slot".into(),
                        });
                    } else {
                        rag_brief = Some(brief);
                    }
                }
                PreferredSource::Web if caps.search => {
                    if web_brief.is_some() {
                        tracing::warn!(
                            sub_task_id = %brief.sub_task.id,
                            is_rebrief,
                            "lead_workers: extra web brief skipped (one per channel)"
                        );
                        rejects.push(BriefReject {
                            id: brief.sub_task.id.clone(),
                            source: "web".into(),
                            reason: "duplicate_channel_slot".into(),
                        });
                    } else {
                        web_brief = Some(brief);
                    }
                }
                PreferredSource::BaseTools | PreferredSource::None => {
                    // Handled outside dispatch (base tools leaf / no-op).
                }
                PreferredSource::Rag | PreferredSource::Web => {
                    rejects.push(BriefReject {
                        id: brief.sub_task.id.clone(),
                        source: brief.sub_task.preferred_source.as_str().to_string(),
                        reason: "source_not_activated".into(),
                    });
                }
            }
        }

        if let Some(b) = rag_brief {
            let _ = crate::progress::emit_work_fact(
                sink,
                WorkFact::delegate(ProgressKind::DelegateRag, &b.sub_task.objective),
            )
            .await;
        }
        if let Some(b) = web_brief {
            let _ = crate::progress::emit_work_fact(
                sink,
                WorkFact::delegate(ProgressKind::DelegateSearch, &b.sub_task.objective),
            )
            .await;
        }

        // True parallel when both channels: wall ≈ max(rag, web).
        // catch_unwind so one Worker panic does not take down the other.
        let (rag_out, web_out) = match (rag_brief, web_brief) {
            (Some(rb), Some(wb)) => {
                let rag_fut = async {
                    if is_rebrief {
                        (
                            self.run_rag_worker_host(
                                auth,
                                request,
                                rb,
                                rebrief_tool_id,
                                alias_offset,
                            )
                            .await,
                            LlmUsage::zeroed(),
                        )
                    } else {
                        self.run_rag_worker_short_sac(
                            mode, auth, request, rb, hooks, cancel, sink, alias_offset,
                        )
                        .await
                    }
                };
                let web_fut = async {
                    (
                        self.run_web_worker_host_leaf(wb, cancel).await,
                        LlmUsage::zeroed(),
                    )
                };
                let (r, w) = tokio::join!(
                    AssertUnwindSafe(rag_fut).catch_unwind(),
                    AssertUnwindSafe(web_fut).catch_unwind()
                );
                let r = match r {
                    Ok(v) => Some(v),
                    Err(_) => {
                        tracing::error!("lead_workers rag worker panicked; channel isolated");
                        Some((
                            vec![empty_panic_outcome("rag", rb.sub_task.id.as_str())],
                            LlmUsage::zeroed(),
                        ))
                    }
                };
                let w = match w {
                    Ok(v) => Some(v),
                    Err(_) => {
                        tracing::error!("lead_workers web worker panicked; channel isolated");
                        Some((
                            empty_panic_outcome("web", wb.sub_task.id.as_str()),
                            LlmUsage::zeroed(),
                        ))
                    }
                };
                (r, w)
            }
            (Some(rb), None) => {
                let fut = async {
                    if is_rebrief {
                        (
                            self.run_rag_worker_host(
                                auth,
                                request,
                                rb,
                                rebrief_tool_id,
                                alias_offset,
                            )
                            .await,
                            LlmUsage::zeroed(),
                        )
                    } else {
                        self.run_rag_worker_short_sac(
                            mode, auth, request, rb, hooks, cancel, sink, alias_offset,
                        )
                        .await
                    }
                };
                let r = match AssertUnwindSafe(fut).catch_unwind().await {
                    Ok(v) => Some(v),
                    Err(_) => {
                        tracing::error!("lead_workers rag worker panicked; channel isolated");
                        Some((
                            vec![empty_panic_outcome("rag", rb.sub_task.id.as_str())],
                            LlmUsage::zeroed(),
                        ))
                    }
                };
                (r, None)
            }
            (None, Some(wb)) => {
                let fut = async {
                    (
                        self.run_web_worker_host_leaf(wb, cancel).await,
                        LlmUsage::zeroed(),
                    )
                };
                let w = match AssertUnwindSafe(fut).catch_unwind().await {
                    Ok(v) => Some(v),
                    Err(_) => {
                        tracing::error!("lead_workers web worker panicked; channel isolated");
                        Some((
                            empty_panic_outcome("web", wb.sub_task.id.as_str()),
                            LlmUsage::zeroed(),
                        ))
                    }
                };
                (None, w)
            }
            (None, None) => (None, None),
        };

        let mut out = Vec::new();
        let mut usage = LlmUsage::zeroed();
        let mut gate_events = Vec::new();
        if let Some((os, u)) = rag_out {
            usage.accumulate(&u);
            for o in os {
                gate_events.push(pack_gate_json(&o));
                out.push(o);
            }
        }
        if let Some((o, u)) = web_out {
            usage.accumulate(&u);
            gate_events.push(pack_gate_json(&o));
            out.push(o);
        }
        (out, usage, rejects, gate_events)
    }

    /// Short SaC: one Worker, facets executed **sequentially** — each facet
    /// gets its own step budget (independent budget) and its own
    /// host-assembled pack (independent screening). Single-facet briefs
    /// behave exactly like the pre-facet path.
    async fn run_rag_worker_short_sac(
        &self,
        parent_mode: &ModeConfig,
        auth: &contracts::auth_runtime::AuthContext,
        request: &AgentRequest,
        brief: &TaskBrief,
        hooks: &dyn LoopHooks,
        cancel: &CancellationToken,
        sink: &dyn AgentEventSink,
        alias_offset: usize,
    ) -> (Vec<WorkerOutcome>, LlmUsage) {
        // Product clamps Worker SaC to ≤5 per facet. Full-149 budget baseline
        // (`E2E_UNLIMITED_BUDGET=1`) raises the clamp so measured usage is not
        // capped by the host Worker step wall before Lead rounds.
        let step_cap = if e2e_unlimited_budget() { 32 } else { 5 };
        let max_steps = brief.sub_task.max_steps.clamp(1, step_cap);

        let mut outcomes = Vec::new();
        let mut total_usage = LlmUsage::zeroed();
        let mut alias_cursor = alias_offset;
        for facet in brief.sub_task.effective_facets() {
            // Facet-scoped brief view: the worker sees this unit standalone.
            let mut fbrief = brief.clone();
            fbrief.sub_task.id = facet.id.clone();
            fbrief.sub_task.objective = facet.objective.clone();
            fbrief.sub_task.facets = vec![];
            let (outcome, usage) = self
                .run_rag_facet_sac(
                    parent_mode,
                    auth,
                    request,
                    &fbrief,
                    max_steps,
                    hooks,
                    cancel,
                    sink,
                    alias_cursor,
                )
                .await;
            alias_cursor +=
                crate::helpers::selected::alias_chunk_ids_in_order(&outcome.tool_results).len();
            total_usage.accumulate(&usage);
            outcomes.push(outcome);
            if cancel.is_cancelled() {
                break;
            }
        }
        (outcomes, total_usage)
    }

    /// One facet: nested SacCodegen retrieve (rag-only SDK, max_steps) →
    /// EvidencePack. Host assembles pack from ToolResults (no dense rewire).
    /// Returns (outcome, nested LLM usage for product budget telemetry).
    #[allow(clippy::too_many_arguments)]
    async fn run_rag_facet_sac(
        &self,
        parent_mode: &ModeConfig,
        auth: &contracts::auth_runtime::AuthContext,
        request: &AgentRequest,
        brief: &TaskBrief,
        max_steps: u8,
        hooks: &dyn LoopHooks,
        cancel: &CancellationToken,
        sink: &dyn AgentEventSink,
        alias_offset: usize,
    ) -> (WorkerOutcome, LlmUsage) {
        let started = Instant::now();

        let mut worker_mode = parent_mode.clone();
        worker_mode.retrieve_strategy = RetrieveStrategy::SacCodegen;
        worker_mode.budget.max_iterations = max_steps;
        worker_mode.budget.baseline_iterations = 0;
        worker_mode.sdk_primitives = sdk_primitives_for_caps(true, false)
            .into_iter()
            .map(str::to_string)
            .collect();
        worker_mode.tool_pool.clear();
        worker_mode.loop_exit.forbid_retrieve_direct_answer = true;
        worker_mode.loop_exit.skip_synthesis_on_direct_answer = false;
        worker_mode.loop_exit.verify = false;
        worker_mode.worker_handoff = false;
        // Worker system: sandbox + KB contract only (not Lead, not web contract).
        worker_mode.system_prompt_base = "prompts/system/worker-sandbox.md".into();

        let mut worker_request = request.clone();
        worker_request.metadata.insert(
            "system_prompt_parts".into(),
            json!([
                "prompts/system/worker-sandbox.md",
                "prompts/capabilities/knowledge-base/contract.md",
                "prompts/workers/rag/SKILL.md",
            ]),
        );
        // Point worker query at sub-task (still keeps history on request.messages).
        if !brief.original_query.trim().is_empty() {
            worker_request.query = brief.original_query.clone();
        }

        let brief_json = serde_json::to_string_pretty(brief).unwrap_or_else(|_| "{}".into());
        let mut worker_state = IterationState {
            messages: vec![
                ChatMessage::user(prompt_assets::task_brief_observation(&brief_json)),
                ChatMessage::user(prompt_assets::rag_worker_sac_observation()),
                ChatMessage::user(brief.original_query.clone()),
            ],
            disclosed: Default::default(),
            tool_results: Vec::new(),
            total_tool_calls: 0,
            consecutive_sandbox_errors: 0,
            reasoning_acc: String::new(),
            answer_deltas_streamed: false,
            compile_continuations: 0,
            retrieval_aliases: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            evidence: super::evidence_pool::EvidencePool::new(),
            knockout: crate::helpers::shared_knockout(),
            ews: crate::helpers::EwsState::new(),
            session_fs: std::sync::Arc::new(session_fs::SessionFs::new()),
            sdk_allowed: std::sync::Arc::new(
                worker_mode.sdk_primitives.iter().cloned().collect(),
            ),
            query_card: None,
            max_iterations: max_steps,
        };

        let loop_exit = worker_mode.loop_exit.clone();
        // Nested SacCodegen loop (not LeadWorkers). Pin to avoid infinite async type.
        let nested = Box::pin(self.run_retrieval_loop(
            &worker_mode,
            &worker_request,
            auth,
            &loop_exit,
            hooks,
            0,
            max_steps,
            cancel,
            &mut worker_state,
            sink,
            super::run_retrieval::RetrievalBudgetSeed::default(),
        ))
        .await;

        // No host dense rewire on SaC fail/empty (design §13.3): assemble pack from
        // whatever ToolResults the worker left; gaps surface to Lead.
        let nested_usage = match &nested {
            Ok((_, _, _, _, u, _)) => u.clone(),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "rag short SaC failed; host assembles pack from available tool results (no dense rewire)"
                );
                LlmUsage::zeroed()
            }
        };

        let mut tool_results = worker_state.tool_results;
        let evidence = evidence_from_tool_results(&tool_results, alias_offset);
        let n = evidence.len();
        let tool_ok = count_tool_ok(&tool_results);
        let pack = EvidencePack {
            schema_version: "evidence_pack_v1".into(),
            sub_task_id: brief.sub_task.id.clone(),
            channel: "rag".into(),
            evidence,
            coverage: if n > 0 {
                Coverage::Partial
            } else {
                Coverage::Insufficient
            },
            gaps: if n == 0 {
                if nested.is_err() {
                    "rag_sac_error".into()
                } else {
                    "rag_sac_empty".into()
                }
            } else {
                String::new()
            },
            tool_ok_count: 0,
        };
        let (pack, gate) = apply_pack_gate(pack, tool_ok, Some("rag"));
        let pack_json = serde_json::to_string_pretty(&pack).unwrap_or_else(|_| "{}".into());

        tracing::info!(
            channel = "rag",
            mode = "short_sac",
            n_hits = n,
            max_steps,
            elapsed_ms = started.elapsed().as_millis() as u64,
            coverage = pack.coverage.as_str(),
            pack_gate = gate.kind_str(),
            // Diagnosis fields: 0 executions = model never emitted a runnable
            // block; executions>0 + tool_results=0 + sandbox_errors>0 = sandbox
            // failing; executions>0 + tool_results=0 + errors=0 = code ran but
            // made no bridge RPC.
            tool_calls = worker_state.total_tool_calls,
            tool_results = tool_results.len(),
            sandbox_errors = worker_state.consecutive_sandbox_errors,
            "lead_workers rag worker done"
        );

        // Avoid duplicating huge tool_results into parent if empty evidence after gate.
        if pack.evidence.is_empty() {
            tool_results.clear();
        }

        (
            WorkerOutcome {
                pack,
                pack_gate: gate,
                tool_results,
                observation: prompt_assets::evidence_pack_observation(&pack_json),
            },
            nested_usage,
        )
    }

    async fn run_web_worker_host_leaf(
        &self,
        brief: &TaskBrief,
        cancel: &CancellationToken,
    ) -> WorkerOutcome {
        let queries = effective_web_queries(brief);
        let started = Instant::now();

        // Parallel multi-query fan-out (wall ≈ max latency).
        let futs: Vec<_> = queries
            .iter()
            .map(|q| {
                let q = q.clone();
                async move {
                    if cancel.is_cancelled() {
                        return (q, WebAttempt::Cancelled);
                    }
                    match self.deps.execute_search_fallback(&q, Some("web")).await {
                        None => (q, WebAttempt::Unavailable),
                        Some(Err(e)) => (q, WebAttempt::Err(e.to_string())),
                        Some(Ok(resp)) => (q, WebAttempt::Ok(resp)),
                    }
                }
            })
            .collect();
        let results = join_all(futs).await;

        let mut pairs: Vec<(String, avrag_search::SearchResponse)> = Vec::new();
        let mut any_ok = false;
        let mut last_err = String::new();
        let mut cancelled = false;
        for (q, res) in results {
            match res {
                WebAttempt::Cancelled => {
                    cancelled = true;
                    last_err = "cancelled".into();
                }
                WebAttempt::Unavailable => {
                    if !cancelled {
                        last_err = "search provider not available".into();
                    }
                }
                WebAttempt::Err(e) => {
                    if !cancelled {
                        last_err = e;
                    }
                }
                WebAttempt::Ok(resp) => {
                    any_ok = true;
                    pairs.push((q, resp));
                }
            }
        }

        let merged = merge_search_responses(&pairs, 80);
        let evidence = hits_to_evidence_items(&merged);
        let n = evidence.len();

        let tool_result = if any_ok && n > 0 {
            web_search_tool_result(&merged)
        } else {
            ToolResult {
                tool: "web_search".into(),
                version: "1.0".into(),
                status: ToolStatus::Error,
                data: Some(json!({
                    "error": if last_err.is_empty() { "empty results" } else { &last_err },
                    "queries": queries,
                })),
                trace: None,
            }
        };

        let tool_ok = count_tool_ok(std::slice::from_ref(&tool_result));
        let pack = EvidencePack {
            schema_version: "evidence_pack_v1".into(),
            sub_task_id: brief.sub_task.id.clone(),
            channel: "web".into(),
            evidence,
            coverage: if n > 0 {
                Coverage::Partial
            } else {
                Coverage::Insufficient
            },
            gaps: if n == 0 {
                if last_err.is_empty() {
                    "web_empty".into()
                } else {
                    last_err
                }
            } else {
                String::new()
            },
            tool_ok_count: 0,
        };
        let (pack, gate) = apply_pack_gate(pack, tool_ok, Some("web"));
        let pack_json = serde_json::to_string_pretty(&pack).unwrap_or_else(|_| "{}".into());

        tracing::info!(
            channel = "web",
            n_hits = n,
            elapsed_ms = started.elapsed().as_millis() as u64,
            coverage = pack.coverage.as_str(),
            pack_gate = gate.kind_str(),
            "lead_workers web worker done"
        );

        WorkerOutcome {
            pack,
            pack_gate: gate,
            tool_results: vec![tool_result],
            observation: prompt_assets::evidence_pack_observation(&pack_json),
        }
    }

    /// Host-side rag leaf (re-brief path), facet-granular: one host retrieval
    /// per facet, one pack per facet.
    async fn run_rag_worker_host(
        &self,
        auth: &contracts::auth_runtime::AuthContext,
        request: &AgentRequest,
        brief: &TaskBrief,
        tool_id: &str,
        alias_offset: usize,
    ) -> Vec<WorkerOutcome> {
        let mut out = Vec::new();
        let mut alias_cursor = alias_offset;
        for facet in brief.sub_task.effective_facets() {
            let mut fbrief = brief.clone();
            fbrief.sub_task.id = facet.id.clone();
            fbrief.sub_task.facets = vec![];
            let o = self
                .run_rag_host_leaf_one(auth, request, &fbrief, tool_id, alias_cursor)
                .await;
            alias_cursor +=
                crate::helpers::selected::alias_chunk_ids_in_order(&o.tool_results).len();
            out.push(o);
        }
        out
    }

    /// One host-leaf retrieval for a single (facet-scoped) brief.
    async fn run_rag_host_leaf_one(
        &self,
        auth: &contracts::auth_runtime::AuthContext,
        request: &AgentRequest,
        brief: &TaskBrief,
        tool_id: &str,
        alias_offset: usize,
    ) -> WorkerOutcome {
        let started = Instant::now();
        let query = brief.sub_task.objective.trim();

        let mut doc_ids: Vec<String> = request.doc_scope.clone();
        if doc_ids.is_empty() {
            doc_ids = request
                .metadata
                .get("doc_ids")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
        }

        let args = if tool_id == "lexical_retrieval" {
            let terms = lexical_terms_from_query(query);
            let mut a = json!({ "terms": terms, "top_k": 10 });
            if !doc_ids.is_empty() {
                a["doc_scope"] = json!(doc_ids);
            }
            a
        } else {
            let mut a = json!({ "query": query, "top_k": 10 });
            if !doc_ids.is_empty() {
                a["doc_scope"] = json!(doc_ids);
            }
            a
        };

        let tool_result = match self.deps.dispatch_rag_fallback(auth, tool_id, args).await {
            Some(tr) => tr,
            None => ToolResult {
                tool: tool_id.into(),
                version: "1.0".into(),
                status: ToolStatus::Error,
                data: Some(json!({ "error": "rag runtime not available" })),
                trace: None,
            },
        };

        let evidence = evidence_from_dense_tool(&tool_result, alias_offset);
        let n = evidence.len();
        let tool_ok = count_tool_ok(std::slice::from_ref(&tool_result));

        let pack = EvidencePack {
            schema_version: "evidence_pack_v1".into(),
            sub_task_id: brief.sub_task.id.clone(),
            channel: "rag".into(),
            evidence,
            coverage: if n > 0 {
                Coverage::Partial
            } else {
                Coverage::Insufficient
            },
            gaps: if n == 0 {
                format!("rag_empty_or_unavailable ({tool_id})")
            } else {
                String::new()
            },
            tool_ok_count: 0,
        };
        let (pack, gate) = apply_pack_gate(pack, tool_ok, Some("rag"));
        let pack_json = serde_json::to_string_pretty(&pack).unwrap_or_else(|_| "{}".into());

        tracing::info!(
            channel = "rag",
            tool = tool_id,
            n_hits = n,
            elapsed_ms = started.elapsed().as_millis() as u64,
            coverage = pack.coverage.as_str(),
            pack_gate = gate.kind_str(),
            "lead_workers rag worker done"
        );

        WorkerOutcome {
            pack,
            pack_gate: gate,
            tool_results: vec![tool_result],
            observation: prompt_assets::evidence_pack_observation(&pack_json),
        }
    }
}

struct WorkerOutcome {
    pack: EvidencePack,
    pack_gate: PackGateOutcome,
    tool_results: Vec<ToolResult>,
    observation: String,
}

/// Brief rejected at the dispatch gate (never spawned a Worker).
#[derive(Debug, serde::Serialize)]
struct BriefReject {
    id: String,
    source: String,
    reason: String,
}

impl BriefReject {
    /// Single line for the `[brief_gate_rejects]` observation.
    fn line(&self) -> String {
        format!("- id={} source={} reason={}", self.id, self.source, self.reason)
    }
}

fn e2e_unlimited_budget() -> bool {
    match std::env::var("E2E_UNLIMITED_BUDGET") {
        Ok(v) => {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        }
        Err(_) => false,
    }
}

enum WebAttempt {
    Cancelled,
    Unavailable,
    Err(String),
    Ok(avrag_search::SearchResponse),
}

fn empty_panic_outcome(channel: &str, sub_task_id: &str) -> WorkerOutcome {
    let pack = EvidencePack {
        schema_version: "evidence_pack_v1".into(),
        sub_task_id: sub_task_id.into(),
        channel: channel.into(),
        evidence: vec![],
        coverage: Coverage::Insufficient,
        gaps: format!("{channel}_worker_panic"),
        tool_ok_count: 0,
    };
    let (pack, gate) = apply_pack_gate(pack, 0, Some(channel));
    let pack_json = serde_json::to_string_pretty(&pack).unwrap_or_else(|_| "{}".into());
    WorkerOutcome {
        pack,
        pack_gate: gate,
        tool_results: vec![],
        observation: prompt_assets::evidence_pack_observation(&pack_json),
    }
}

fn pack_gate_json(o: &WorkerOutcome) -> serde_json::Value {
    json!({
        "channel": o.pack.channel,
        "sub_task_id": o.pack.sub_task_id,
        "kind": o.pack_gate.kind_str(),
        "reasons": o.pack_gate.reasons_joined(),
        "coverage": o.pack.coverage.as_str(),
        "tool_ok_count": o.pack.tool_ok_count,
        "n_evidence": o.pack.evidence.len(),
    })
}

fn llm_usage_to_agent_run(u: &LlmUsage) -> crate::runtime::AgentRunUsage {
    crate::runtime::AgentRunUsage {
        provider: u.provider.clone(),
        model: u.model.clone(),
        prompt_tokens: u.prompt_tokens as u64,
        completion_tokens: u.completion_tokens as u64,
        total_tokens: u.total_tokens as u64,
        request_count: if u.total_tokens > 0 { 1 } else { 0 },
        cached_tokens: u.cached_tokens as u64,
        reasoning_tokens: u.reasoning_tokens as u64,
    }
}

fn apply_wave_outcomes(
    state: &mut IterationState,
    packs: &mut Vec<EvidencePack>,
    wave: Vec<WorkerOutcome>,
    wave_no: u8,
    run_log: &mut super::run_log::RunEventLog,
) {
    for outcome in wave {
        state.tool_results.extend(outcome.tool_results);
        state.messages.push(ChatMessage::user(outcome.observation));
        let channel = outcome.pack.channel.clone();
        if merge_or_push_pack(packs, outcome.pack) == MergeResult::Replaced {
            run_log.push(super::run_log::RunEventKind::PackSuperseded {
                wave: wave_no,
                channel,
            });
        }
    }
}

/// Record one wave into the run event log: brief rejects, per-call tool
/// traces (log-only), worker completions (surface), pack gate rewrites
/// (log-only, non-accept only).
fn log_wave_outcomes(
    run_log: &mut super::run_log::RunEventLog,
    wave_no: u8,
    briefs: &[TaskBrief],
    outcomes: &[WorkerOutcome],
    rejects: &[BriefReject],
) {
    use super::run_log::RunEventKind;
    for r in rejects {
        run_log.push(RunEventKind::BriefRejected {
            id: r.id.clone(),
            source: r.source.clone(),
            reason: r.reason.clone(),
        });
    }
    for o in outcomes {
        let channel = o.pack.channel.clone();
        for tr in &o.tool_results {
            let preview = if tr.status == ToolStatus::Ok {
                tr.trace
                    .as_ref()
                    .and_then(|t| t.degrade_reason.clone())
                    .unwrap_or_default()
            } else {
                tr.data
                    .as_ref()
                    .map(|d| d.to_string().chars().take(120).collect())
                    .unwrap_or_default()
            };
            run_log.push(RunEventKind::ToolCall {
                wave: wave_no,
                channel: channel.clone(),
                tool: tr.tool.clone(),
                ok: tr.status == ToolStatus::Ok,
                elapsed_ms: tr.trace.as_ref().and_then(|t| t.elapsed_ms),
                preview,
            });
        }
        let objective = briefs
            .iter()
            .flat_map(|b| b.sub_task.effective_facets())
            .find(|f| f.id == o.pack.sub_task_id)
            .map(|f| f.objective)
            .unwrap_or_default();
        run_log.push(RunEventKind::WorkerCompleted {
            wave: wave_no,
            sub_task_id: o.pack.sub_task_id.clone(),
            channel: channel.clone(),
            objective,
            n_evidence: o.pack.evidence.len(),
            gaps: o.pack.gaps.clone(),
        });
        if !matches!(o.pack_gate, PackGateOutcome::Accept) {
            run_log.push(RunEventKind::PackGated {
                wave: wave_no,
                channel,
                outcome: o.pack_gate.reasons_joined(),
            });
        }
    }
}

/// Outcome of merging a pack into the per-channel slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MergeResult {
    Pushed,
    KeptExisting,
    /// Later wave replaced the channel's earlier pack.
    Replaced,
}

/// Merge keyed by **sub_task_id** (facet-granular): packs from different
/// facets of the same channel coexist; a re-brief pack replaces its own
/// empty slot only.
fn merge_or_push_pack(packs: &mut Vec<EvidencePack>, new_pack: EvidencePack) -> MergeResult {
    if let Some(existing) = packs
        .iter_mut()
        .find(|p| p.sub_task_id == new_pack.sub_task_id)
    {
        let better = new_pack.evidence.len() > existing.evidence.len()
            || (new_pack.coverage.as_str() != "insufficient"
                && existing.coverage == Coverage::Insufficient);
        if better {
            *existing = new_pack;
            return MergeResult::Replaced;
        } else if !new_pack.evidence.is_empty() && existing.evidence.is_empty() {
            *existing = new_pack;
            return MergeResult::Replaced;
        }
        MergeResult::KeptExisting
        // else keep existing (wave0 may already have partial hits)
    } else {
        packs.push(new_pack);
        MergeResult::Pushed
    }
}

/// Ok-call counts per rag retrieval tool family across the waves so far.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PriorToolStats {
    dense: usize,
    lexical: usize,
    grep: usize,
}

/// Host-known fact: which rag tools already ran (Ok) in prior waves.
fn prior_tool_stats(tool_results: &[ToolResult]) -> PriorToolStats {
    let mut s = PriorToolStats::default();
    for tr in tool_results {
        if tr.status != ToolStatus::Ok {
            continue;
        }
        match tr.tool.as_str() {
            "dense_retrieval" => s.dense += 1,
            "lexical_retrieval" => s.lexical += 1,
            "doc_grep" => s.grep += 1,
            _ => {}
        }
    }
    s
}

impl PriorToolStats {
    fn render(&self) -> String {
        format!(
            "dense {} 次、lexical {} 次、grep {} 次",
            self.dense, self.lexical, self.grep
        )
    }
}

/// Structural tool choice for the host re-brief leaf: first **unused** tool in
/// preference order (dense covers natural-language sentences best, lexical
/// exact terms, grep literal patterns); dense when all were tried. Pure
/// function of host-known usage facts — no semantic judgment.
fn rebrief_tool_choice(stats: PriorToolStats) -> &'static str {
    if stats.dense == 0 {
        "dense_retrieval"
    } else if stats.lexical == 0 {
        "lexical_retrieval"
    } else if stats.grep == 0 {
        "doc_grep"
    } else {
        "dense_retrieval"
    }
}

/// Host structural re-brief (design W4 / D4 hard cap), facet-granular.
///
/// Only sub-tasks that **already produced a pack** and still have empty
/// evidence are re-briefed. Sub-tasks Lead intentionally omitted (no pack)
/// are **not** invented — fixes unwrap_or(true) forced-dispatch bug.
pub(super) fn packs_needing_rebrief(packs: &[EvidencePack]) -> Vec<String> {
    packs
        .iter()
        .filter(|p| p.evidence.is_empty() || p.coverage == Coverage::Insufficient)
        .map(|p| p.sub_task_id.clone())
        .collect()
}

/// Build re-brief briefs targeting exactly the empty facets. One output brief
/// per original brief, containing only its empty facets (ids preserved so the
/// follow-up pack **replaces** the empty one at merge).
fn host_rebrief_briefs(
    prior: &[EvidencePack],
    targets: &[String],
    originals: &[TaskBrief],
) -> Vec<TaskBrief> {
    let mut out = Vec::new();
    for ob in originals {
        let hit: Vec<crate::lead_workers::Facet> = ob
            .sub_task
            .effective_facets()
            .into_iter()
            .filter(|f| targets.iter().any(|t| t == &f.id))
            .collect();
        if hit.is_empty() {
            continue;
        }
        let gap = prior
            .iter()
            .find(|p| p.sub_task_id == hit[0].id)
            .map(|p| p.gaps.as_str())
            .unwrap_or("empty");
        let mut b = ob.clone();
        b.conversation_context_summary = format!("rebrief after: {gap}");
        if !ob.sub_task.facets.is_empty() {
            // Unscope facet ids ("{brief}/{facet}" → "{facet}") so the rebuilt
            // brief re-scopes to the same pack slot.
            let prefix = format!("{}/", ob.sub_task.id);
            b.sub_task.facets = hit
                .iter()
                .map(|f| crate::lead_workers::Facet {
                    id: f.id.strip_prefix(&prefix).unwrap_or(&f.id).to_string(),
                    objective: f.objective.clone(),
                })
                .collect();
        }
        out.push(b);
    }
    out
}

fn summarize_packs(packs: &[EvidencePack]) -> (String, String) {
    if packs.is_empty() {
        return ("no_packs".into(), "无通道 pack".into());
    }
    let cov = packs
        .iter()
        .map(|p| format!("{}={}", p.channel, p.coverage.as_str()))
        .collect::<Vec<_>>()
        .join("; ");
    let gaps = packs
        .iter()
        .filter(|p| !p.gaps.is_empty())
        .map(|p| format!("{}: {}", p.channel, p.gaps))
        .collect::<Vec<_>>();
    let gaps = if gaps.is_empty() {
        "（无明显缺口字段）".into()
    } else {
        gaps.join(" | ")
    };
    (cov, gaps)
}

const DEFAULT_BOUNDARIES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../prompts/workers/default-boundaries.md"
));
const DEFAULT_GROUNDING: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../prompts/workers/default-grounding.md"
));

fn host_default_briefs(query: &str, caps: ActivatedCaps) -> Vec<TaskBrief> {
    let q = query.trim();
    let boundaries = DEFAULT_BOUNDARIES.trim().to_string();
    let grounding = DEFAULT_GROUNDING.trim().to_string();
    let mut out = Vec::new();
    if caps.rag {
        out.push(TaskBrief {
            schema_version: "task_brief_v1".into(),
            original_query: q.into(),
            conversation_context_summary: String::new(),
            sub_task: SubTask {
                id: "t_rag".into(),
                objective: format!("从知识库检索：{q}"),
                boundaries: boundaries.clone(),
                preferred_source: PreferredSource::Rag,
                base_tool: String::new(),
                base_tool_arg: String::new(),
                facets: vec![],
                queries: vec![],
                max_steps: 4,
                success_criteria: "有可引用片段".into(),
            },
            output_schema: "evidence_pack_v1".into(),
            grounding_rule: grounding.clone(),
        });
    }
    if caps.search {
        let queries = vec![q.to_string()];
        out.push(TaskBrief {
            schema_version: "task_brief_v1".into(),
            original_query: q.into(),
            conversation_context_summary: String::new(),
            sub_task: SubTask {
                id: "t_web".into(),
                objective: format!("从网页检索：{q}"),
                boundaries,
                preferred_source: PreferredSource::Web,
                base_tool: String::new(),
                base_tool_arg: String::new(),
                facets: vec![],
                queries,
                max_steps: 1,
                success_criteria: "有带 URL 的 snippet".into(),
            },
            output_schema: "evidence_pack_v1".into(),
            grounding_rule: grounding,
        });
    }
    out
}

fn build_plan_context(request: &AgentRequest, caps: ActivatedCaps) -> LeadPlanContext {
    let workspace_id = request.workspace_id.clone().or_else(|| {
        request
            .metadata
            .get("workspace_id")
            .and_then(|v| v.as_str())
            .map(str::to_string)
    });

    let mut doc_scope: Vec<DocScopeSummary> = request
        .doc_scope
        .iter()
        .map(|s| DocScopeSummary {
            doc_id: s.clone(),
            title: String::new(),
            profile_line: String::new(),
        })
        .collect();
    if doc_scope.is_empty() {
        if let Some(arr) = request.metadata.get("doc_scope").and_then(|v| v.as_array()) {
            doc_scope = arr
                .iter()
                .filter_map(|item| {
                    if let Some(s) = item.as_str() {
                        return Some(DocScopeSummary {
                            doc_id: s.into(),
                            title: String::new(),
                            profile_line: String::new(),
                        });
                    }
                    let doc_id = item.get("doc_id")?.as_str()?.to_string();
                    let title = item
                        .get("title")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string();
                    let profile_line = item
                        .get("profile")
                        .or_else(|| item.get("profile_line"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string();
                    Some(DocScopeSummary {
                        doc_id,
                        title,
                        profile_line,
                    })
                })
                .collect();
        }
    }

    LeadPlanContext {
        caps_rag: caps.rag,
        caps_search: caps.search,
        doc_scope,
        workspace_id,
    }
}

// BM25 is AND semantics (lexical.rs: one zero-hit term zeroes the whole
// query), so a natural-language question must be cut down to content terms.
// Split on non-alphanumeric boundaries (CJK runs stay one term), drop a small
// English stopword list (case-insensitive match; original case kept — the pg
// 'simple' tsvector config folds case, and the CJK LIKE path is
// case-sensitive as written). If filtering empties the list, fall back to
// the whole trimmed query as one term.
fn lexical_terms_from_query(query: &str) -> Vec<String> {
    const STOPWORDS: &[&str] = &[
        "the", "a", "an", "of", "in", "on", "for", "to", "is", "are", "what", "which", "how",
        "does", "do", "say", "about", "and", "or",
    ];
    let q = query.trim();
    if q.is_empty() {
        return vec![" ".into()]; // will fail empty terms — use whole query
    }
    let terms: Vec<String> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .filter(|s| !STOPWORDS.contains(&s.to_lowercase().as_str()))
        .map(str::to_string)
        .collect();
    if terms.is_empty() {
        vec![q.to_string()]
    } else {
        terms
    }
}

fn evidence_from_tool_results(trs: &[ToolResult], start_alias: usize) -> Vec<EvidenceItem> {
    let mut evidence = Vec::new();
    // Alias numbering mirrors helpers::selected::alias_chunk_ids_in_order exactly:
    // every Ok, aliased-tool item with a non-empty chunk_id consumes one number
    // (even when its text is empty and yields no evidence item), so pack aliases
    // resolve to the same chunks at delivery. start_alias keeps waves continuous.
    let mut alias_i = start_alias;
    for tr in trs {
        if tr.status != ToolStatus::Ok
            || !crate::helpers::selected::ALIASED_TOOLS.contains(&tr.tool.as_str())
        {
            continue;
        }
        let Some(data) = tr.data.as_ref() else {
            continue;
        };
        let chunks = data
            .as_array()
            .or_else(|| data.get("chunks").and_then(|c| c.as_array()))
            .cloned()
            .unwrap_or_default();
        for c in chunks {
            let chunk_id = c
                .get("chunk_id")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            let alias = if chunk_id.is_empty() {
                String::new()
            } else {
                alias_i += 1;
                format!("#{alias_i}")
            };
            let text = c
                .get("text")
                .or_else(|| c.get("content"))
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            if text.trim().is_empty() {
                continue; // number already consumed above; empty text yields no evidence
            }
            let doc_id = c
                .get("doc_id")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            let source = if !doc_id.is_empty() {
                doc_id
            } else if !chunk_id.is_empty() {
                chunk_id.clone()
            } else {
                format!("chunk-{}", evidence.len() + 1)
            };
            let score = c.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0);
            evidence.push(EvidenceItem {
                content: text,
                source,
                score,
                provenance: chunk_id,
                alias,
            });
        }
    }
    evidence
}

fn evidence_from_dense_tool(tr: &ToolResult, start_alias: usize) -> Vec<EvidenceItem> {
    evidence_from_tool_results(std::slice::from_ref(tr), start_alias)
}

/// Web worker host leaf payload: a real [`avrag_search::SearchResponse`] so
/// `helpers::citations::build_search_citations_from_tool_results` deserializes
/// and `[[web:n]]` citations resolve at delivery (`lead_workers` tag kept).
fn web_search_tool_result(merged: &crate::lead_workers::MergedWebHits) -> ToolResult {
    let response = avrag_search::SearchResponse {
        query_type: "web".to_string(),
        sub_queries: merged.queries.clone(),
        results: merged
            .hits
            .iter()
            .map(|h| avrag_search::SearchResult {
                title: h.title.clone(),
                url: h.url.clone(),
                snippet: truncate_preview(&h.snippet, 800),
                citation_index: Some(h.web_index),
            })
            .collect(),
        synthesized_answer: String::new(),
        llm_usage: None,
    };
    let mut data = serde_json::to_value(&response).unwrap_or_else(|_| json!({}));
    data["lead_workers"] = json!(true);
    ToolResult {
        tool: "web_search".into(),
        version: "1.0".into(),
        status: ToolStatus::Ok,
        data: Some(data),
        trace: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_pack(channel: &str) -> EvidencePack {
        empty_pack_id(channel, "t")
    }

    fn empty_pack_id(channel: &str, sub_task_id: &str) -> EvidencePack {
        EvidencePack {
            schema_version: "evidence_pack_v1".into(),
            sub_task_id: sub_task_id.into(),
            channel: channel.into(),
            evidence: vec![],
            coverage: Coverage::Insufficient,
            gaps: format!("{channel}_empty"),
            tool_ok_count: 0,
        }
    }

    fn partial_pack(channel: &str) -> EvidencePack {
        partial_pack_id(channel, "t")
    }

    fn partial_pack_id(channel: &str, sub_task_id: &str) -> EvidencePack {
        EvidencePack {
            schema_version: "evidence_pack_v1".into(),
            sub_task_id: sub_task_id.into(),
            channel: channel.into(),
            evidence: vec![EvidenceItem {
                content: "hit".into(),
                source: "s1".into(),
                score: 0.5,
                provenance: String::new(),
                alias: "#1".into(),
            }],
            coverage: Coverage::Partial,
            gaps: String::new(),
            tool_ok_count: 1,
        }
    }

    #[test]
    fn evidence_from_tool_results_extracts_lexical_hit() {
        let tr = ToolResult {
            tool: "lexical_retrieval".into(),
            version: "1.0".into(),
            status: ToolStatus::Ok,
            data: Some(json!({
                "chunks": [{
                    "chunk_id": "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
                    "doc_id": "11111111-2222-3333-4444-555555555555",
                    "text": "Antifragility is beyond resilience or robustness.",
                    "score": 0.9
                }]
            })),
            trace: None,
        };
        let evidence = evidence_from_tool_results(std::slice::from_ref(&tr), 0);
        assert_eq!(evidence.len(), 1, "evidence: {evidence:?}");
        assert_eq!(evidence[0].source, "11111111-2222-3333-4444-555555555555");
        assert!(!evidence[0].alias.is_empty(), "alias must be assigned");
    }

    #[test]
    fn evidence_from_tool_results_skips_non_ok_or_empty_text() {
        let err_tr = ToolResult {
            tool: "lexical_retrieval".into(),
            version: "1.0".into(),
            status: ToolStatus::Error,
            data: Some(json!({ "chunks": [{ "chunk_id": "x", "text": "hi", "score": 0.5 }] })),
            trace: None,
        };
        assert!(evidence_from_tool_results(std::slice::from_ref(&err_tr), 0).is_empty());

        let empty_text = ToolResult {
            tool: "lexical_retrieval".into(),
            version: "1.0".into(),
            status: ToolStatus::Ok,
            data: Some(json!({ "chunks": [{ "chunk_id": "x", "text": "", "score": 0.5 }] })),
            trace: None,
        };
        assert!(evidence_from_tool_results(std::slice::from_ref(&empty_text), 0).is_empty());
    }

    #[test]
    fn dual_briefs_two_channels() {
        let b = host_default_briefs(
            "对比报告与行业实践",
            ActivatedCaps {
                rag: true,
                search: true,
            },
        );
        assert_eq!(b.len(), 2);
        assert!(b
            .iter()
            .any(|x| x.sub_task.preferred_source == PreferredSource::Rag));
        assert!(b
            .iter()
            .any(|x| x.sub_task.preferred_source == PreferredSource::Web));
    }

    #[test]
    fn rag_only_one_brief() {
        let b = host_default_briefs(
            "q",
            ActivatedCaps {
                rag: true,
                search: false,
            },
        );
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].sub_task.preferred_source, PreferredSource::Rag);
    }

    #[test]
    fn search_only_one_web_brief() {
        let b = host_default_briefs(
            "什么是 BYOK",
            ActivatedCaps {
                rag: false,
                search: true,
            },
        );
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].sub_task.preferred_source, PreferredSource::Web);
    }

    #[test]
    fn rebrief_when_empty_insufficient() {
        let packs = vec![empty_pack_id("rag", "t_rag"), empty_pack_id("web", "t_web")];
        let ids = packs_needing_rebrief(&packs);
        assert_eq!(ids, vec!["t_rag".to_string(), "t_web".to_string()]);
    }

    #[test]
    fn no_rebrief_when_partial_hits() {
        let packs = vec![partial_pack("rag"), partial_pack("web")];
        assert!(packs_needing_rebrief(&packs).is_empty());
    }

    #[test]
    fn rebrief_only_empty_facet() {
        // 同一通道两个 facet 的 pack：只补空的那一路。
        let packs = vec![partial_pack_id("rag", "t1/f1"), empty_pack_id("rag", "t1/f2")];
        assert_eq!(packs_needing_rebrief(&packs), vec!["t1/f2".to_string()]);
    }

    #[test]
    fn no_rebrief_for_missing_pack_lead_omitted_channel() {
        // Lead dispatched web only; host must not invent rag re-brief.
        let packs = vec![partial_pack("web")];
        assert!(packs_needing_rebrief(&packs).is_empty());
    }

    #[test]
    fn merge_pack_prefers_more_evidence() {
        let mut packs = vec![empty_pack("rag")];
        merge_or_push_pack(&mut packs, partial_pack("rag"));
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].evidence.len(), 1);
        assert_eq!(packs[0].coverage, Coverage::Partial);
    }

    #[test]
    fn merge_keeps_distinct_facets_of_same_channel() {
        let mut packs = vec![partial_pack_id("rag", "t1/f1")];
        merge_or_push_pack(&mut packs, partial_pack_id("rag", "t1/f2"));
        assert_eq!(packs.len(), 2, "facet packs must coexist");
    }

    #[test]
    fn rebrief_rebuilds_only_empty_facet_with_same_slot() {
        // 原始 brief：t1 带两个 facet；f2 空 → 补派 brief 只含 f2，且 scoped id 不变。
        let mut brief = host_default_briefs(
            "q",
            ActivatedCaps {
                rag: true,
                search: false,
            },
        )
        .remove(0);
        brief.sub_task.id = "t1".into();
        brief.sub_task.facets = vec![
            crate::lead_workers::Facet {
                id: "f1".into(),
                objective: "侧 A".into(),
            },
            crate::lead_workers::Facet {
                id: "f2".into(),
                objective: "侧 B".into(),
            },
        ];
        let prior = vec![
            partial_pack_id("rag", "t1/f1"),
            empty_pack_id("rag", "t1/f2"),
        ];
        let targets = packs_needing_rebrief(&prior);
        let briefs = host_rebrief_briefs(&prior, &targets, &[brief]);
        assert_eq!(briefs.len(), 1);
        assert_eq!(briefs[0].sub_task.facets.len(), 1);
        assert_eq!(briefs[0].sub_task.facets[0].id, "f2");
        // 重新 scope 后仍是 t1/f2 → merge 时替换空槽位。
        let eff = briefs[0].sub_task.effective_facets();
        assert_eq!(eff[0].id, "t1/f2");
    }

    #[test]
    fn rebrief_single_unit_clones_original_brief() {
        let brief = host_default_briefs(
            "q",
            ActivatedCaps {
                rag: false,
                search: true,
            },
        )
        .remove(0);
        let prior = vec![empty_pack_id("web", "t_web")];
        let targets = packs_needing_rebrief(&prior);
        let briefs = host_rebrief_briefs(&prior, &targets, std::slice::from_ref(&brief));
        assert_eq!(briefs.len(), 1);
        assert_eq!(briefs[0].sub_task.id, "t_web");
        assert!(
            validate_task_brief(
                &briefs[0],
                ActivatedCaps {
                    rag: false,
                    search: true
                }
            )
            .is_ok()
        );
    }

    #[test]
    fn rebrief_tool_choice_prefers_first_unused() {
        use contracts::ToolStatus;
        fn ok(tool: &str) -> ToolResult {
            ToolResult {
                tool: tool.into(),
                version: "1.0".into(),
                status: ToolStatus::Ok,
                data: None,
                trace: None,
            }
        }
        // 全未用 → dense
        assert_eq!(rebrief_tool_choice(prior_tool_stats(&[])), "dense_retrieval");
        // lexical 已用且空 → dense（q14 死路场景：不再复读 lexical）
        assert_eq!(
            rebrief_tool_choice(prior_tool_stats(&[ok("lexical_retrieval"), ok("lexical_retrieval")])),
            "dense_retrieval"
        );
        // dense 已用 → lexical
        assert_eq!(
            rebrief_tool_choice(prior_tool_stats(&[ok("dense_retrieval")])),
            "lexical_retrieval"
        );
        // dense+lexical 已用 → grep
        assert_eq!(
            rebrief_tool_choice(prior_tool_stats(&[ok("dense_retrieval"), ok("lexical_retrieval")])),
            "doc_grep"
        );
        // 全用过 → dense
        assert_eq!(
            rebrief_tool_choice(prior_tool_stats(&[
                ok("dense_retrieval"),
                ok("lexical_retrieval"),
                ok("doc_grep"),
            ])),
            "dense_retrieval"
        );
        // Error 不计入「已用」
        let mut err = ok("dense_retrieval");
        err.status = ToolStatus::Error;
        assert_eq!(rebrief_tool_choice(prior_tool_stats(&[err])), "dense_retrieval");
    }

    #[test]
    fn max_rebrief_waves_is_one() {        assert_eq!(MAX_REBRIEF_WAVES, 1);
    }

    #[test]
    fn lexical_terms_split() {
        assert_eq!(lexical_terms_from_query("foo bar"), vec!["foo", "bar"]);
        assert_eq!(lexical_terms_from_query("中文查询"), vec!["中文查询"]);
    }

    #[test]
    fn lexical_terms_drop_stopwords_from_natural_question() {
        let terms = lexical_terms_from_query("What does the deployment guide say about redis?");
        for t in ["deployment", "guide", "redis"] {
            assert!(terms.iter().any(|x| x == t), "missing {t}: {terms:?}");
        }
        for t in ["what", "What", "does", "the", "about"] {
            assert!(!terms.iter().any(|x| x == t), "kept {t}: {terms:?}");
        }
    }

    #[test]
    fn lexical_terms_all_stopwords_falls_back_to_whole_query() {
        assert_eq!(
            lexical_terms_from_query("what is the?"),
            vec!["what is the?"]
        );
    }

    #[test]
    fn evidence_alias_numbering_matches_delivery_replay() {
        let tr = ToolResult {
            tool: "dense_retrieval".into(),
            version: "1.0".into(),
            status: ToolStatus::Ok,
            data: Some(json!({"chunks": [
                {"chunk_id": "c1", "text": "alpha"},
                {"text": "orphan"},
                {"chunk_id": "c2", "text": ""},
                {"chunk_id": "c3", "text": "gamma"},
            ]})),
            trace: None,
        };
        let evidence = evidence_from_tool_results(std::slice::from_ref(&tr), 0);
        // orphan has text but no chunk_id → uncitable (no alias, no number consumed);
        // c2 consumes #2 but empty text yields no evidence item.
        let aliases: Vec<&str> = evidence.iter().map(|e| e.alias.as_str()).collect();
        assert_eq!(aliases, vec!["#1", "", "#3"]);
        assert_eq!(
            crate::helpers::selected::alias_chunk_ids_in_order(std::slice::from_ref(&tr)),
            vec!["c1", "c2", "c3"]
        );
        // Wave continuity: a re-brief wave starts after the 3 consumed numbers.
        let tr2 = ToolResult {
            tool: "dense_retrieval".into(),
            version: "1.0".into(),
            status: ToolStatus::Ok,
            data: Some(json!({"chunks": [{"chunk_id": "c4", "text": "delta"}]})),
            trace: None,
        };
        let evidence2 = evidence_from_tool_results(std::slice::from_ref(&tr2), 3);
        assert_eq!(evidence2[0].alias, "#4");
    }

    #[test]
    fn web_leaf_payload_deserializes_and_builds_citations() {
        let resp = |q: &str, title: &str, url: &str, snippet: &str| {
            (
                q.to_string(),
                avrag_search::SearchResponse {
                    query_type: "web".into(),
                    sub_queries: vec![q.into()],
                    results: vec![avrag_search::SearchResult {
                        title: title.into(),
                        url: url.into(),
                        snippet: snippet.into(),
                        citation_index: None,
                    }],
                    synthesized_answer: String::new(),
                    llm_usage: None,
                },
            )
        };
        let merged = merge_search_responses(
            &[
                resp("q1", "T1", "https://a.example/1", "snippet one"),
                resp("q2", "T2", "https://b.example/2", "snippet two"),
            ],
            80,
        );
        let tr = web_search_tool_result(&merged);
        assert!(
            serde_json::from_value::<avrag_search::SearchResponse>(tr.data.clone().unwrap()).is_ok()
        );
        let citations = crate::helpers::build_search_citations_from_tool_results(
            std::slice::from_ref(&tr),
        );
        assert_eq!(citations.len(), 2);
        assert_eq!(citations[0].citation_id, 1);
        assert_eq!(citations[1].citation_id, 2);
    }
}
