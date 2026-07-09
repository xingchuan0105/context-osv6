use avrag_llm::{ChatMessage, LlmResponse};
use common::AppError;
use contracts::ToolResult;

use super::config::ModeConfig;
use super::telemetry::ReActIterationRecord;
use super::{ReActLoop, build_assistant_message_with_tool_calls, build_tool_message};
use crate::events::{AgentEvent, AgentEventSink};
use crate::runtime::AgentRequest;
use agent_tools::tool_registry::OwnedToolDeps;

use super::iteration::{
    IterationControl, IterationOutcome, IterationState, disclosed_skill_ids, iteration_llm_usage,
};

impl ReActLoop {
    /// Single dispatch entry: every tool goes through [`agent_tools::tool_registry`].
    pub(super) async fn dispatch_tool_call(
        &self,
        call: &contracts::ToolCall,
        auth: &contracts::auth_runtime::AuthContext,
        doc_scope: &[String],
        session_id: Option<&str>,
    ) -> ToolResult {
        let deps = OwnedToolDeps {
            search_executor: self.search_executor.clone(),
            rag_runtime: self.rag_runtime.clone(),
            chat_persistence: self.effective_chat_persistence(),
        };
        deps.dispatch(call, auth, doc_scope, session_id).await
    }

    pub(super) async fn dispatch_native_tool_calls(
        &self,
        iteration: u8,
        _mode: &ModeConfig,
        request: &AgentRequest,
        auth: &contracts::auth_runtime::AuthContext,
        _loop_exit: &super::config::LoopExitConfig,
        state: &mut IterationState,
        sink: &dyn AgentEventSink,
        llm_response: &LlmResponse,
        iter_start: std::time::Instant,
        calls: Vec<contracts::ToolCall>,
    ) -> Result<IterationOutcome, AppError> {
        let llm_usage = iteration_llm_usage(llm_response);
        let call_ids: Vec<String> = (0..calls.len()).map(|i| format!("call_{}", i)).collect();

        let mut tool_messages = Vec::new();
        for (idx, call) in calls.iter().enumerate() {
            let call_id = &call_ids[idx];
            let tool_start = std::time::Instant::now();
            let result = self
                .dispatch_tool_call(
                    call,
                    auth,
                    &request.doc_scope,
                    request.session_id.as_deref(),
                )
                .await;
            let tool_elapsed_ms = tool_start.elapsed().as_millis() as u64;

            let _ = sink
                .emit(AgentEvent::ToolResult {
                    tool: call.tool.clone(),
                    status: result.status.clone(),
                    data: result.data.clone(),
                    elapsed_ms: tool_elapsed_ms,
                })
                .await;

            tool_messages.push(build_tool_message(call_id, &call.tool, &result));
            state.tool_results.push(result);
        }

        self.update_state_after_tool_calls(
            state,
            &calls,
            &call_ids,
            llm_response,
            tool_messages,
        );

        let exit_reason = "native_tool_call".to_string();
        Ok(IterationOutcome {
            control: IterationControl::Continue,
            record: Some(ReActIterationRecord {
                iteration,
                disclosed_skills: disclosed_skill_ids(&state.disclosed),
                action_type: exit_reason.clone(),
                observation_preview: format!("{} tool calls", calls.len()),
                llm_usage: Some(llm_usage),
                elapsed_ms: iter_start.elapsed().as_millis() as u64,
                exit_reason,
            }),
            sandbox_break: false,
        })
    }

    fn update_state_after_tool_calls(
        &self,
        state: &mut IterationState,
        calls: &[contracts::ToolCall],
        call_ids: &[String],
        llm_response: &LlmResponse,
        tool_messages: Vec<ChatMessage>,
    ) {
        state.messages.push(build_assistant_message_with_tool_calls(
            calls,
            call_ids,
            &llm_response.content,
            llm_response.reasoning_content.clone(),
        ));
        for tm in tool_messages {
            state.messages.push(tm);
        }

        state.total_tool_calls += calls.len() as u32;
        state.consecutive_sandbox_errors = 0;
    }
}

