//! WriteRefine Agent Loop runner — domain side (ADR 0006).
//!
//! Agent-coupled adapters (research workers, ModeConfig, AgentEventSink) live in
//! app-chat and implement [`crate::ports`] traits.

mod handlers;
mod prompt;

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

use crate::message_format::{build_assistant_message_with_tool_calls, build_tool_message};
use crate::ports::{WriteActivitySink, WriteParentMeta, WriteRefineModeHost, WriteResearchPort};
use crate::refine_helpers;
use crate::refine_types::{
    BestSnapshot, FinishReason, RefineContext, RefineLoopBudget, WRITE_REFINE_HARD_REACT_CAP,
};

/// The WriteRefine ReAct loop runner (ports for research / mode / activity).
pub struct WriteRefineLoopRunner<'a> {
    llm: &'a WriterLlm,
    research: &'a dyn WriteResearchPort,
    mode: &'a dyn WriteRefineModeHost,
    parent: WriteParentMeta,
    style: StyleParams,
    budget: RefineLoopBudget,
}

impl<'a> WriteRefineLoopRunner<'a> {
    pub fn new(
        llm: &'a WriterLlm,
        research: &'a dyn WriteResearchPort,
        mode: &'a dyn WriteRefineModeHost,
        parent: WriteParentMeta,
        style: StyleParams,
        budget: RefineLoopBudget,
    ) -> Self {
        Self {
            llm,
            research,
            mode,
            parent,
            style,
            budget,
        }
    }

