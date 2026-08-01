use avrag_llm::{ChatMessage, LlmUsage};
use common::AppError;
use contracts::ToolResult;

use super::assembler::{ContextAssembler, DisclosedState};
use super::config::{LoopExitConfig, ModeConfig};
use super::exit_policy::{SynthesisGate, decide_synthesis_gate, has_retrieval_observation};
use super::reasoning_emit;
use super::run_result::{RunContext, build_run_result};
use super::synthesis::SynthesisPhase;
use super::telemetry::ReActIterationRecord;
use super::{ReActLoop, truncate_preview};
use crate::events::{AgentEvent, AgentEventSink};
use crate::runtime::{AgentRequest, AgentRunResult, FinalDecision};

impl ReActLoop {
    pub(super) async fn resolve_synthesis_gate(
        &self,
        mode: &ModeConfig,
        loop_exit: &LoopExitConfig,
        request: &AgentRequest,
        auth: &contracts::auth_runtime::AuthContext,
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
        answer_deltas_streamed: bool,
    ) -> Result<Option<AgentRunResult>, AppError> {
        // auth / retrieval_query reserved for a future budget-path auto_fallback re-entry.
        let _ = auth;
        let has_evidence = has_retrieval_observation(messages, collected_tool_results, mode);

        match decide_synthesis_gate(
            loop_exit,
            has_evidence,
            direct_answer,
            collected_tool_results,
            retrieval_query,
        ) {
            SynthesisGate::SkipSynthesisUseDirect(answer) => {
                // Streaming request that did not live-stream retrieve tokens must
                // not dump a single MessageDelta — fall through to synthesis
                // (`run_prose_stream`) for true progressive tokens.
                if request.stream && !answer_deltas_streamed {
                    tracing::info!(
                        "stream request: skip direct dump, enter synthesis for live tokens"
                    );
                    // Drop the retrieve-phase prose so synthesis is not polluted.
                    if messages
                        .last()
                        .is_some_and(|m| m.role == "assistant" && m.content == answer)
                    {
                        messages.pop();
                    }
                } else {
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
                            answer_deltas_streamed,
                        )
                        .await?,
                    ));
                }
            }
            SynthesisGate::EnterSynthesis => {}
        }
        Ok(None)
    }

    pub(super) async fn finish_direct_answer_run(
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
        content_already_streamed: bool,
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
        // Avoid a second full-answer MessageDelta when retrieve already streamed
        // live tokens (acceptance A4: one compose-stage blob felt frozen).
        if !content_already_streamed {
            let _ = sink
                .emit(AgentEvent::MessageDelta {
                    text: answer.clone(),
                })
                .await;
        }
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

    pub(super) async fn run_synthesis_phase(
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
        budget_exhaustion: super::run_retrieval::BudgetExhaustion,
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
        // C5: a loop that exhausted its budget (rounds OR tokens) gets ONE
        // explicit final turn before synthesis: stop emitting code/retrieval
        // and produce the final output the task brief requires (the worker's
        // internal handoff JSON). Without it, exhausted workers reach the
        // prose-only stream with the retrieve-phase "output one <code> block"
        // framing still dominant and emit raw code or fabrications.
        let exhausted =
            budget_exhausted_messages(messages, budget_exhaustion, collected_tool_results);
        let messages = exhausted.as_deref().unwrap_or(messages);
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

        let disclosed_skills: Vec<String> = disclosed_state
            .disclosed_skill_ids
            .iter()
            .cloned()
            .collect();
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
    pub(super) async fn emit_run_citations(
        &self,
        sink: &dyn AgentEventSink,
        citations: &[contracts::chat::Citation],
    ) {
        if !citations.is_empty() {
            let _ = sink
                .emit(AgentEvent::Citations {
                    citations: citations.to_vec(),
                })
                .await;
        }
    }

    pub(super) async fn finish_run(
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
        let ctx = RunContext {
            iteration,
            max_iterations,
            total_tool_calls,
            telemetry_records,
            total_usage,
            reasoning_summary_acc,
            start_time,
        };
        let result = build_run_result(
            &self.llm,
            final_answer,
            request,
            collected_tool_results,
            &ctx,
            final_decision,
        );
        self.emit_run_citations(sink, &result.citations).await;
        Ok(result)
    }
}

/// Final-turn instruction appended when the retrieval loop exhausts its
/// budget (C5): stop emitting code blocks / new retrieval and produce the
/// final output the task brief requires. Generic on purpose —
/// the worker brief (app-chat) requires the internal_worker_handoff_v1 JSON,
/// chat modes require prose; this turn re-asserts "wrap up per brief" either
/// way.
/// Body: `prompts/loop/budget-exhausted-final.nudge.md` (rounds exhausted) or
/// `prompts/loop/budget-exhausted-final-tokens.nudge.md` (token-only).
pub(crate) fn budget_exhausted_final_turn(
    exhaustion: super::run_retrieval::BudgetExhaustion,
) -> &'static str {
    if exhaustion.tokens && !exhaustion.rounds {
        super::prompt_assets::budget_exhausted_final_turn_tokens()
    } else {
        super::prompt_assets::budget_exhausted_final_turn()
    }
}

