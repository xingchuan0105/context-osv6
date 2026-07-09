//! WriteRefine Agent Loop runner — the精修 sub-loop of the Write orchestrator.
//!
//! Replaces the legacy `heavytail::refine::refine()` fixed-round loop with a
//! ReAct loop where the LLM decides each round whether to `revise`, `research`,
//! or `finish`. The deterministic kernel (diagnosis, patch parse/apply, Band
//! recompute, best-version tracking) is reused unchanged.
//!
//! See `docs/plans/2026-07-07-write-refine-agent-loop.md`.

mod handlers;
mod helpers;
mod prompt;
pub mod types;
#[cfg(test)]
mod tests;

// Re-export the public API (backward-compatible with the old single-file module).
pub use types::{
    BestSnapshot, FinishReason, RefineContext, RefineLoopBudget,
    WRITE_REFINE_GATE_MAX_REVISE, WRITE_REFINE_HARD_REACT_CAP,
};

use std::path;

use avrag_llm::ChatMessage;
use common::AppError;
use contracts::chat::ToolStatus;
use heavytail::feedforward::fingerprint_workspace;
use heavytail::llm::WriterLlm;
use heavytail::score::composite;
use heavytail::state::{BestVersion, WriterState};
use heavytail::validator;
use heavytail::StyleParams;

use crate::agents::capability::CapabilityRegistry;
use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::r#loop::config::load_mode_config;
use crate::agents::r#loop::{build_assistant_message_with_tool_calls, build_tool_message};
use crate::agents::runtime::AgentRequest;
use crate::writer::invoker::SubagentInvoker;

/// The WriteRefine ReAct loop runner.
///
/// Owns the `WriterLlm` (refine phase), `SubagentInvoker` (for research), and
/// `CapabilityRegistry` (for tool spec resolution + system prompt assembly).
pub struct WriteRefineLoopRunner<'a> {
    llm: &'a WriterLlm,
    invoker: &'a SubagentInvoker,
    registry: &'static CapabilityRegistry,
    parent_request: &'a AgentRequest,
    style: StyleParams,
    budget: RefineLoopBudget,
}

impl<'a> WriteRefineLoopRunner<'a> {
    pub fn new(
        llm: &'a WriterLlm,
        invoker: &'a SubagentInvoker,
        parent_request: &'a AgentRequest,
        style: StyleParams,
        budget: RefineLoopBudget,
    ) -> Self {
        Self {
            llm,
            invoker,
            registry: CapabilityRegistry::standard_cached(),
            parent_request,
            style,
            budget,
        }
    }

