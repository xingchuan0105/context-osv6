use std::sync::Arc;

use avrag_llm::{ChatMessage, LlmResponse, LlmUsage};
use common::{AppError, ToolResult};

use super::assembler::{ContextAssembler, DisclosedState};
use super::config::{LoopExitConfig, ModeConfig};
use super::exit_policy::{has_retrieval_observation, should_block_content_early_stop};
use super::optimizer::{
    build_budget_warning, build_duplicate_hint, extract_chunk_ids, ContextAdjustment,
    IterationProgress, LoopOptimizer,
};
use super::parse::{LlmOutput, parse_llm_output};
use super::reasoning_emit::{self, record_reasoning};
use super::skill_request::{is_skill_request_message, validate_skill_request};
use super::telemetry::ReActIterationRecord;
use super::{
    build_assistant_message_with_tool_calls, build_tool_message, dispatch_rag_tool,
    truncate_preview, ReActLoop,
};
use crate::agents::events::{AgentEvent, AgentEventSink};
use crate::agents::runtime::{AgentRequest, AgentRunUsage};

pub struct IterationState {
    pub messages: Vec<ChatMessage>,
    pub disclosed: DisclosedState,
    pub tool_results: Vec<ToolResult>,
    pub progress: IterationProgress,
    pub total_tool_calls: u32,
    pub consecutive_sandbox_errors: u8,
    pub reasoning_acc: String,
}

pub enum IterationControl {
    Continue,
    BreakToSynthesis { reason: String },
    DirectAnswer { content: String },
}

pub struct IterationOutcome {
    pub control: IterationControl,
    pub record: Option<ReActIterationRecord>,
    /// Sandbox break emits telemetry inline and skips TurnEnd/record (legacy behavior).
    pub sandbox_break: bool,
}

impl ReActLoop {
    async fn dispatch_tool_call(
        &self,
        call: &common::ToolCall,
        auth: &avrag_auth::AuthContext,
        doc_scope: &[String],
        session_id: Option<&str>,
    ) -> ToolResult {
        match call.tool.as_str() {
            "conversation_history_load" | "user_profile_load" | "conversation_history_tag"
            | "calculator" | "code_interpreter" | "weather_query" | "web_fetch" | "web_search" => {
                self.dispatch_skill_tool(call, auth, session_id).await
            }
            "dense_retrieval" | "lexical_retrieval" | "graph_retrieval" | "index_lookup"
            | "doc_summary" | "doc_metadata" | "doc_profile" => {
                if let Some(runtime) = &self.rag_runtime {
                    dispatch_rag_tool(runtime, auth, call, doc_scope).await
                } else {
                    common::ToolResult {
                        tool: call.tool.clone(),
                        version: call.version.clone(),
                        status: common::ToolStatus::NotImplemented,
                        data: Some(serde_json::json!({"error": "rag runtime not configured"})),
                        trace: None,
                    }
                }
            }
            _ => self.dispatch_skill_tool(call, auth, session_id).await,
        }
    }

    async fn dispatch_skill_tool(
        &self,
        call: &common::ToolCall,
        auth: &avrag_auth::AuthContext,
        session_id: Option<&str>,
    ) -> common::ToolResult {
        let session_uuid = session_id.and_then(|id| uuid::Uuid::parse_str(id).ok());
        let pg = self.effective_pg_repo();
        crate::agents::unified::atomic_tools::dispatch_atomic_tool_with_enforcement(
            call,
            self.search_executor.as_deref(),
            Some(auth),
            session_uuid,
            pg.as_deref(),
        )
        .await
    }

