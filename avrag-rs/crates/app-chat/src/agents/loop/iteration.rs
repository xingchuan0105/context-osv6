use avrag_llm::{ChatMessage, LlmResponse, LlmUsage};
use common::AppError;
use contracts::ToolResult;

use super::assembler::{ContextAssembler, DisclosedState};
use super::config::{LoopExitConfig, ModeConfig};
use super::exit_policy::{has_retrieval_observation, should_block_content_early_stop};
use super::optimizer::{IterationProgress, LoopOptimizer};
use super::parse::{LlmOutput, parse_llm_output};
use super::reasoning_emit::{self, record_reasoning};
use super::skill_request::{is_skill_request_message, validate_skill_request};
use super::telemetry::ReActIterationRecord;
use super::{truncate_preview, ReActLoop};
use crate::agents::events::{AgentEventSink};
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
        let assembled = self.assemble_retrieve_context(iteration, mode, request, state, sink).await;
        let iter_start = std::time::Instant::now();
        let llm_response = self
            .call_retrieve_llm(mode, state, total_usage, &assembled, sink)
            .await?;

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

    async fn assemble_retrieve_context(
        &self,
        iteration: u8,
        mode: &ModeConfig,
        request: &AgentRequest,
        state: &mut IterationState,
        sink: &dyn AgentEventSink,
    ) -> super::assembler::AssembledContext {
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
        assembled
    }

    async fn call_retrieve_llm(
        &self,
        mode: &ModeConfig,
        state: &mut IterationState,
        total_usage: &mut LlmUsage,
        assembled: &super::assembler::AssembledContext,
        sink: &dyn AgentEventSink,
    ) -> Result<LlmResponse, AppError> {
        let mut round_messages = vec![ChatMessage::system(assembled.system_content.clone())];
        for msg in &state.messages {
            if msg.role != "system" {
                round_messages.push(msg.clone());
            }
        }

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
        Ok(llm_response)
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

        match parsed {
            LlmOutput::NativeToolCalls(calls) => {
                self.dispatch_native_tool_calls(
                    iteration,
                    max_iterations,
                    mode,
                    request,
                    auth,
                    loop_exit,
                    state,
                    optimizer,
                    sink,
                    llm_response,
                    iter_start,
                    calls,
                )
                .await
            }
            LlmOutput::CodeBlocks(codes) => {
                self.dispatch_codegen(
                    iteration,
                    request,
                    auth,
                    state,
                    sink,
                    llm_response,
                    iter_start,
                    codes,
                )
                .await
            }
            LlmOutput::Content(content) => {
                self.dispatch_content(
                    iteration,
                    mode,
                    loop_exit,
                    state,
                    sink,
                    llm_response,
                    iter_start,
                    content,
                )
                .await
            }
        }
    }

    async fn dispatch_content(
        &self,
        iteration: u8,
        mode: &ModeConfig,
        loop_exit: &LoopExitConfig,
        state: &mut IterationState,
        _sink: &dyn AgentEventSink,
        llm_response: &LlmResponse,
        iter_start: std::time::Instant,
        content: String,
    ) -> Result<IterationOutcome, AppError> {
        let llm_usage = iteration_llm_usage(llm_response);
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
                    llm_usage: Some(llm_usage),
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
                    llm_usage: Some(llm_usage),
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
                llm_usage: Some(llm_usage),
                elapsed_ms: iter_start.elapsed().as_millis() as u64,
                exit_reason,
            }),
            sandbox_break: false,
        })
    }
}

pub(super) fn iteration_llm_usage(llm_response: &LlmResponse) -> AgentRunUsage {
    AgentRunUsage {
        provider: llm_response.usage.provider.clone(),
        model: llm_response.model.clone(),
        prompt_tokens: llm_response.usage.prompt_tokens as u64,
        completion_tokens: llm_response.usage.completion_tokens as u64,
        total_tokens: llm_response.usage.total_tokens as u64,
        request_count: 1,
        cached_tokens: llm_response.usage.cached_tokens as u64,
    }
}

pub(super) fn disclosed_skill_ids(disclosed: &DisclosedState) -> Vec<String> {
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
        response.tool_calls = Some(vec![contracts::ToolCall {
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
                .any(|r| r.tool == "dense_retrieval" && r.status == contracts::ToolStatus::Ok),
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
