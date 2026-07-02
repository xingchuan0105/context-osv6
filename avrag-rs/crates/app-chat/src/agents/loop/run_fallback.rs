use avrag_llm::{ChatMessage, LlmUsage};
use common::AppError;
use contracts::ToolResult;

use super::assembler::DisclosedState;
use super::config::{AutoFallbackConfig, LoopExitConfig, ModeConfig};
use super::exit_policy::{
    PostLoopAction, degraded_no_evidence_answer, has_retrieval_observation, post_fallback_gate,
};
use super::reasoning_emit;
use super::run_result::build_run_result;
use super::telemetry::ReActIterationRecord;
use super::{ReActLoop, fallback, truncate_preview};
use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::runtime::{AgentRequest, AgentRunResult, FinalDecision};
use super::cancellation::DegradeReason;

impl ReActLoop {
    pub(super) async fn trigger_auto_fallback_and_check_degraded(
        &self,
        mode: &ModeConfig,
        loop_exit: &LoopExitConfig,
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
        let has_evidence = has_retrieval_observation(messages, collected_tool_results, mode);
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

    pub(super) async fn finish_degraded_no_evidence_run(
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
                reason: DegradeReason::NoResultsAfterAllFallbacks,
            }),
        );
        result
            .degrade_trace
            .push(contracts::chat::DegradeTraceItem {
                stage: "degraded_no_evidence".to_string(),
                reason: DegradeReason::NoRetrievalEvidence,
                impact: "Answer withheld; synthesis skipped".to_string(),
            });
        self.emit_run_citations(sink, &result.citations).await;
        Ok(result)
    }

    pub(super) async fn run_auto_fallback(
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

    pub(super) async fn run_rag_retrieval_fallback(
        &self,
        request: &AgentRequest,
        auth: &avrag_auth::AuthContext,
        retrieval_query: &str,
        fallback: &AutoFallbackConfig,
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
        let result =
            fallback::inject_fallback_observation(runtime, auth, args, &fallback.tool_id, messages)
                .await;
        collected_tool_results.push(result);
        Ok(())
    }

    pub(super) async fn run_web_search_fallback(
        &self,
        retrieval_query: &str,
        fallback: &AutoFallbackConfig,
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

    pub(super) async fn emit_unknown_fallback_skipped(
        &self,
        sink: &dyn AgentEventSink,
        tool_id: &str,
    ) {
        let _ = sink
            .emit(AgentEvent::Activity {
                stage: "fallback_skipped".to_string(),
                message: format!("unknown fallback tool_id: {tool_id}"),
            })
            .await;
    }
}
