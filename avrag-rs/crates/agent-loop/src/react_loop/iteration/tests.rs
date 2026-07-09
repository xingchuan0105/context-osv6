use std::sync::Arc;

use super::{IterationControl, IterationOutcome, IterationState};
use crate::AgentKind;
use agent_tools::capability::CapabilityRegistry;
use crate::events::CollectingSink;
use crate::react_loop::ReActLoop;
use crate::react_loop::assembler::DisclosedState;
use avrag_llm::{ChatMessage, LlmClient, LlmResponse, LlmUsage, ModelProviderConfig};

fn rag_mode() -> super::super::config::ModeConfig {
    super::super::config::load_mode_config("rag").unwrap()
}

fn chat_mode() -> super::super::config::ModeConfig {
    super::super::config::load_mode_config("chat").unwrap()
}

fn base_request(kind: AgentKind) -> crate::runtime::AgentRequest {
    crate::runtime::AgentRequest {
        kind,
        query: "test".to_string(),
        workspace_id: None,
        session_id: None,
        doc_scope: vec![],
        messages: vec![],
        user_preferences: None,
        debug: false,
        stream: false,
        language: None,
        auth: crate::runtime::stub_agent_auth(),
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
        total_tool_calls: 0,
        consecutive_sandbox_errors: 0,
        reasoning_acc: String::new(),
    }
}

fn test_auth() -> contracts::auth_runtime::AuthContext {
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
            &mode,
            &base_request(AgentKind::Search),
            &auth,
            &mode.loop_exit_for_mode(),
            &mut state,
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
    use avrag_rag_core::RagRuntime;
    use uuid::Uuid;

    struct StubDataPlane {
        chunk_id: Uuid,
        doc_id: Uuid,
    }

    #[async_trait::async_trait]
    impl avrag_retrieval_data_plane::RetrievalReadPort for StubDataPlane {
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
    let data_plane: Arc<dyn avrag_retrieval_data_plane::RetrievalReadPort> =
        Arc::new(StubDataPlane { chunk_id, doc_id });
    let config = avrag_rag_core::RagConfig::new_for_data_plane(embedding, None);
    let runtime = Arc::new(RagRuntime::with_data_plane(config, data_plane));

    let loop_ = test_loop().with_rag_runtime(Some(runtime));
    let mode = rag_mode();
    let mut state = empty_state();
    let sink = CollectingSink::new();
    let auth = test_auth();
    let mut request = base_request(AgentKind::Rag);
    request.doc_scope = vec![doc_id.to_string()];

    let response = fake_llm_response(
        r#"<code language="python">chunks = await client.dense_search(query="antifragility", top_k=10)</code>"#,
    );

    let _outcome = loop_
        .apply_llm_output(
            0,
            &mode,
            &request,
            &auth,
            &mode.loop_exit_for_mode(),
            &mut state,
            &sink,
            &response,
            std::time::Instant::now(),
        )
        .await
        .unwrap();

    let observation = state
        .messages
        .iter()
        .find(|m| m.content.contains("<code_execution_result"))
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
    let auth = test_auth();
    let response = fake_llm_response(r#"<code language="python">print("ok")</code>"#);

    let outcome = loop_
        .apply_llm_output(
            0,
            &mode,
            &base_request(AgentKind::Rag),
            &auth,
            &mode.loop_exit_for_mode(),
            &mut state,
            &sink,
            &response,
            std::time::Instant::now(),
        )
        .await
        .unwrap();

    assert!(matches!(outcome.control, IterationControl::Continue));
    assert_eq!(outcome.record.as_ref().unwrap().exit_reason, "code_gen");
    assert!(
        state
            .messages
            .iter()
            .any(|m| m.content.contains("code_execution_result"))
    );
}

#[tokio::test]
async fn consecutive_code_errors_break_to_synthesis() {
    let loop_ = test_loop();
    let mode = rag_mode();
    let mut state = empty_state();
    state.consecutive_sandbox_errors = 1;
    let sink = CollectingSink::new();
    let auth = test_auth();
    let response =
        fake_llm_response(r#"<code language="python">raise RuntimeError("fail")</code>"#);

    let outcome = loop_
        .apply_llm_output(
            1,
            &mode,
            &base_request(AgentKind::Rag),
            &auth,
            &mode.loop_exit_for_mode(),
            &mut state,
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
    let auth = test_auth();
    let response = fake_llm_response("Here is your answer.");

    let outcome = loop_
        .apply_llm_output(
            0,
            &mode,
            &base_request(AgentKind::Chat),
            &auth,
            &mode.loop_exit_for_mode(),
            &mut state,
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
    assert_eq!(
        outcome.record.as_ref().unwrap().exit_reason,
        "direct_content"
    );
}

#[tokio::test]
async fn content_without_evidence_in_rag_is_blocked() {
    let loop_ = test_loop();
    let mode = rag_mode();
    let mut state = empty_state();
    let sink = CollectingSink::new();
    let auth = test_auth();
    let response = fake_llm_response("Answer without retrieval.");

    let outcome = loop_
        .apply_llm_output(
            0,
            &mode,
            &base_request(AgentKind::Rag),
            &auth,
            &mode.loop_exit_for_mode(),
            &mut state,
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
    assert!(
        state
            .messages
            .iter()
            .any(|m| { m.role == "user" && m.content.contains("retrieve evidence") })
    );
}

#[tokio::test]
async fn skill_request_json_in_chat_is_not_direct_answer() {
    let loop_ = test_loop();
    let mode = chat_mode();
    let mut state = empty_state();
    let sink = CollectingSink::new();
    let auth = test_auth();
    let response = fake_llm_response(r#"{"skill_request":["memory"]}"#);

    let outcome = loop_
        .apply_llm_output(
            0,
            &mode,
            &base_request(AgentKind::Chat),
            &auth,
            &mode.loop_exit_for_mode(),
            &mut state,
            &sink,
            &response,
            std::time::Instant::now(),
        )
        .await
        .unwrap();

    assert!(matches!(outcome.control, IterationControl::Continue));
    assert_eq!(
        outcome.record.as_ref().unwrap().exit_reason,
        "skill_request"
    );
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
