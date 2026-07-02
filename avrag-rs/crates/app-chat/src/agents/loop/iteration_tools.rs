use avrag_llm::{ChatMessage, LlmResponse};
use common::AppError;
use contracts::ToolResult;

use super::config::ModeConfig;
use super::optimizer::{ContextAdjustment, LoopOptimizer, build_duplicate_hint, extract_chunk_ids};
use super::telemetry::ReActIterationRecord;
use super::{
    ReActLoop, build_assistant_message_with_tool_calls, build_tool_message, dispatch_rag_tool,
};
use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::runtime::AgentRequest;

use super::iteration::{
    IterationControl, IterationOutcome, IterationState, disclosed_skill_ids, iteration_llm_usage,
};

impl ReActLoop {
    pub(super) async fn dispatch_tool_call(
        &self,
        call: &contracts::ToolCall,
        auth: &avrag_auth::AuthContext,
        doc_scope: &[String],
        session_id: Option<&str>,
    ) -> ToolResult {
        match call.tool.as_str() {
            "dense_retrieval" | "lexical_retrieval" | "graph_retrieval" | "index_lookup"
            | "doc_summary" | "doc_metadata" | "doc_profile" => {
                self.dispatch_rag_tool_call(call, auth, doc_scope).await
            }
            "web_fetch" | "web_search" => self.dispatch_search(call, auth, session_id).await,
            "conversation_history_load"
            | "user_profile_load"
            | "calculator"
            | "code_interpreter"
            | "weather_query" => self.dispatch_native(call, auth, session_id).await,
            _ => self.dispatch_native(call, auth, session_id).await,
        }
    }

    async fn dispatch_rag_tool_call(
        &self,
        call: &contracts::ToolCall,
        auth: &avrag_auth::AuthContext,
        doc_scope: &[String],
    ) -> ToolResult {
        if let Some(runtime) = &self.rag_runtime {
            dispatch_rag_tool(runtime, auth, call, doc_scope).await
        } else {
            contracts::ToolResult {
                tool: call.tool.clone(),
                version: call.version.clone(),
                status: contracts::ToolStatus::NotImplemented,
                data: Some(serde_json::json!({"error": "rag runtime not configured"})),
                trace: None,
            }
        }
    }

    async fn dispatch_search(
        &self,
        call: &contracts::ToolCall,
        auth: &avrag_auth::AuthContext,
        session_id: Option<&str>,
    ) -> ToolResult {
        self.dispatch_skill_tool(call, auth, session_id).await
    }

    async fn dispatch_native(
        &self,
        call: &contracts::ToolCall,
        auth: &avrag_auth::AuthContext,
        session_id: Option<&str>,
    ) -> ToolResult {
        self.dispatch_skill_tool(call, auth, session_id).await
    }

    async fn dispatch_skill_tool(
        &self,
        call: &contracts::ToolCall,
        auth: &avrag_auth::AuthContext,
        session_id: Option<&str>,
    ) -> contracts::ToolResult {
        let session_uuid = session_id.and_then(|id| uuid::Uuid::parse_str(id).ok());
        let chat_persistence = self.effective_chat_persistence();
        crate::agents::unified::atomic_tools::dispatch_atomic_tool_with_enforcement(
            call,
            self.search_executor.as_deref(),
            Some(auth),
            session_uuid,
            chat_persistence.as_ref().map(|p| &**p),
        )
        .await
    }

    pub(super) async fn dispatch_native_tool_calls(
        &self,
        iteration: u8,
        _mode: &ModeConfig,
        request: &AgentRequest,
        auth: &avrag_auth::AuthContext,
        _loop_exit: &super::config::LoopExitConfig,
        state: &mut IterationState,
        optimizer: &LoopOptimizer,
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
            optimizer,
            iteration,
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
        optimizer: &LoopOptimizer,
        iteration: u8,
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

        let current_chunk_ids = extract_chunk_ids(&state.tool_results);
        state
            .progress
            .record_iteration(iteration, &current_chunk_ids);
        match optimizer.advise(&state.progress, &current_chunk_ids) {
            ContextAdjustment::DuplicateChunksHint {
                chunk_ids,
                first_seen_at,
            } => {
                state
                    .messages
                    .push(ChatMessage::system(build_duplicate_hint(
                        &chunk_ids,
                        &first_seen_at,
                    )));
            }
            ContextAdjustment::None => {}
        }

        state.total_tool_calls += calls.len() as u32;
        state.consecutive_sandbox_errors = 0;
    }
}