/// Char budget for the last-tool-result carryover appended to the C5 turn —
/// enough for computed conclusions/numbers, bounded so a giant retrieval
/// payload can't swamp the final turn.
const C5_CARRYOVER_MAX_CHARS: usize = 2000;

/// C5: when the retrieval loop exhausted its budget (rounds or tokens),
/// append ONE final user turn carrying the budget-exhausted final nudge so
/// the synthesis call starts from an explicit handoff instruction instead of
/// the retrieve-phase framing. Returns `Some(new_messages)` when appended.
///
/// 2026-07-29：强制交接会丢掉收官轮刚算出的关键数字——把最后一次成功工具
/// 调用的原始结果确定性地拼进 C5 提示，要求原样带入最终输出。
fn budget_exhausted_messages(
    messages: &[ChatMessage],
    exhaustion: super::run_retrieval::BudgetExhaustion,
    tool_results: &[ToolResult],
) -> Option<Vec<ChatMessage>> {
    if !exhaustion.any() {
        return None;
    }
    let mut out = messages.to_vec();
    let mut turn = budget_exhausted_final_turn(exhaustion).to_string();
    if let Some(last) = tool_results
        .iter()
        .rev()
        .find(|r| r.status == contracts::ToolStatus::Ok && r.data.is_some())
    {
        let body = match last.data.as_ref().expect("data checked above") {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        let truncated = super::truncate_observation(&body, C5_CARRYOVER_MAX_CHARS);
        turn.push_str(&super::prompt_assets::budget_exhausted_carryover(
            &last.tool, &truncated,
        ));
    }
    out.push(ChatMessage::user(turn));
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::super::run_retrieval::BudgetExhaustion;
    use super::*;

    fn rounds_exhausted() -> BudgetExhaustion {
        BudgetExhaustion {
            rounds: true,
            tokens: false,
        }
    }

    fn tokens_exhausted() -> BudgetExhaustion {
        BudgetExhaustion {
            rounds: false,
            tokens: true,
        }
    }

    #[test]
    fn budget_exhaustion_appends_handoff_turn() {
        let history = vec![
            ChatMessage::system("retrieve with <code> blocks"),
            ChatMessage::user("question"),
        ];
        let out = budget_exhausted_messages(&history, rounds_exhausted(), &[])
            .expect("exhausted  appended");
        assert_eq!(out.len(), history.len() + 1);
        let last = out.last().unwrap();
        assert_eq!(last.role, "user");
        assert!(last.content.contains("迭代额度已用尽") || last.content.contains("迭代预算已用尽"));
        assert!(
            last.content.contains("不再产生新的代码块")
                || last.content.contains("不再发起新的检索")
        );
        assert!(
            last.content.contains("summary")
                || last.content.contains("coverage")
                || last.content.contains("结论散文")
        );
    }

    #[test]
    fn token_exhaustion_alone_appends_handoff_turn() {
        // F2: the C5 gate previously only watched the rounds ceiling; a loop
        // that burned its token budget first reached synthesis with no
        // closing observation at all.
        let history = vec![ChatMessage::user("question")];
        let out = budget_exhausted_messages(&history, tokens_exhausted(), &[])
            .expect("token-exhausted  appended");
        let last = &out.last().unwrap().content;
        assert!(last.contains("token"), "{last}");
        assert!(last.contains("不再产生新的代码块"), "{last}");
        // Token-only exhaustion states the token fact, not the rounds fact.
        assert!(!last.contains("迭代额度已用尽"), "{last}");
    }

    #[test]
    fn rounds_and_tokens_exhaustion_uses_rounds_turn() {
        let history = vec![ChatMessage::user("question")];
        let both = BudgetExhaustion {
            rounds: true,
            tokens: true,
        };
        let out = budget_exhausted_messages(&history, both, &[]).expect("appended");
        let last = &out.last().unwrap().content;
        assert!(last.contains("迭代额度已用尽"), "{last}");
    }

    #[test]
    fn no_final_turn_before_budget_exhaustion() {
        let history = vec![ChatMessage::user("question")];
        assert!(budget_exhausted_messages(&history, BudgetExhaustion::default(), &[]).is_none());
        assert!(budget_exhausted_messages(&history, rounds_exhausted(), &[]).is_some());
    }

    #[test]
    fn c5_carries_last_ok_tool_result_into_the_turn() {
        // Synthetic payload only — no realistic-corpus strings.
        let history = vec![ChatMessage::user("question")];
        let results = vec![
            ToolResult {
                tool: "dense_retrieval".into(),
                version: "1".into(),
                status: contracts::ToolStatus::Ok,
                data: Some(serde_json::json!([{"chunk_id": "c1", "text": "background"}])),
                trace: None,
            },
            ToolResult {
                tool: "code_execution".into(),
                version: "1".into(),
                status: contracts::ToolStatus::Ok,
                data: Some(serde_json::json!({"stdout": "total_count=12"})),
                trace: None,
            },
        ];
        let out =
            budget_exhausted_messages(&history, rounds_exhausted(), &results).expect("appended");
        let last = &out.last().unwrap().content;
        assert!(last.contains("code_execution"), "{last}");
        assert!(last.contains("total_count=12"), "{last}");
        assert!(last.contains("原始结果") || last.contains("观察"), "{last}");
        // Picks the LAST Ok result, not an earlier one.
        assert!(!last.contains("dense_retrieval"), "{last}");
    }

    #[test]
    fn c5_skips_failed_or_dataless_results_for_carryover() {
        let history = vec![ChatMessage::user("question")];
        let results = vec![
            ToolResult {
                tool: "code_execution".into(),
                version: "1".into(),
                status: contracts::ToolStatus::Ok,
                data: Some(serde_json::json!("答案 42")),
                trace: None,
            },
            ToolResult {
                tool: "dense_retrieval".into(),
                version: "1".into(),
                status: contracts::ToolStatus::Error,
                data: Some(serde_json::json!({"error": "boom"})),
                trace: None,
            },
            ToolResult {
                tool: "doc_scan".into(),
                version: "1".into(),
                status: contracts::ToolStatus::Ok,
                data: None,
                trace: None,
            },
        ];
        let out =
            budget_exhausted_messages(&history, rounds_exhausted(), &results).expect("appended");
        let last = &out.last().unwrap().content;
        // Walks back to the most recent Ok-with-data result.
        assert!(last.contains("答案 42"), "{last}");
        assert!(!last.contains("boom"), "{last}");
        // No tool results at all → plain C5 turn, no carryover block.
        let plain = budget_exhausted_messages(&history, rounds_exhausted(), &[]).expect("appended");
        assert!(!plain.last().unwrap().content.contains("原始结果如下"));
    }
}
