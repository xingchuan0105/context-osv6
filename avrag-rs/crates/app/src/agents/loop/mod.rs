use std::sync::Arc;

pub mod config;
pub mod parse;
pub mod skills;
pub mod fallback;
pub mod synthesis;
pub mod telemetry;

use crate::agents::capability::{CapabilityRegistry, SkillMetadata};
use crate::agents::events::{AgentEvent, AgentEventSink};
use config::ModeConfig;
use parse::{parse_llm_output, LlmOutput};
use skills::SkillDisclosure;
use synthesis::SynthesisPhase;
use telemetry::ReActIterationRecord;
use crate::agents::runtime::{
    AgentRequest, AgentRunResult, AgentRunUsage, BudgetUsage, FinalDecision, IterationRecord,
};
use crate::agents::evaluator::EvaluationSignals;
use avrag_llm::{ChatMessage, LlmClient, LlmUsage};
use common::{AppError, ToolResult};



fn merge_request_doc_scope(call: &mut common::ToolCall, doc_scope: &[String]) {
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
        args.insert(
            "doc_scope".to_string(),
            serde_json::json!(doc_scope),
        );
    }
}

async fn dispatch_rag_tool(
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
    code_interpreter: Arc<std::sync::Mutex<Option<avrag_code_interpreter::CodeInterpreter>>>,
}

