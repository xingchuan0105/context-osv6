use std::sync::Arc;

use avrag_llm::{ChatMessage, LlmResponse};
use common::AppError;
use contracts::ToolResult;

use super::reasoning_emit;
use super::telemetry::ReActIterationRecord;
use super::{truncate_preview, ReActLoop};
use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::runtime::AgentRequest;

use super::iteration::{disclosed_skill_ids, iteration_llm_usage, IterationControl, IterationOutcome, IterationState};

impl ReActLoop {
    pub(super) async fn dispatch_codegen(
        &self,
        iteration: u8,
        request: &AgentRequest,
        auth: &avrag_auth::AuthContext,
        state: &mut IterationState,
        sink: &dyn AgentEventSink,
        llm_response: &LlmResponse,
        _iter_start: std::time::Instant,
        codes: Vec<String>,
    ) -> Result<IterationOutcome, AppError> {
        let llm_usage = iteration_llm_usage(llm_response);
        let code_start = std::time::Instant::now();
        let interpreter_lock = Arc::clone(&self.code_interpreter);
        let mut combined_result = String::new();
        let mut any_error = false;
        let mut bridge_tool_results = Vec::new();

        for (idx, code) in codes.iter().enumerate() {
            let (block_status, block_text, is_err, block_bridge_results) = self
                .execute_codegen_block(idx, code, request, auth, &interpreter_lock)
                .await;
            bridge_tool_results.extend(block_bridge_results);
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
        self.append_codegen_messages(state, llm_response, &combined_result);

        if any_error {
            if let Some(outcome) = self
                .handle_codegen_error(iteration, state, sink)
                .await
            {
                return Ok(outcome);
            }
        } else {
            self.record_codegen_success(state, &combined_result, bridge_tool_results);
        }

        state.total_tool_calls += codes.len() as u32;
        let exit_reason = if any_error {
            "code_gen_error".to_string()
        } else {
            "code_gen".to_string()
        };
        Ok(IterationOutcome {
            control: IterationControl::Continue,
            record: Some(ReActIterationRecord {
                iteration,
                disclosed_skills: disclosed_skill_ids(&state.disclosed),
                action_type: exit_reason.clone(),
                observation_preview: truncate_preview(&combined_result, 200),
                llm_usage: Some(llm_usage),
                elapsed_ms,
                exit_reason,
            }),
            sandbox_break: false,
        })
    }

    fn append_codegen_messages(
        &self,
        state: &mut IterationState,
        llm_response: &LlmResponse,
        combined_result: &str,
    ) {
        state.messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: llm_response.content.clone(),
            name: None,
            tool_call_id: None,
            tool_calls: None,
            multimodal_content: None,
            reasoning_content: llm_response.reasoning_content.clone(),
        });
        state.messages.push(ChatMessage {
            role: "user".to_string(),
            content: format!(
                "<code_execution_result>\n{combined_result}\n</code_execution_result>"
            ),
            name: None,
            tool_call_id: None,
            tool_calls: None,
            multimodal_content: None,
            reasoning_content: None,
        });
    }

    async fn handle_codegen_error(
        &self,
        iteration: u8,
        state: &mut IterationState,
        sink: &dyn AgentEventSink,
    ) -> Option<IterationOutcome> {
        state.consecutive_sandbox_errors += 1;
        if state.consecutive_sandbox_errors < 2 {
            return None;
        }
        let disclosed_skills = disclosed_skill_ids(&state.disclosed);
        reasoning_emit::emit_evaluation_telemetry(
            sink,
            iteration,
            "sandbox_break_to_synthesis",
            "consecutive sandbox errors, breaking to synthesis",
            &disclosed_skills,
            "sandbox_break_to_synthesis",
        )
        .await;
        let _ = sink
            .emit(AgentEvent::Activity {
                stage: "sandbox_error".to_string(),
                message: "consecutive sandbox errors, breaking to synthesis".to_string(),
            })
            .await;
        Some(IterationOutcome {
            control: IterationControl::BreakToSynthesis {
                reason: "sandbox_break_to_synthesis".to_string(),
            },
            record: None,
            sandbox_break: true,
        })
    }

    fn record_codegen_success(
        &self,
        state: &mut IterationState,
        combined_result: &str,
        bridge_tool_results: Vec<ToolResult>,
    ) {
        state.consecutive_sandbox_errors = 0;
        if !bridge_tool_results.is_empty() {
            state.tool_results.extend(bridge_tool_results);
        } else if let Some(result) =
            crate::agents::unified::helpers::tool_result_from_code_execution_observation(
                combined_result,
            )
        {
            state.tool_results.push(result);
        }
    }

    async fn execute_codegen_block(
        &self,
        idx: usize,
        code: &str,
        request: &AgentRequest,
        auth: &avrag_auth::AuthContext,
        interpreter_lock: &Arc<std::sync::Mutex<Option<avrag_code_interpreter::CodeInterpreter>>>,
    ) -> (
        contracts::ToolStatus,
        String,
        bool,
        Vec<ToolResult>,
    ) {
        let code = code.to_string();
        let interpreter_lock = Arc::clone(interpreter_lock);
        let exec_result: Result<
            avrag_code_interpreter::ExecutionResult,
            avrag_code_interpreter::InterpreterError,
        >;
        let mut block_observation_stdout: Option<String> = None;
        let mut block_bridge_results = Vec::new();

        if let Some(runtime) = &self.rag_runtime {
            let bridge = Arc::new(avrag_rag_core::runtime::bridge::RuntimeBridge::new(
                Arc::clone(runtime),
                auth.clone(),
                request.doc_scope.clone(),
            ));
            let interpreter = avrag_code_interpreter::CodeInterpreter::new();
            exec_result = match interpreter
                .execute_with_bridge(&code, Arc::clone(&bridge))
                .await
            {
                Ok(exec) => {
                    block_bridge_results = bridge.take_captured_results();
                    block_observation_stdout = Some(
                        crate::agents::unified::helpers::codegen_observation_stdout(
                            &exec.stdout,
                            &block_bridge_results,
                        ),
                    );
                    Ok(exec)
                }
                Err(e) => Err(e),
            };
        } else {
            let interpreter_lock = Arc::clone(&interpreter_lock);
            let join_result = tokio::task::spawn_blocking(move || {
                let mut guard = interpreter_lock.lock().unwrap_or_else(|e| e.into_inner());
                if guard.is_none() {
                    *guard = Some(avrag_code_interpreter::CodeInterpreter::new());
                }
                guard.as_ref().unwrap().execute(&code)
            })
            .await;
            exec_result = match join_result {
                Ok(result) => result,
                Err(e) => Err(avrag_code_interpreter::InterpreterError::Bridge(format!(
                    "interpreter task panicked: {e}"
                ))),
            };
        }

        match exec_result {
            Ok(exec) => {
                let is_err = !exec.success
                    || !exec.stderr.is_empty()
                    || exec.exit_code.unwrap_or(0) != 0;
                let status = if is_err {
                    contracts::ToolStatus::Error
                } else {
                    contracts::ToolStatus::Ok
                };
                let stdout_for_observation = block_observation_stdout
                    .as_deref()
                    .unwrap_or(exec.stdout.as_str());
                let text = format!(
                    "[block {}] stdout: {}\nstderr: {}",
                    idx, stdout_for_observation, exec.stderr
                );
                (status, text, is_err, block_bridge_results)
            }
            Err(e) => {
                let text = format!("[block {}] Execution failed: {e}", idx);
                (contracts::ToolStatus::Error, text, true, block_bridge_results)
            }
        }
    }
}