    pub(super) async fn run_iteration(
        &self,
        iteration: u8,
        max_iterations: u8,
        mode: &ModeConfig,
        request: &AgentRequest,
        auth: &avrag_auth::AuthContext,
        loop_exit: &LoopExitConfig,
        state: &mut IterationState,
        total_usage: &mut LlmUsage,
        optimizer: &LoopOptimizer,
        sink: &dyn AgentEventSink,
    ) -> Result<IterationOutcome, AppError> {
        let last_assistant_content = state
            .messages
            .iter()
            .rev()
            .find(|m| m.role == "assistant")
            .map(|m| m.content.as_str());

        let assembled = ContextAssembler::assemble_retrieve(
            iteration,
            mode,
            request,
            &self.skill_registry,
            &mut state.disclosed,
            last_assistant_content,
        );
        reasoning_emit::emit_prompt_snapshot(
            sink,
            "retrieve",
            iteration,
            &assembled,
            &state.disclosed,
        )
        .await;
        reasoning_emit::emit_plan_decision_telemetry(
            sink,
            "retrieve",
            iteration,
            &assembled,
            &state.disclosed,
        )
        .await;

        let mut round_messages = vec![ChatMessage::system(assembled.system_content)];
        for msg in &state.messages {
            if msg.role != "system" {
                round_messages.push(msg.clone());
            }
        }

        let iter_start = std::time::Instant::now();
        let temperature = mode.temperature.unwrap_or(0.7);
        let llm_response = self
            .llm
            .complete_with_tools(&round_messages, &assembled.tools, Some(temperature))
            .await
            .map_err(|e| AppError::internal(format!("llm completion failed: {e}")))?;

        total_usage.accumulate(&llm_response.usage);
        record_reasoning(
            sink,
            &mut state.reasoning_acc,
            llm_response.reasoning_content.as_deref(),
        )
        .await;

        self.apply_llm_output(
            iteration,
            max_iterations,
            mode,
            request,
            auth,
            loop_exit,
            state,
            optimizer,
            sink,
            &llm_response,
            iter_start,
        )
        .await
    }