impl ReActLoop {
    pub fn new(llm: Arc<LlmClient>, skill_registry: Arc<CapabilityRegistry>) -> Self {
        Self {
            llm,
            skill_registry,
            rag_runtime: None,
            search_executor: None,
            code_interpreter: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub fn with_rag_runtime(
        mut self,
        runtime: Option<Arc<avrag_rag_core::RagRuntime>>,
    ) -> Self {
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

        let base_prompt = config::load_system_prompt(&mode.system_prompt_base)?;

        let mut messages: Vec<ChatMessage> = Vec::new();
        messages.push(ChatMessage::system(base_prompt.clone()));

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

        messages.push(ChatMessage::user(&request.query));

        // base_message_count = system prompt + conversation history + user query.
        // ReAct steps are appended after this. Truncation must never touch
        // the base conversation — only intermediate ReAct rounds.
        let base_message_count = messages.len();

        let max_iterations = request
            .max_iterations
            .unwrap_or_else(|| mode.budget.resolve_max_iterations(request.metadata.get("user_tier")))
            .max(1);

        let auth: avrag_auth::AuthContext =
            serde_json::from_value(request.auth_context.clone())
                .map_err(|e| AppError::internal(format!("invalid auth context: {e}")))?;

        let mut disclosed_skills: Vec<SkillMetadata> = vec![];
        let mut iteration: u8 = 0;
        let mut consecutive_sandbox_errors: u8 = 0;
        let mut telemetry_records: Vec<ReActIterationRecord> = vec![];
        let mut total_usage = LlmUsage::zeroed();
        let mut total_tool_calls: u32 = 0;
        let mut collected_tool_results: Vec<ToolResult> = Vec::new();

        loop {
            if cancel.is_cancelled() {
                break;
            }

            if iteration >= max_iterations {
                let _ = sink
                    .emit(AgentEvent::Activity {
                        stage: "budget_exhausted".to_string(),
                        message: "iteration budget exhausted, proceeding to synthesis".to_string(),
                    })
                    .await;
                break;
            }

            let disclosure = SkillDisclosure;
            let new_skills = disclosure.progressive_disclose(
                &mode.disclosure,
                &mode.skill_catalog,
                &self.skill_registry,
                &messages,
                iteration,
                &disclosed_skills,
            );

            disclosed_skills.extend(new_skills);

            let skills_text = if disclosed_skills.is_empty() {
                String::new()
            } else {
                let mut text = String::from("\n\n<available_skills>\n");
                for skill in &disclosed_skills {
                    text.push_str(&format!("- {}: {}\n", skill.id, skill.description));
                }
                text.push_str("</available_skills>");
                text
            };

            let system_content = format!("{}{}", base_prompt, skills_text);
            let mut round_messages: Vec<ChatMessage> = Vec::new();
            round_messages.push(ChatMessage::system(system_content));

            // Append all non-system messages from conversation history.
            for msg in &messages {
                if msg.role != "system" {
                    round_messages.push(msg.clone());
                }
            }

            let iter_start = std::time::Instant::now();
            let temperature = mode.temperature.unwrap_or(0.7);
            let llm_response = self
                .llm
                .complete_with_tools(&round_messages, &mode.native_tools, Some(temperature))
                .await
                .map_err(|e| AppError::internal(format!("llm completion failed: {e}")))?;

            total_usage.accumulate(&llm_response.usage);

            let parsed = parse_llm_output(&llm_response);

            match parsed {
                LlmOutput::NativeToolCalls(calls) => {
                    let call_ids: Vec<String> = (0..calls.len())
                        .map(|i| format!("call_{}", i))
                        .collect();

                    let mut tool_messages: Vec<ChatMessage> = Vec::new();
                    for (idx, call) in calls.iter().enumerate() {
                        let call_id = &call_ids[idx];
                        let tool_start = std::time::Instant::now();
                        let result = match call.tool.as_str() {
                            "dense_retrieval" | "lexical_retrieval" | "graph_retrieval"
                            | "index_lookup" | "doc_summary" | "doc_metadata" => {
                                if let Some(runtime) = &self.rag_runtime {
                                    dispatch_rag_tool(runtime, &auth, call, &request.doc_scope)
                                        .await
                                } else {
                                    common::ToolResult {
                                        tool: call.tool.clone(),
                                        version: call.version.clone(),
                                        status: common::ToolStatus::NotImplemented,
                                        data: Some(
                                            serde_json::json!({"error": "rag runtime not configured"}),
                                        ),
                                        trace: None,
                                    }
                                }
                            }
                            "web_search" => {
                                if let Some(executor) = &self.search_executor {
                                    let query = call.args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                                    let vertical = call.args.get("vertical").and_then(|v| v.as_str());
                                    let v = vertical.unwrap_or("web");
                                    if v != "web" && v != "news" {
                                        common::ToolResult {
                                            tool: call.tool.clone(),
                                            version: call.version.clone(),
                                            status: common::ToolStatus::Error,
                                            data: Some(serde_json::json!({"error": format!("unsupported vertical: {v}. allowed: web, news")})),
                                            trace: None,
                                        }
                                    } else {
                                        match executor.execute_search(query, Some(v)).await {
                                            Ok(response) => common::ToolResult {
                                                tool: call.tool.clone(),
                                                version: call.version.clone(),
                                                status: common::ToolStatus::Ok,
                                                data: Some(serde_json::to_value(&response).unwrap_or_default()),
                                                trace: None,
                                            },
                                            Err(e) => common::ToolResult {
                                                tool: call.tool.clone(),
                                                version: call.version.clone(),
                                                status: common::ToolStatus::Error,
                                                data: Some(serde_json::json!({"error": e.to_string()})),
                                                trace: None,
                                            }
                                        }
                                    }
                                } else {
                                    common::ToolResult {
                                        tool: call.tool.clone(),
                                        version: call.version.clone(),
                                        status: common::ToolStatus::NotImplemented,
                                        data: Some(serde_json::json!({"error": "search executor not configured"})),
                                        trace: None,
                                    }
                                }
                            }
                            _ => common::ToolResult {
                                tool: call.tool.clone(),
                                version: call.version.clone(),
                                status: common::ToolStatus::NotImplemented,
                                data: None,
                                trace: None,
                            },
                        };
                        let tool_elapsed_ms = tool_start.elapsed().as_millis() as u64;

                        let _ = sink
                            .emit(AgentEvent::ToolResult {
                                tool: call.tool.clone(),
                                status: result.status.clone(),
                                data: result.data.clone(),
                                elapsed_ms: tool_elapsed_ms,
                            })
                            .await;

                        tool_messages.push(build_tool_message(
                            call_id,
                            &call.tool,
                            &result,
                        ));
                        collected_tool_results.push(result);
                    }

                    messages.push(build_assistant_message_with_tool_calls(
                        &calls,
                        &call_ids,
                        &llm_response.content,
                        llm_response.reasoning_content.clone(),
                    ));

                    for tm in tool_messages {
                        messages.push(tm);
                    }

                    total_tool_calls += calls.len() as u32;
                    consecutive_sandbox_errors = 0;

                    telemetry_records.push(ReActIterationRecord {
                        iteration,
                        disclosed_skills: disclosed_skills.iter().map(|s| s.id.clone()).collect(),
                        action_type: "native_tool_call".to_string(),
                        observation_preview: format!("{} tool calls", calls.len()),
                        llm_usage: Some(AgentRunUsage {
                            provider: llm_response.usage.provider.clone(),
                            model: llm_response.model.clone(),
                            prompt_tokens: llm_response.usage.prompt_tokens as u64,
                            completion_tokens: llm_response.usage.completion_tokens as u64,
                            total_tokens: llm_response.usage.total_tokens as u64,
                            request_count: 1,
                            cached_tokens: llm_response.usage.cached_tokens as u64,
                        }),
                        elapsed_ms: iter_start.elapsed().as_millis() as u64,
                    });

                    iteration += 1;
                }
                LlmOutput::CodeBlocks(codes) => {
                    let code_start = std::time::Instant::now();
                    let interpreter_lock = Arc::clone(&self.code_interpreter);
                    let mut combined_result = String::new();
                    let mut any_error = false;

                    for (idx, code) in codes.iter().enumerate() {
                        let exec_result = tokio::task::spawn_blocking({
                            let code = code.clone();
                            let interpreter_lock = Arc::clone(&interpreter_lock);
                            move || {
                                let mut guard = interpreter_lock.lock().unwrap_or_else(|e| e.into_inner());
                                if guard.is_none() {
                                    *guard = Some(avrag_code_interpreter::CodeInterpreter::new());
                                }
                                guard.as_ref().unwrap().execute(&code)
                            }
                        })
                        .await;

                        let (block_status, block_text, is_err) = match exec_result {
                            Ok(Ok(exec)) => {
                                let is_err = !exec.success
                                    || !exec.stderr.is_empty()
                                    || exec.exit_code.unwrap_or(0) != 0;
                                let status = if is_err {
                                    common::ToolStatus::Error
                                } else {
                                    common::ToolStatus::Ok
                                };
                                let text = format!(
                                    "[block {}] stdout: {}\nstderr: {}",
                                    idx, exec.stdout, exec.stderr
                                );
                                (status, text, is_err)
                            }
                            Ok(Err(e)) => {
                                let text = format!("[block {}] Execution failed: {e}", idx);
                                (common::ToolStatus::Error, text, true)
                            }
                            Err(e) => {
                                let text = format!("[block {}] Interpreter task panicked: {e}", idx);
                                (common::ToolStatus::Error, text, true)
                            }
                        };

                        combined_result.push_str(&block_text);
                        combined_result.push('\n');
                        if is_err {
                            any_error = true;
                        }

                        let _ = sink
                            .emit(AgentEvent::ToolResult {
                                tool: "code_gen".to_string(),
                                status: block_status,
                                data: Some(serde_json::json!({ "result": block_text })),
                                elapsed_ms: code_start.elapsed().as_millis() as u64,
                            })
                            .await;
                    }

                    let elapsed_ms = code_start.elapsed().as_millis() as u64;

                    // Preserve the full LLM response (thought + code tags) for ReAct chain.
                    messages.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: llm_response.content.clone(),
                        name: None,
                        tool_call_id: None,
                        tool_calls: None,
                        reasoning_content: llm_response.reasoning_content.clone(),
                    });
                    // Code interpreter observations are not OpenAI native tool
                    // calls; use a user-role observation to avoid API 400 on
                    // tool messages missing tool_call_id.
                    messages.push(ChatMessage {
                        role: "user".to_string(),
                        content: format!(
                            "<code_execution_result>\n{combined_result}\n</code_execution_result>"
                        ),
                        name: None,
                        tool_call_id: None,
                        tool_calls: None,
                        reasoning_content: None,
                    });

                    if any_error {
                        consecutive_sandbox_errors += 1;
                        if consecutive_sandbox_errors >= 2 {
                            let _ = sink
                                .emit(AgentEvent::Activity {
                                    stage: "sandbox_error".to_string(),
                                    message: "consecutive sandbox errors, breaking to synthesis"
                                        .to_string(),
                                })
                                .await;
                            break;
                        }
                    } else {
                        consecutive_sandbox_errors = 0;
                    }

                    total_tool_calls += codes.len() as u32;

                    telemetry_records.push(ReActIterationRecord {
                        iteration,
                        disclosed_skills: disclosed_skills.iter().map(|s| s.id.clone()).collect(),
                        action_type: "code_gen".to_string(),
                        observation_preview: truncate_preview(&combined_result, 200),
                        llm_usage: Some(AgentRunUsage {
                            provider: llm_response.usage.provider.clone(),
                            model: llm_response.model.clone(),
                            prompt_tokens: llm_response.usage.prompt_tokens as u64,
                            completion_tokens: llm_response.usage.completion_tokens as u64,
                            total_tokens: llm_response.usage.total_tokens as u64,
                            request_count: 1,
                            cached_tokens: llm_response.usage.cached_tokens as u64,
                        }),
                        elapsed_ms,
                    });

                    iteration += 1;
                }
                LlmOutput::Content(content) => {
                    messages.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: content.clone(),
                        name: None,
                        tool_call_id: None,
                        tool_calls: None,
                        reasoning_content: llm_response.reasoning_content.clone(),
                    });

                    telemetry_records.push(ReActIterationRecord {
                        iteration,
                        disclosed_skills: disclosed_skills.iter().map(|s| s.id.clone()).collect(),
                        action_type: "direct_content".to_string(),
                        observation_preview: truncate_preview(&content, 200),
                        llm_usage: Some(AgentRunUsage {
                            provider: llm_response.usage.provider.clone(),
                            model: llm_response.model.clone(),
                            prompt_tokens: llm_response.usage.prompt_tokens as u64,
                            completion_tokens: llm_response.usage.completion_tokens as u64,
                            total_tokens: llm_response.usage.total_tokens as u64,
                            request_count: 1,
                            cached_tokens: llm_response.usage.cached_tokens as u64,
                        }),
                        elapsed_ms: iter_start.elapsed().as_millis() as u64,
                    });

                    break;
                }
            }

