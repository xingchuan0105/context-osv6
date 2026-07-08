use std::time::Instant;

use avrag_llm::{LlmClient, LlmUsage};
use contracts::ToolResult;

use crate::agents::runtime::{
    AgentRequest, AgentRunResult, AgentRunUsage, BudgetUsage, EvaluationSignals, FinalDecision,
    IterationRecord,
};

use super::telemetry::ReActIterationRecord;

pub struct RunContext<'a> {
    pub iteration: u8,
    pub max_iterations: u8,
    pub total_tool_calls: u32,
    pub telemetry_records: &'a [ReActIterationRecord],
    pub total_usage: &'a LlmUsage,
    pub reasoning_summary_acc: &'a str,
    pub start_time: Instant,
}

pub fn build_run_result(
    llm: &LlmClient,
    final_answer: String,
    request: &AgentRequest,
    collected_tool_results: &[ToolResult],
    ctx: &RunContext<'_>,
    final_decision: Option<FinalDecision>,
) -> AgentRunResult {
    let total_elapsed_ms = ctx.start_time.elapsed().as_millis() as u64;
    let citations = crate::agents::unified::helpers::build_all_citations_from_tool_results(
        collected_tool_results,
    );
    let citations = crate::agents::unified::helpers::filter_citations_for_mode(
        &request.kind.as_canonical_str(),
        &final_answer,
        citations,
    );
    let sources =
        crate::agents::unified::helpers::build_sources_from_tool_results(collected_tool_results);
    let degrade_trace =
        crate::agents::unified::helpers::degrade_trace_from_tool_results(collected_tool_results);

    AgentRunResult {
        answer: final_answer,
        answer_blocks: Vec::new(),
        citations,
        sources,
        reasoning_summary: if ctx.reasoning_summary_acc.is_empty() {
            None
        } else {
            Some(ctx.reasoning_summary_acc.to_string())
        },
        degrade_trace,
        usage: Some(AgentRunUsage {
            provider: if ctx.total_usage.provider.is_empty() {
                llm.config.provider_name()
            } else {
                ctx.total_usage.provider.clone()
            },
            model: if ctx.total_usage.model.is_empty() {
                llm.config.model.clone()
            } else {
                ctx.total_usage.model.clone()
            },
            prompt_tokens: ctx.total_usage.prompt_tokens as u64,
            completion_tokens: ctx.total_usage.completion_tokens as u64,
            total_tokens: ctx.total_usage.total_tokens as u64,
            request_count: ctx.telemetry_records.len() as u64,
            cached_tokens: ctx.total_usage.cached_tokens as u64,
        }),
        debug_payload: None,
        message_id: None,
        iterations: ctx.telemetry_records
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
        total_tool_calls: ctx.total_tool_calls,
        tool_results: collected_tool_results.to_vec(),
        final_decision,
        trace_id: request.session_id.clone(),
        budget_used: Some(BudgetUsage {
            current: ctx.iteration,
            max: ctx.max_iterations,
        }),
        total_elapsed_ms: Some(total_elapsed_ms),
        routing_decision: None,
    }
}