    pub(crate) async fn apply_llm_output(
        &self,
        iteration: u8,
        max_iterations: u8,
        mode: &ModeConfig,
        request: &AgentRequest,
        auth: &avrag_auth::AuthContext,
        loop_exit: &LoopExitConfig,
        state: &mut IterationState,
        optimizer: &LoopOptimizer,
        sink: &dyn AgentEventSink,
        llm_response: &LlmResponse,
        iter_start: std::time::Instant,
    ) -> Result<IterationOutcome, AppError> {
        let validated = validate_skill_request(mode, &llm_response.content);
        if !validated.is_empty() {
            state.disclosed.last_skill_request = Some(validated);
        }

        let parsed = parse_llm_output(llm_response);
        let llm_usage = || AgentRunUsage {
            provider: llm_response.usage.provider.clone(),
            model: llm_response.model.clone(),
            prompt_tokens: llm_response.usage.prompt_tokens as u64,
            completion_tokens: llm_response.usage.completion_tokens as u64,
            total_tokens: llm_response.usage.total_tokens as u64,
            request_count: 1,
            cached_tokens: llm_response.usage.cached_tokens as u64,
        };

        match parsed {
            LlmOutput::NativeToolCalls(calls) => {
                let call_ids: Vec<String> =
                    (0..calls.len()).map(|i| format!("call_{}", i)).collect();

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

                state.messages.push(build_assistant_message_with_tool_calls(
                    &calls,
                    &call_ids,
                    &llm_response.content,
                    llm_response.reasoning_content.clone(),
                ));
                for tm in tool_messages {
                    state.messages.push(tm);
                }

                let current_chunk_ids = extract_chunk_ids(&state.tool_results);
                state.progress.record_iteration(iteration, &current_chunk_ids);
                let remaining = max_iterations.saturating_sub(iteration + 1);
                match optimizer.advise(
                    &state.progress,
                    &current_chunk_ids,
                    remaining,
                    max_iterations,
                ) {
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
                    ContextAdjustment::BudgetWarning { remaining, max } => {
                        state
                            .messages
                            .push(ChatMessage::system(build_budget_warning(remaining, max)));
                    }
                    ContextAdjustment::None => {}
                }

                state.total_tool_calls += calls.len() as u32;
                state.consecutive_sandbox_errors = 0;

                let exit_reason = "native_tool_call".to_string();
                Ok(IterationOutcome {
                    control: IterationControl::Continue,
                    record: Some(ReActIterationRecord {
                        iteration,
                        disclosed_skills: disclosed_skill_ids(&state.disclosed),
                        action_type: exit_reason.clone(),
                        observation_preview: format!("{} tool calls", calls.len()),
                        llm_usage: Some(llm_usage()),
                        elapsed_ms: iter_start.elapsed().as_millis() as u64,
                        exit_reason,
                    }),
                    sandbox_break: false,
                })
            }
            LlmOutput::CodeBlocks(codes) => {
                let code_start = std::time::Instant::now();
                let interpreter_lock = Arc::clone(&self.code_interpreter);
                let mut combined_result = String::new();
                let mut any_error = false;
                let mut bridge_tool_results = Vec::new();

                for (idx, code) in codes.iter().enumerate() {
                    let code = code.clone();
                    let interpreter_lock = Arc::clone(&interpreter_lock);
                    let exec_result: Result<
                        avrag_code_interpreter::ExecutionResult,
                        avrag_code_interpreter::InterpreterError,
                    >;
                    let mut block_observation_stdout: Option<String> = None;

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
                                let block_bridge_results = bridge.take_captured_results();
                                bridge_tool_results.extend(block_bridge_results.clone());
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
                            let mut guard =
                                interpreter_lock.lock().unwrap_or_else(|e| e.into_inner());
                            if guard.is_none() {
                                *guard = Some(avrag_code_interpreter::CodeInterpreter::new());
                            }
                            guard.as_ref().unwrap().execute(&code)
                        })
                        .await;
                        exec_result = match join_result {
                            Ok(result) => result,
                            Err(e) => Err(avrag_code_interpreter::InterpreterError::Bridge(
                                format!("interpreter task panicked: {e}"),
                            )),
                        };
                    }

                    let (block_status, block_text, is_err) = match exec_result {
                        Ok(exec) => {
                            let is_err = !exec.success
                                || !exec.stderr.is_empty()
                                || exec.exit_code.unwrap_or(0) != 0;
                            let status = if is_err {
                                common::ToolStatus::Error
                            } else {
                                common::ToolStatus::Ok
                            };
                            let stdout_for_observation = block_observation_stdout
                                .as_deref()
                                .unwrap_or(exec.stdout.as_str());
                            let text = format!(
                                "[block {}] stdout: {}\nstderr: {}",
                                idx, stdout_for_observation, exec.stderr
                            );
                            (status, text, is_err)
                        }
                        Err(e) => {
                            let text = format!("[block {}] Execution failed: {e}", idx);
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

                if any_error {
                    state.consecutive_sandbox_errors += 1;
                    if state.consecutive_sandbox_errors >= 2 {
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
                                message: "consecutive sandbox errors, breaking to synthesis"
                                    .to_string(),
                            })
                            .await;
                        return Ok(IterationOutcome {
                            control: IterationControl::BreakToSynthesis {
                                reason: "sandbox_break_to_synthesis".to_string(),
                            },
                            record: None,
                            sandbox_break: true,
                        });
                    }
                } else {
                    state.consecutive_sandbox_errors = 0;
                    if !bridge_tool_results.is_empty() {
                        state.tool_results.extend(bridge_tool_results);
                    } else if let Some(result) =
                        crate::agents::unified::helpers::tool_result_from_code_execution_observation(
                            &combined_result,
                        )
                    {
                        state.tool_results.push(result);
                    }
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
                        llm_usage: Some(llm_usage()),
                        elapsed_ms,
                        exit_reason,
                    }),
                    sandbox_break: false,
                })
            }
            LlmOutput::Content(content) => {
                state.messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: content.clone(),
                    name: None,
                    tool_call_id: None,
                    tool_calls: None,
                    multimodal_content: None,
                    reasoning_content: llm_response.reasoning_content.clone(),
                });

                if is_skill_request_message(&content) {
                    let exit_reason = "skill_request".to_string();
                    return Ok(IterationOutcome {
                        control: IterationControl::Continue,
                        record: Some(ReActIterationRecord {
                            iteration,
                            disclosed_skills: disclosed_skill_ids(&state.disclosed),
                            action_type: exit_reason.clone(),
                            observation_preview: truncate_preview(&content, 200),
                            llm_usage: Some(llm_usage()),
                            elapsed_ms: iter_start.elapsed().as_millis() as u64,
                            exit_reason,
                        }),
                        sandbox_break: false,
                    });
                }

                let has_evidence_now =
                    has_retrieval_observation(&state.messages, &state.tool_results, mode);
                if should_block_content_early_stop(loop_exit, has_evidence_now) {
                    state.messages.push(ChatMessage::user(
                        "You must retrieve evidence (code execution or tools) before answering. \
                         Continue with retrieval — do not answer from memory alone.",
                    ));
                    let exit_reason = "content_blocked_no_evidence".to_string();
                    return Ok(IterationOutcome {
                        control: IterationControl::Continue,
                        record: Some(ReActIterationRecord {
                            iteration,
                            disclosed_skills: disclosed_skill_ids(&state.disclosed),
                            action_type: exit_reason.clone(),
                            observation_preview: truncate_preview(&content, 200),
                            llm_usage: Some(llm_usage()),
                            elapsed_ms: iter_start.elapsed().as_millis() as u64,
                            exit_reason,
                        }),
                        sandbox_break: false,
                    });
                }

                let exit_reason = "direct_content".to_string();
                Ok(IterationOutcome {
                    control: IterationControl::DirectAnswer {
                        content: content.clone(),
                    },
                    record: Some(ReActIterationRecord {
                        iteration,
                        disclosed_skills: disclosed_skill_ids(&state.disclosed),
                        action_type: exit_reason.clone(),
                        observation_preview: truncate_preview(&content, 200),
                        llm_usage: Some(llm_usage()),
                        elapsed_ms: iter_start.elapsed().as_millis() as u64,
                        exit_reason,
                    }),
                    sandbox_break: false,
                })
            }
        }
    }
}