    /// Run the WriteRefine ReAct loop.
    pub async fn run(
        self,
        ctx: &mut RefineContext,
        reservoir: &[String],
        state: &mut WriterState,
        sink: &dyn WriteActivitySink,
        job_dir: &path::Path,
    ) -> Result<(), AppError> {
        let temperature = self.mode.temperature();
        let tools = self.mode.tool_specs();

        sink.activity("refine", "Starting WriteRefine loop".to_string())
            .await;

        refine_helpers::checkpoint_refine(ctx, job_dir);

        let mut messages: Vec<ChatMessage> = Vec::new();

        let init_fp = fingerprint_workspace(&ctx.workspace);
        let init_score = composite(&init_fp, &self.style).s;
        ctx.best_snapshot = Some(BestSnapshot {
            score: init_score,
            workspace: ctx.workspace.clone(),
        });

        let max_iter = {
            let configured = if self.budget.react_iterations_capped() {
                let tier = self.parent.user_tier.as_deref();
                self.mode
                    .max_react_iterations(tier, self.budget.max_react_iterations)
            } else {
                self.budget.max_react_iterations
            };
            configured.min(WRITE_REFINE_HARD_REACT_CAP)
        };
        const NO_TOOL_FORCE_EXIT: u8 = 3;
        let mut no_tool_streak: u8 = 0;
        for iteration in 0..max_iter {
            ctx.react_iteration = iteration;
            state.phase = heavytail::state::WriterPhase::Refining {
                round: ctx.revise_rounds_used,
            };

            let is_last_round = iteration + 1 >= max_iter;
            let force_lexical_last_round =
                is_last_round && refine_helpers::core_lexical_bands_unmet(&ctx.diagnosis.validation);
            let round_tools: Vec<_> = if force_lexical_last_round {
                tools
                    .iter()
                    .filter(|t| t.name == "write_refine_lexical")
                    .cloned()
                    .collect()
            } else {
                tools.clone()
            };

            sink.activity(
                "refine_round",
                format!(
                    "Refine round {}/{max_iter} (revise {})",
                    iteration + 1,
                    ctx.revise_rounds_used
                ),
            )
            .await;

            let user_content = self.render_round_user_message(
                ctx,
                reservoir,
                iteration == 0,
                iteration,
                max_iter,
                force_lexical_last_round,
            );
            let system_content = self.mode.system_prompt(
                iteration,
                max_iter,
                ctx.persona.as_ref(),
                ctx.revise_rounds_used,
                ctx.research_calls_used,
                &self.budget,
            );

            let mut round_messages = vec![ChatMessage::system(system_content)];
            round_messages.extend(messages.iter().cloned());
            round_messages.push(ChatMessage::user(user_content));

            let (response, tokens) = self
                .llm
                .complete_with_tools(&round_messages, &round_tools, temperature)
                .await
                .map_err(|e| AppError::internal(format!("refine llm call failed: {e}")))?;
            ctx.tokens_used += tokens as usize;
            state.tokens_used += tokens as usize;

            if self.budget.tokens_capped() && ctx.tokens_used >= self.budget.max_refine_tokens {
                ctx.finish_reason = Some(FinishReason::TokenCap);
                break;
            }

            let tool_calls = response.tool_calls.clone().unwrap_or_default();
            if tool_calls.is_empty() {
                if force_lexical_last_round {
                    if let Some(call) = refine_helpers::synthesize_force_lexical_call(ctx, reservoir)
                    {
                        let result = self
                            .dispatch_tool_call(&call, ctx, reservoir, sink, state)
                            .await;
                        sink.tool_result(
                            "write_refine_lexical",
                            result.status,
                            result.data.clone(),
                        )
                        .await;
                        refine_helpers::checkpoint_refine(ctx, job_dir);
                        break;
                    }
                }
                no_tool_streak = no_tool_streak.saturating_add(1);
                let content = response.content.clone();
                let reasoning = response.reasoning_content.clone();
                messages.push(ChatMessage::assistant(content));
                if let Some(r) = reasoning {
                    if let Some(last) = messages.last_mut() {
                        last.reasoning_content = Some(r);
                    }
                }
                if self.budget.react_iterations_capped() && no_tool_streak >= NO_TOOL_FORCE_EXIT {
                    ctx.finish_reason = Some(FinishReason::IterationCap);
                    break;
                }
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
            no_tool_streak = 0;

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

            let mut should_break = false;
            for (call, call_id) in tool_calls.iter().zip(call_ids.iter()) {
                sink.tool_call(call.tool.as_str(), Some(call.args.clone()))
                    .await;
                let result = self
                    .dispatch_tool_call(call, ctx, reservoir, sink, state)
                    .await;
                sink.tool_result(call.tool.as_str(), result.status, result.data.clone())
                    .await;
                let tool_msg = build_tool_message(call_id, &call.tool, &result);
                messages.push(tool_msg);

                if call.tool == "write_refine_finish" && result.status == ToolStatus::Ok {
                    should_break = true;
                    break;
                }
            }

            if should_break {
                ctx.finish_reason = Some(FinishReason::AgentFinish);
                break;
            }

            if force_lexical_last_round
                && refine_helpers::core_lexical_bands_unmet(&ctx.diagnosis.validation)
            {
                if let Some(call) = refine_helpers::synthesize_force_lexical_call(ctx, reservoir) {
                    let result = self
                        .dispatch_tool_call(&call, ctx, reservoir, sink, state)
                        .await;
                    sink.tool_result(
                        "write_refine_lexical",
                        result.status,
                        result.data.clone(),
                    )
                    .await;
                }
            }

            refine_helpers::checkpoint_refine(ctx, job_dir);

            if self.budget.revise_rounds_capped()
                && ctx.revise_rounds_used >= self.budget.max_rounds
            {
                ctx.finish_reason = Some(FinishReason::ReviseRoundCap);
                break;
            }
        }

        if ctx.finish_reason.is_none() {
            ctx.finish_reason = Some(FinishReason::IterationCap);
        }

        if refine_helpers::core_lexical_bands_unmet(&ctx.diagnosis.validation) {
            if let Some(call) = refine_helpers::synthesize_force_lexical_call(ctx, reservoir) {
                let result = self
                    .dispatch_tool_call(&call, ctx, reservoir, sink, state)
                    .await;
                sink.tool_result(
                    "write_refine_lexical",
                    result.status,
                    result.data.clone(),
                )
                .await;
            }
        }

        let cur_fp = fingerprint_workspace(&ctx.workspace);
        let _cur_validation = validator::validate(&cur_fp, &self.style);
        let cur_score = composite(&cur_fp, &self.style).s;
        let prefer_current = refine_helpers::should_prefer_current_workspace(ctx, &self.style);
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
        let _validation = validator::validate(
            &fingerprint_workspace(&ctx.workspace),
            &self.style,
        );

        sink.activity(
            "refine",
            format!(
                "WriteRefine finished: {:?} (bands_satisfied={})",
                ctx.finish_reason, ctx.bands_satisfied
            ),
        )
        .await;

        Ok(())
    }
}