    /// Run the WriteRefine ReAct loop.
    ///
    /// Replaces `heavytail::refine::refine(...)`. Mutates `state` in place
    /// (phase, rounds, best, tokens_used). Returns `Ok(())` on completion
    /// (soft or hard), `Err` on infra failure.
    pub async fn run(
        self,
        ctx: &mut RefineContext,
        reservoir: &[String],
        state: &mut WriterState,
        sink: &dyn AgentEventSink,
        job_dir: &path::Path,
    ) -> Result<(), AppError> {
        let mode = load_mode_config("write_refine")
            .map_err(|e| AppError::internal(format!("write_refine mode config load failed: {e}")))?;
        let temperature = mode.temperature.unwrap_or(0.4);
        let tools = mode.tools_for_retrieve(self.registry);

        // ── emit loop-start activity ───────────────────────────────────
        let _ = sink
            .emit(AgentEvent::Activity {
                stage: "refine".to_string(),
                message: "Starting WriteRefine loop".to_string(),
            })
            .await;

        // P2.7: best-effort refine checkpoint at loop entry.
        helpers::checkpoint_refine(ctx, job_dir);

        // ── per-iteration message history (system injected fresh each round) ──
        // The round messages are: [user_round_1, assistant_tool_call, tool_result,
        // user_round_2, ...]. The system prompt is rebuilt each round from the
        // ModeConfig (it includes the iteration budget hint + mandatory skills).
        let mut messages: Vec<ChatMessage> = Vec::new();

        // ── seed the best-version snapshot with the diagnosed initial draft ──
        // Guarantees the deliverable is never worse than the pre-refine draft
        // (plan §4.4). Updated on each effective revise; restored at loop exit.
        let init_fp = fingerprint_workspace(&ctx.workspace);
        let init_score = composite(&init_fp, &self.style).s;
        ctx.best_snapshot = Some(BestSnapshot {
            score: init_score,
            workspace: ctx.workspace.clone(),
        });

        let max_iter = {
            let configured = if self.budget.react_iterations_capped() {
                // P2.4: honor the mode's per-tier iteration budget
                // (write_refine.yaml budget.by_user_tier) instead of a hardcoded cap.
                // Capped by the runner's hard ceiling as a safety net.
                let tier_iter = mode
                    .budget
                    .resolve_max_iterations(self.parent_request.metadata.get("user_tier"));
                tier_iter.min(self.budget.max_react_iterations)
            } else {
                self.budget.max_react_iterations
            };
            configured.min(WRITE_REFINE_HARD_REACT_CAP)
        };
        // P2.6: consecutive no-tool-call rounds before forcing a soft exit.
        const NO_TOOL_FORCE_EXIT: u8 = 3;
        let mut no_tool_streak: u8 = 0;
        for iteration in 0..max_iter {
            ctx.react_iteration = iteration;
            state.phase = heavytail::state::WriterPhase::Refining {
                round: ctx.revise_rounds_used,
            };

            let is_last_round = iteration + 1 >= max_iter;
            let force_lexical_last_round =
                is_last_round && helpers::core_lexical_bands_unmet(&ctx.diagnosis.validation);
            let round_tools: Vec<_> = if force_lexical_last_round {
                tools
                    .iter()
                    .filter(|t| t.name == "write_refine_lexical")
                    .cloned()
                    .collect()
            } else {
                tools.clone()
            };

            // ── emit round activity ─────────────────────────────────────
            let _ = sink
                .emit(AgentEvent::Activity {
                    stage: "refine_round".to_string(),
                    message: format!(
                        "Refine round {}/{max_iter} (revise {})",
                        iteration + 1,
                        ctx.revise_rounds_used
                    ),
                })
                .await;

            // ── build the per-round user message: diagnosis brief + canonical + appendix ──
            let user_content = self.render_round_user_message(
                ctx,
                reservoir,
                iteration == 0,
                iteration,
                max_iter,
                force_lexical_last_round,
            );
            let system_content = self.build_system_prompt(
                &mode,
                iteration,
                max_iter,
                ctx.persona.as_ref(),
                ctx.revise_rounds_used,
                ctx.research_calls_used,
            );

            // Prepend a fresh system message each round (the diagnosis context
            // changes, so we cannot carry a stale system prompt).
            let mut round_messages = vec![ChatMessage::system(system_content)];
            round_messages.extend(messages.iter().cloned());
            round_messages.push(ChatMessage::user(user_content));

            // ── call the LLM with tool specs ────────────────────────────
            let (response, tokens) = self
                .llm
                .complete_with_tools(&round_messages, &round_tools, temperature)
                .await
                .map_err(|e| AppError::internal(format!("refine llm call failed: {e}")))?;
            ctx.tokens_used += tokens as usize;
            state.tokens_used += tokens as usize;

            // ── token-cap hard exit ─────────────────────────────────────
            if self.budget.tokens_capped() && ctx.tokens_used >= self.budget.max_refine_tokens {
                ctx.finish_reason = Some(FinishReason::TokenCap);
                break;
            }

            // ── parse the LLM output ────────────────────────────────────
            let tool_calls = response.tool_calls.clone().unwrap_or_default();
            if tool_calls.is_empty() {
                if force_lexical_last_round {
                    if let Some(call) = helpers::synthesize_force_lexical_call(ctx, reservoir) {
                        let result = self
                            .dispatch_tool_call(&call, ctx, reservoir, sink, state)
                            .await;
                        let _ = sink
                            .emit(AgentEvent::ToolResult {
                                tool: "write_refine_lexical".to_string(),
                                status: result.status,
                                data: result.data.clone(),
                                elapsed_ms: 0,
                            })
                            .await;
                        helpers::checkpoint_refine(ctx, job_dir);
                        break;
                    }
                }
                // P2.6: a round with no tool call makes no progress. Record the
                // assistant content, then nudge the model back to tool-use. After
                // `NO_TOOL_FORCE_EXIT` consecutive no-op rounds we stop wasting
                // budget and soft-exit (the best-version is still delivered).
                no_tool_streak = no_tool_streak.saturating_add(1);
                let content = response.content.clone();
                let reasoning = response.reasoning_content.clone();
                messages.push(ChatMessage::assistant(content));
                if let Some(r) = reasoning {
                    // Attach reasoning to the last assistant message.
                    if let Some(last) = messages.last_mut() {
                        last.reasoning_content = Some(r);
                    }
                }
                if self.budget.react_iterations_capped() && no_tool_streak >= NO_TOOL_FORCE_EXIT {
                    ctx.finish_reason = Some(FinishReason::IterationCap);
                    break;
                }
                // Inject a corrective user turn so the next LLM call is reminded
                // it MUST call one of the three tools.
                messages.push(ChatMessage::user(
                    if force_lexical_last_round {
                        "上一轮未调用 `write_refine_lexical`。本轮 hapax/zipf 仍未过关，\
                         **必须**调用 `write_refine_lexical`（优先 `repeat_term` 抬 hapax，\
                         或 `replace_term` 抬 zipf）。"
                            .to_string()
                    } else {
                        "上一轮未调用任何工具。精修阶段必须调用 `write_refine_revise`、\
                         `write_refine_lexical`、`write_refine_research` 或 `write_refine_finish` 之一；\
                         若已满意请调用 `write_refine_finish` 收工。"
                            .to_string()
                    },
                ));
                continue;
            }
            // The model called a tool — reset the no-op streak.
            no_tool_streak = 0;

            // ── build the assistant message carrying tool_calls ─────────
            let call_ids: Vec<String> = tool_calls
                .iter()
                .enumerate()
                .map(|(i, _)| format!("call_{i}"))
                .collect();
            let assistant_msg = build_assistant_message_with_tool_calls(
                &tool_calls,
                &call_ids,
                &response.content,
                response.reasoning_content.clone(),
            );
            messages.push(assistant_msg);

            // ── dispatch each tool call ──────────────────────────────────
            let mut should_break = false;
            for (call, call_id) in tool_calls.iter().zip(call_ids.iter()) {
                // P2.5: emit the model's tool invocation (paired with the
                // ToolResult below) so callers can observe what the LLM asked for.
                let _ = sink
                    .emit(AgentEvent::ToolCall {
                        tool: call.tool.clone(),
                        args: Some(call.args.clone()),
                    })
                    .await;
                let result = self
                    .dispatch_tool_call(call, ctx, reservoir, sink, state)
                    .await;
                // Emit a ToolResult event for observability.
                let _ = sink
                    .emit(AgentEvent::ToolResult {
                        tool: call.tool.clone(),
                        status: result.status,
                        data: result.data.clone(),
                        elapsed_ms: 0,
                    })
                    .await;
                // Build the tool-role message and append to history.
                let tool_msg = build_tool_message(call_id, &call.tool, &result);
                messages.push(tool_msg);

                // `finish` breaks the loop only on success; rejected finish (e.g. core
                // band gate) returns a tool error and the loop continues.
                if call.tool == "write_refine_finish" && result.status == ToolStatus::Ok {
                    should_break = true;
                    break;
                }
            }

            if should_break {
                ctx.finish_reason = Some(FinishReason::AgentFinish);
                break;
            }

            // Last-round lexical fallback: if hapax/zipf still fail after the LLM
            // turn, auto-apply a deterministic lexical op (user gate policy).
            if force_lexical_last_round
                && helpers::core_lexical_bands_unmet(&ctx.diagnosis.validation)
            {
                if let Some(call) = helpers::synthesize_force_lexical_call(ctx, reservoir) {
                    let result = self
                        .dispatch_tool_call(&call, ctx, reservoir, sink, state)
                        .await;
                    let _ = sink
                        .emit(AgentEvent::ToolResult {
                            tool: "write_refine_lexical".to_string(),
                            status: result.status,
                            data: result.data.clone(),
                            elapsed_ms: 0,
                        })
                        .await;
                }
            }

            // P2.7: best-effort refine checkpoint after each iteration.
            helpers::checkpoint_refine(ctx, job_dir);

            // ── revise-round cap hard exit ──────────────────────────────
            if self.budget.revise_rounds_capped()
                && ctx.revise_rounds_used >= self.budget.max_rounds
            {
                ctx.finish_reason = Some(FinishReason::ReviseRoundCap);
                break;
            }
        }

        // ── iteration cap soft exit ─────────────────────────────────────
        if ctx.finish_reason.is_none() {
            ctx.finish_reason = Some(FinishReason::IterationCap);
        }

        // Final lexical safety net: if hapax/zipf still fail at loop exit
        // (revise cap, iteration cap, or no-tool soft exit), auto-apply once.
        if helpers::core_lexical_bands_unmet(&ctx.diagnosis.validation) {
            if let Some(call) = helpers::synthesize_force_lexical_call(ctx, reservoir) {
                let result = self
                    .dispatch_tool_call(&call, ctx, reservoir, sink, state)
                    .await;
                let _ = sink
                    .emit(AgentEvent::ToolResult {
                        tool: "write_refine_lexical".to_string(),
                        status: result.status,
                        data: result.data.clone(),
                        elapsed_ms: 0,
                    })
                    .await;
            }
        }

        // ── best-version restore + state sync (plan §4.4 soft-exit invariant) ──
        // Prefer the current workspace when it improves core lexical bands (hapax/zipf)
        // or passes all bands, even if composite S is below the seeded initial draft.
        let cur_fp = fingerprint_workspace(&ctx.workspace);
        let _cur_validation = validator::validate(&cur_fp, &self.style);
        let cur_score = composite(&cur_fp, &self.style).s;
        let prefer_current = helpers::should_prefer_current_workspace(ctx, &self.style);
        let delivered_score = if prefer_current {
            cur_score
        } else if let Some(best) = ctx.best_snapshot.as_ref() {
            if best.score > cur_score {
                ctx.workspace = best.workspace.clone();
                best.score
            } else {
                cur_score
            }
        } else {
            cur_score
        };
        state.workspace = ctx.workspace.clone();
        let needs_best_update = state
            .best
            .as_ref()
            .is_none_or(|b| b.score < delivered_score);
        if needs_best_update {
            state.best = Some(BestVersion {
                round: state.rounds.len(),
                score: delivered_score,
                canonical_text: ctx.workspace.render_canonical(),
            });
        }
        let validation = validator::validate(
            &fingerprint_workspace(&ctx.workspace),
            &self.style,
        );

        // ── emit done activity ─────────────────────────────────────────
        let _ = sink
            .emit(AgentEvent::Activity {
                stage: "refine".to_string(),
                message: format!(
                    "WriteRefine finished: {:?} (bands_satisfied={})",
                    ctx.finish_reason, ctx.bands_satisfied
                ),
            })
            .await;

        let _ = validation; // (validation used by orchestrator post-refine)
        Ok(())
    }
}