fn disclosed_skill_ids(disclosed: &DisclosedState) -> Vec<String> {
    disclosed.disclosed_skill_ids.iter().cloned().collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::agents::capability::CapabilityRegistry;
    use crate::agents::events::CollectingSink;
    use crate::agents::AgentKind;
    use avrag_llm::{LlmClient, ModelProviderConfig};

    fn rag_mode() -> ModeConfig {
        super::super::config::load_mode_config("rag").unwrap()
    }

    fn chat_mode() -> ModeConfig {
        super::super::config::load_mode_config("chat").unwrap()
    }

    fn base_request(kind: AgentKind) -> AgentRequest {
        AgentRequest {
            kind,
            query: "test".to_string(),
            resolved_query: "test".to_string(),
            query_resolution: None,
            notebook_id: None,
            session_id: None,
            doc_scope: vec![],
            messages: vec![],
            user_preferences: None,
            debug: false,
            stream: false,
            language: None,
            auth_context: serde_json::json!({}),
            docscope_metadata: None,
            metadata: Default::default(),
            cancellation_token: None,
            guard_pipeline: None,
            preferred_tools: vec![],
            format_hint: None,
            max_iterations: None,
        }
    }

    fn test_loop() -> ReActLoop {
        ReActLoop::new(
            Arc::new(LlmClient::new(ModelProviderConfig {
                base_url: "http://localhost".to_string(),
                api_key: String::new(),
                model: "test".to_string(),
                timeout_ms: 1000,
                api_style: None,
                dimensions: None,
                enable_thinking: None,
                enable_cache: None,
                rpm_limit: None,
                tpm_limit: None,
            })),
            Arc::new(CapabilityRegistry::standard()),
        )
    }

    fn fake_llm_response(content: &str) -> LlmResponse {
        LlmResponse {
            content: content.to_string(),
            reasoning_content: None,
            usage: LlmUsage::zeroed(),
            model: "test-model".to_string(),
            tool_calls: None,
        }
    }

    fn empty_state() -> IterationState {
        IterationState {
            messages: vec![ChatMessage::user("test")],
            disclosed: DisclosedState::default(),
            tool_results: vec![],
            progress: IterationProgress::new(),
            total_tool_calls: 0,
            consecutive_sandbox_errors: 0,
            reasoning_acc: String::new(),
        }
    }

    fn test_auth() -> avrag_auth::AuthContext {
        serde_json::from_value(serde_json::json!({
            "org_id": "00000000-0000-0000-0000-000000000001",
            "subject_kind": "User",
            "permissions": []
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn native_tool_call_returns_continue_with_record() {
        let loop_ = test_loop();
        let mode = super::super::config::load_mode_config("search").unwrap();
        let mut state = empty_state();
        let sink = CollectingSink::new();
        let optimizer = LoopOptimizer::new();
        let auth = test_auth();
        let mut response = fake_llm_response("");
        response.tool_calls = Some(vec![common::ToolCall {
            tool: "web_search".to_string(),
            version: "1".to_string(),
            args: serde_json::json!({"query": "news"}),
        }]);

        let outcome = loop_
            .apply_llm_output(
                0,
                3,
                &mode,
                &base_request(AgentKind::Search),
                &auth,
                &mode.loop_exit_for_mode(),
                &mut state,
                &optimizer,
                &sink,
                &response,
                std::time::Instant::now(),
            )
            .await
            .unwrap();

        assert!(matches!(outcome.control, IterationControl::Continue));
        assert_eq!(
            outcome.record.as_ref().unwrap().exit_reason,
            "native_tool_call"
        );
        assert_eq!(state.messages.len(), 3);
        assert_eq!(state.total_tool_calls, 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn codegen_without_print_leaves_model_observation_empty_but_bridge_has_chunks() {
        use avrag_llm::ModelProviderConfig;
        use avrag_rag_core::RagRuntime;
        use uuid::Uuid;

        struct StubDataPlane {
            chunk_id: Uuid,
            doc_id: Uuid,
        }

        #[async_trait::async_trait]
        impl avrag_retrieval_data_plane::RetrievalDataPlane for StubDataPlane {
            async fn search_text_dense(
                &self,
                _request: avrag_retrieval_data_plane::TextDenseSearchRequest,
            ) -> anyhow::Result<Vec<avrag_retrieval_data_plane::ScoredChunk>> {
                Ok(vec![avrag_retrieval_data_plane::ScoredChunk {
                    chunk_id: self.chunk_id,
                    doc_id: self.doc_id,
                    content: "bridge hit".to_string(),
                    score: 0.95,
                    source: "stub".to_string(),
                    page: Some(1),
                    chunk_type: "text".to_string(),
                    asset_id: None,
                    caption: None,
                    image_path: None,
                    parser_backend: None,
                    source_locator: None,
                    parse_run_id: None,
                }])
            }

            async fn search_bm25(
                &self,
                _request: avrag_retrieval_data_plane::Bm25SearchRequest,
            ) -> anyhow::Result<avrag_retrieval_data_plane::Bm25SearchOutput> {
                Ok(avrag_retrieval_data_plane::Bm25SearchOutput {
                    chunks: vec![],
                    trace: avrag_retrieval_data_plane::Bm25SearchTrace {
                        backend: "stub".to_string(),
                        raw_hit_count: 0,
                        hydrated_hit_count: 0,
                        fallback_reason: None,
                    },
                })
            }

            async fn search_multimodal(
                &self,
                _request: avrag_retrieval_data_plane::MultimodalSearchRequest,
            ) -> anyhow::Result<Vec<avrag_retrieval_data_plane::ScoredChunk>> {
                Ok(vec![])
            }

            async fn search_graph(
                &self,
                _request: avrag_retrieval_data_plane::GraphSearchRequest,
            ) -> anyhow::Result<avrag_retrieval_data_plane::GraphSearchOutput> {
                Ok(avrag_retrieval_data_plane::GraphSearchOutput {
                    relation_paths: vec![],
                    supporting_chunks: vec![],
                })
            }
        }

        let embedding = Arc::new(avrag_llm::EmbeddingClient::new(ModelProviderConfig {
            base_url: "http://localhost:9999".to_string(),
            api_key: "test".to_string(),
            model: "test-model".to_string(),
            timeout_ms: 5000,
            api_style: None,
            dimensions: None,
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        }));
        let chunk_id = Uuid::from_u128(1);
        let doc_id = Uuid::parse_str("00000000-0000-0000-0000-000000000010").unwrap();
        let data_plane: Arc<dyn avrag_retrieval_data_plane::RetrievalDataPlane> =
            Arc::new(StubDataPlane { chunk_id, doc_id });
        let config = avrag_rag_core::RagConfig::new_for_data_plane(embedding, None);
        let runtime = Arc::new(RagRuntime::with_data_plane(config, data_plane));

        let loop_ = test_loop().with_rag_runtime(Some(runtime));
        let mode = rag_mode();
        let mut state = empty_state();
        let sink = CollectingSink::new();
        let optimizer = LoopOptimizer::new();
        let auth = test_auth();
        let mut request = base_request(AgentKind::Rag);
        request.doc_scope = vec![doc_id.to_string()];

        // Real LLM often assigns without printing — stdout stays empty.
        let response = fake_llm_response(
            r#"<code language="python">chunks = await client.dense_search(query="antifragility", top_k=10)</code>"#,
        );

        let _outcome = loop_
            .apply_llm_output(
                0,
                4,
                &mode,
                &request,
                &auth,
                &mode.loop_exit_for_mode(),
                &mut state,
                &optimizer,
                &sink,
                &response,
                std::time::Instant::now(),
            )
            .await
            .unwrap();

        let observation = state
            .messages
            .iter()
            .find(|m| m.content.contains("<code_execution_result>"))
            .map(|m| m.content.as_str())
            .expect("code_execution_result message");
        assert!(
            !super::super::exit_policy::code_execution_has_evidence(observation)
                || observation.contains("chunk_id"),
            "when bridge returns chunks, observation stdout should carry chunk json: {observation}"
        );
        assert!(
            state
                .tool_results
                .iter()
                .any(|r| r.tool == "dense_retrieval" && r.status == common::ToolStatus::Ok),
            "bridge side-channel should record dense_retrieval Ok even when stdout empty; tool_results: {:?}",
            state.tool_results
        );
    }

    #[tokio::test]
    async fn code_block_success_returns_continue() {
        let loop_ = test_loop();
        let mode = rag_mode();
        let mut state = empty_state();
        let sink = CollectingSink::new();
        let optimizer = LoopOptimizer::new();
        let auth = test_auth();
        let response = fake_llm_response(r#"<code language="python">print("ok")</code>"#);

        let outcome = loop_
            .apply_llm_output(
                0,
                4,
                &mode,
                &base_request(AgentKind::Rag),
                &auth,
                &mode.loop_exit_for_mode(),
                &mut state,
                &optimizer,
                &sink,
                &response,
                std::time::Instant::now(),
            )
            .await
            .unwrap();

        assert!(matches!(outcome.control, IterationControl::Continue));
        assert_eq!(outcome.record.as_ref().unwrap().exit_reason, "code_gen");
        assert!(state.messages.iter().any(|m| m.content.contains("code_execution_result")));
    }

    #[tokio::test]
    async fn consecutive_code_errors_break_to_synthesis() {
        let loop_ = test_loop();
        let mode = rag_mode();
        let mut state = empty_state();
        state.consecutive_sandbox_errors = 1;
        let sink = CollectingSink::new();
        let optimizer = LoopOptimizer::new();
        let auth = test_auth();
        let response =
            fake_llm_response(r#"<code language="python">raise RuntimeError("fail")</code>"#);

        let outcome = loop_
            .apply_llm_output(
                1,
                4,
                &mode,
                &base_request(AgentKind::Rag),
                &auth,
                &mode.loop_exit_for_mode(),
                &mut state,
                &optimizer,
                &sink,
                &response,
                std::time::Instant::now(),
            )
            .await
            .unwrap();

        assert!(matches!(
            outcome.control,
            IterationControl::BreakToSynthesis { .. }
        ));
        assert!(outcome.sandbox_break);
        assert!(outcome.record.is_none());
    }

    #[tokio::test]
    async fn content_with_evidence_in_chat_returns_direct_answer() {
        let loop_ = test_loop();
        let mode = chat_mode();
        let mut state = empty_state();
        let sink = CollectingSink::new();
        let optimizer = LoopOptimizer::new();
        let auth = test_auth();
        let response = fake_llm_response("Here is your answer.");

        let outcome = loop_
            .apply_llm_output(
                0,
                2,
                &mode,
                &base_request(AgentKind::Chat),
                &auth,
                &mode.loop_exit_for_mode(),
                &mut state,
                &optimizer,
                &sink,
                &response,
                std::time::Instant::now(),
            )
            .await
            .unwrap();

        assert!(matches!(
            outcome.control,
            IterationControl::DirectAnswer { content } if content == "Here is your answer."
        ));
        assert_eq!(outcome.record.as_ref().unwrap().exit_reason, "direct_content");
    }

    #[tokio::test]
    async fn content_without_evidence_in_rag_is_blocked() {
        let loop_ = test_loop();
        let mode = rag_mode();
        let mut state = empty_state();
        let sink = CollectingSink::new();
        let optimizer = LoopOptimizer::new();
        let auth = test_auth();
        let response = fake_llm_response("Answer without retrieval.");

        let outcome = loop_
            .apply_llm_output(
                0,
                4,
                &mode,
                &base_request(AgentKind::Rag),
                &auth,
                &mode.loop_exit_for_mode(),
                &mut state,
                &optimizer,
                &sink,
                &response,
                std::time::Instant::now(),
            )
            .await
            .unwrap();

        assert!(matches!(outcome.control, IterationControl::Continue));
        assert_eq!(
            outcome.record.as_ref().unwrap().exit_reason,
            "content_blocked_no_evidence"
        );
        assert!(state.messages.iter().any(|m| {
            m.role == "user" && m.content.contains("retrieve evidence")
        }));
    }

    #[tokio::test]
    async fn skill_request_json_in_chat_is_not_direct_answer() {
        let loop_ = test_loop();
        let mode = chat_mode();
        let mut state = empty_state();
        let sink = CollectingSink::new();
        let optimizer = LoopOptimizer::new();
        let auth = test_auth();
        let response = fake_llm_response(r#"{"skill_request":["memory"]}"#);

        let outcome = loop_
            .apply_llm_output(
                0,
                2,
                &mode,
                &base_request(AgentKind::Chat),
                &auth,
                &mode.loop_exit_for_mode(),
                &mut state,
                &optimizer,
                &sink,
                &response,
                std::time::Instant::now(),
            )
            .await
            .unwrap();

        assert!(matches!(outcome.control, IterationControl::Continue));
        assert_eq!(outcome.record.as_ref().unwrap().exit_reason, "skill_request");
        assert_eq!(
            state.disclosed.last_skill_request,
            Some(vec!["memory".to_string()])
        );
    }

    #[test]
    fn iteration_state_defaults_are_empty() {
        let state = empty_state();
        assert_eq!(state.messages.len(), 1);
        assert!(state.disclosed.disclosed_skill_ids.is_empty());
    }
}