            // Truncate only ReAct steps to prevent unbounded growth.
            // Never touch the base conversation (system + history + query).
            const MAX_REACT_MESSAGES: usize = 20;
            if messages.len() > base_message_count + MAX_REACT_MESSAGES {
                let drain_end = messages.len() - MAX_REACT_MESSAGES;
                let drain_start = base_message_count;
                if drain_end > drain_start {
                    messages.drain(drain_start..drain_end);
                }
            }

            let _ = sink
                .emit(AgentEvent::BudgetTick {
                    current: iteration,
                    max: max_iterations,
                })
                .await;
        }

        // Auto-fallback when loop degrades.
        let degraded = iteration >= max_iterations || consecutive_sandbox_errors >= 2;
        if degraded {
            if let Some(fallback) = &mode.auto_fallback {
                if fallback.enabled {
                    match fallback.tool_id.as_str() {
                        "dense_retrieval" => {
                            if let Some(runtime) = &self.rag_runtime {
                                let args = serde_json::to_value(common::DenseRetrievalArgs {
                                    queries: vec![request.query.clone()],
                                    modality: common::DenseRetrievalModality::Text,
                                    top_k: fallback.top_k as usize,
                                    doc_scope: request.doc_scope.clone(),
                                })
                                .map_err(|e| AppError::internal(format!("serialize fallback args: {e}")))?;
                                let result = fallback::inject_fallback_observation(
                                    runtime, &auth, args, &fallback.tool_id, &mut messages,
                                )
                                .await;
                                collected_tool_results.push(result);
                            }
                        }
                        "lexical_retrieval" => {
                            if let Some(runtime) = &self.rag_runtime {
                                let args = serde_json::to_value(common::LexicalRetrievalArgs {
                                    terms: request.query.split_whitespace().map(ToOwned::to_owned).collect(),
                                    top_k: fallback.top_k as usize,
                                    doc_scope: request.doc_scope.clone(),
                                })
                                .map_err(|e| AppError::internal(format!("serialize fallback args: {e}")))?;
                                let result = fallback::inject_fallback_observation(
                                    runtime, &auth, args, &fallback.tool_id, &mut messages,
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
                                    query: Some(request.query.clone()),
                                    doc_scope: request.doc_scope.clone(),
                                })
                                .map_err(|e| AppError::internal(format!("serialize fallback args: {e}")))?;
                                let result = fallback::inject_fallback_observation(
                                    runtime, &auth, args, &fallback.tool_id, &mut messages,
                                )
                                .await;
                                collected_tool_results.push(result);
                            }
                        }
                        "web_search" => {
                            if let Some(executor) = &self.search_executor {
                                let v = fallback.vertical.as_deref().unwrap_or("web");
                                match executor.execute_search(&request.query, Some(v)).await {
                                    Ok(response) => {
                                        let text = serde_json::to_string_pretty(&response)
                                            .unwrap_or_else(|_| "search succeeded".to_string());
                                        messages.push(ChatMessage::system(format!(
                                            "自动兜底搜索结果:\n{text}"
                                        )));
                                    }
                                    Err(e) => {
                                        messages.push(ChatMessage::system(format!(
                                            "[fallback failed: {e}]"
                                        )));
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
                }
            }
        }

        if let Some(format_hint) = request.format_hint.as_deref() {
            if let Some(skill) = self.skill_registry.skill(format_hint) {
                let already_disclosed = disclosed_skills.iter().any(|s| s.id == skill.id);
                if !already_disclosed {
                    disclosed_skills.push(skill.clone());
                }
            }
        }

        let synthesis = SynthesisPhase;
        let final_answer = synthesis
            .run(&self.llm, &base_prompt, mode, &messages, &disclosed_skills, sink, &cancel)
            .await?;

        let total_elapsed_ms = start_time.elapsed().as_millis() as u64;
        let citations =
            crate::agents::unified::helpers::build_all_citations_from_tool_results(
                &collected_tool_results,
            );
        let sources = crate::agents::unified::helpers::build_sources_from_tool_results(
            &collected_tool_results,
        );
        let degrade_trace = crate::agents::unified::helpers::degrade_trace_from_tool_results(
            &collected_tool_results,
        );

        Ok(AgentRunResult {
            answer: final_answer,
            answer_blocks: Vec::new(),
            citations,
            sources,
            reasoning_summary: None,
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
                    }),
                    signals: EvaluationSignals::default(),
                    decision: "synthesize".to_string(),
                    elapsed_ms: r.elapsed_ms,
                    llm_evaluation: None,
                    usage: r.llm_usage.clone(),
                })
                .collect(),
            total_tool_calls,
            tool_results: collected_tool_results,
            final_decision: Some(FinalDecision::Synthesized),
            trace_id: request.session_id.clone(),
            state_history: None,
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
        })
    }
}

/// Safely truncate a string to at most `max_chars` characters (not bytes).
fn truncate_preview(s: &str, max_chars: usize) -> String {
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
fn build_assistant_message_with_tool_calls(
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
fn build_tool_message(
    call_id: &str,
    tool_name: &str,
    result: &common::ToolResult,
) -> ChatMessage {
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
        assert_eq!(
            msg.reasoning_content.as_deref(),
            Some("internal reasoning")
        );
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
