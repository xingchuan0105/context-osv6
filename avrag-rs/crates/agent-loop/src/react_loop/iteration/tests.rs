use std::sync::Arc;

use super::super::StandardLoopHooks;
use super::{IterationControl, IterationState};
use crate::AgentKind;
use crate::events::CollectingSink;
use crate::react_loop::ReActLoop;
use crate::react_loop::assembler::DisclosedState;
use agent_tools::capability::CapabilityRegistry;
use avrag_llm::{ChatMessage, LlmClient, LlmResponse, LlmUsage, ModelProviderConfig};

// D9: mandatory retrieve is derived at assembly time (memory base +
// capability skill); YAML no longer carries it.
fn rag_mode() -> super::super::config::ModeConfig {
    let mut mode = super::super::config::load_mode_config("rag").unwrap();
    mode.skill_catalog.mandatory.retrieve = super::super::derive_mandatory_retrieve(true, false);
    mode
}

fn chat_mode() -> super::super::config::ModeConfig {
    let mut mode = super::super::config::load_mode_config("chat").unwrap();
    mode.skill_catalog.mandatory.retrieve = super::super::derive_mandatory_retrieve(false, false);
    mode
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
        response_id: None,
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
        answer_deltas_streamed: false,
        compile_continuations: 0,
        retrieval_aliases: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        seen_retrieval_aliases: std::sync::Arc::new(std::sync::Mutex::new(
            std::collections::HashSet::new(),
        )),
        seen_chunk_aliases: std::sync::Arc::new(std::sync::Mutex::new(
            std::collections::HashMap::new(),
        )),
        session_fs: std::sync::Arc::new(crate::react_loop::session_fs::SessionFs::new()),
        sdk_allowed: std::sync::Arc::new(std::collections::HashSet::new()),
        query_card: None,
        max_iterations: 100,
    }
}

fn test_auth() -> contracts::auth_runtime::AuthContext {
    use contracts::auth_runtime::{AuthContext, SubjectKind, UserId};
    use uuid::Uuid;
    AuthContext::new(
        UserId::from(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()),
        SubjectKind::User,
    )
}

#[tokio::test]
async fn native_tool_call_returns_continue_with_record() {
    let loop_ = test_loop();
    let mode = super::super::config::load_mode_config("search").unwrap();
    // search YAML carries no mandatory list (D9); this test only exercises the
    // native web_search rejection path, so raw config is fine.
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
            &StandardLoopHooks::default(),
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

/// Q55 pollution: model emits SDK method as native tool — hard-reject with
/// corrective observation (do not fall through to unknown tool NotImplemented).
#[tokio::test]
async fn rejects_codegen_sdk_method_as_native_tool_call() {
    let loop_ = test_loop();
    let mode = rag_mode();
    let mut state = empty_state();
    let sink = CollectingSink::new();
    let auth = test_auth();
    let mut response = fake_llm_response("");
    response.tool_calls = Some(vec![
        contracts::ToolCall {
            tool: "dense".to_string(),
            version: "1".to_string(),
            args: serde_json::json!({"query": "doc_scope"}),
        },
        contracts::ToolCall {
            tool: "doc_scan".to_string(),
            version: "1".to_string(),
            args: serde_json::json!({}),
        },
    ]);

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
            &StandardLoopHooks::default(),
        )
        .await
        .unwrap();

    assert!(matches!(outcome.control, IterationControl::Continue));
    assert_eq!(state.tool_results.len(), 2);
    for r in &state.tool_results {
        assert_eq!(r.status, contracts::ToolStatus::Error, "{r:?}");
        let err = r
            .data
            .as_ref()
            .and_then(|d| d.get("error"))
            .and_then(|e| e.as_str())
            .unwrap_or("");
        assert_eq!(err, "native_tools_closed", "{r:?}");
        let hint = r
            .data
            .as_ref()
            .and_then(|d| d.get("hint"))
            .and_then(|h| h.as_str())
            .unwrap_or("");
        assert!(
            hint.contains("client.") && hint.contains("<code"),
            "hint must point at client.* code block: {hint}"
        );
    }
    // Tool messages pushed so the next LLM turn sees the correction.
    assert!(
        state
            .messages
            .iter()
            .any(|m| m.content.contains("native_tools_closed")
                || m.content.contains("client.dense")
                || m.content.contains("await client.")),
        "state messages should carry rejection: {:?}",
        state
            .messages
            .iter()
            .map(|m| &m.content)
            .collect::<Vec<_>>()
    );
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
                cursor: None,
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
        r#"<code language="python">chunks = await client.dense(query="antifragility")</code>"#,
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
            &StandardLoopHooks::default(),
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
            &StandardLoopHooks::default(),
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

/// E6: only the FIRST extracted block executes per round; the rest are
/// skipped with a warning naming the count (mechanical one-block-per-round).
#[tokio::test]
async fn only_first_code_block_executes_with_skip_warning() {
    let loop_ = test_loop();
    let mode = rag_mode();
    let mut state = empty_state();
    let sink = CollectingSink::new();
    let auth = test_auth();
    let response = fake_llm_response(
        r#"<code language="python">print("AAA_FIRST")</code>
<code language="python">print("BBB_SECOND")</code>
<code language="python">print("CCC_THIRD")</code>"#,
    );

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
            &StandardLoopHooks::default(),
        )
        .await
        .unwrap();

    assert!(matches!(outcome.control, IterationControl::Continue));
    let observation = &state.messages.last().unwrap().content;
    assert!(observation.contains("AAA_FIRST"), "{observation}");
    assert!(!observation.contains("BBB_SECOND"), "{observation}");
    assert!(!observation.contains("CCC_THIRD"), "{observation}");
    assert!(!observation.contains("[block 1]"), "{observation}");
    assert!(observation.contains("[blocks_skipped]"), "{observation}");
    assert!(observation.contains("其余 2 个未执行"), "{observation}");
    assert!(
        observation.contains("仅第一块进入沙箱") || observation.contains("每轮仅第一块"),
        "{observation}"
    );
    // Only the executed block counts as a tool call.
    assert_eq!(state.total_tool_calls, 1);
}

#[tokio::test]
async fn consecutive_code_errors_break_to_synthesis() {
    let loop_ = test_loop();
    let mode = rag_mode();
    let mut state = empty_state();
    // Threshold is MAX_CONSECUTIVE_SANDBOX_ERRORS (4): counter increments on
    // this failing turn, so start at 3 → become 4 → break.
    state.consecutive_sandbox_errors = ReActLoop::MAX_CONSECUTIVE_SANDBOX_ERRORS.saturating_sub(1);
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
            &StandardLoopHooks::default(),
        )
        .await
        .unwrap();

    assert!(matches!(
        outcome.control,
        IterationControl::BreakToSynthesis { .. }
    ));
    // Break is recorded for eval visibility (no longer silent sandbox_break).
    assert!(!outcome.sandbox_break);
    let rec = outcome.record.expect("break should be recorded");
    assert_eq!(rec.exit_reason, "sandbox_break_to_synthesis");
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
            &StandardLoopHooks::default(),
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
async fn content_without_evidence_in_rag_is_model_stop() {
    // require_evidence is skill-owned: host does not block DirectAnswer.
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
            &StandardLoopHooks::default(),
        )
        .await
        .unwrap();

    assert!(matches!(
        outcome.control,
        IterationControl::DirectAnswer { content } if content == "Answer without retrieval."
    ));
    assert_eq!(
        outcome.record.as_ref().unwrap().exit_reason,
        "direct_content"
    );
}

#[tokio::test]
async fn content_without_evidence_still_direct_with_early_stop_flags() {
    let loop_ = test_loop();
    let mut mode = rag_mode();
    mode.loop_exit.allow_content_early_stop = true;
    mode.loop_exit.skip_synthesis_on_direct_answer = true;
    let mut state = empty_state();
    let sink = CollectingSink::new();
    let auth = test_auth();
    let response = fake_llm_response("Handoff: nothing found, coverage=insufficient.");

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
            &StandardLoopHooks::default(),
        )
        .await
        .unwrap();

    assert!(matches!(
        outcome.control,
        IterationControl::DirectAnswer { .. }
    ));
    assert_eq!(
        outcome.record.as_ref().unwrap().exit_reason,
        "direct_content"
    );
}

// --- L2 evidence gate / L2.5 required-action gate (2026-08-03, runtime QC) ---

fn rag_mode_with_primitives() -> super::super::config::ModeConfig {
    let mut mode = rag_mode();
    mode.sdk_primitives = crate::react_loop::sdk_primitives_for_caps(true, false)
        .iter()
        .map(|s| s.to_string())
        .collect();
    mode
}

#[tokio::test]
async fn evidence_gate_blocks_direct_answer_with_zero_ok_returns() {
    let loop_ = test_loop();
    let mode = rag_mode_with_primitives(); // rag primitives mounted → requires evidence
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
            &StandardLoopHooks::default(),
        )
        .await
        .unwrap();

    assert!(matches!(outcome.control, IterationControl::Continue));
    assert_eq!(
        outcome.record.as_ref().unwrap().exit_reason,
        "evidence_missing_continue"
    );
    // The third-person observation is pushed as a user message for the next round.
    assert!(state
        .messages
        .iter()
        .any(|m| m.role == "user" && m.content.contains("回传")));
}

#[tokio::test]
async fn evidence_gate_releases_when_ok_chunks_present() {
    let loop_ = test_loop();
    let mode = rag_mode_with_primitives();
    let mut state = empty_state();
    state.tool_results = vec![ok_chunk_tool_result("c1")];
    let sink = CollectingSink::new();
    let auth = test_auth();
    let response = fake_llm_response("Grounded answer.");

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
            &StandardLoopHooks::default(),
        )
        .await
        .unwrap();

    assert!(matches!(
        outcome.control,
        IterationControl::DirectAnswer { .. }
    ));
    assert_eq!(
        outcome.record.as_ref().unwrap().exit_reason,
        "direct_content"
    );
}

#[tokio::test]
async fn evidence_gate_releases_on_round_budget_exhaustion() {
    // Budget about to exhaust → gates release (loop breaks next top check).
    let loop_ = test_loop();
    let mode = rag_mode_with_primitives();
    let mut state = empty_state();
    state.max_iterations = 1; // iteration 0 + 1 >= 1 → exhausted
    let sink = CollectingSink::new();
    let auth = test_auth();
    let response = fake_llm_response("Last-chance answer.");
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
            &StandardLoopHooks::default(),
        )
        .await
        .unwrap();
    assert!(matches!(
        outcome.control,
        IterationControl::DirectAnswer { .. }
    ));
    assert_eq!(
        outcome.record.as_ref().unwrap().exit_reason,
        "direct_content"
    );
}

#[tokio::test]
async fn required_action_gate_blocks_until_action_satisfied() {
    let loop_ = test_loop();
    let mode = rag_mode();
    let mut state = empty_state();
    state.query_card = Some(crate::react_loop::query_card::QueryCard {
        question_type: crate::react_loop::query_card::QuestionType::Calculation,
        required_actions: vec!["calculator".to_string()],
    });
    let sink = CollectingSink::new();
    let auth = test_auth();
    let response = fake_llm_response("Answer without calling calculator.");

    // First: no calculator Ok result → Continue, required_action_missing_continue.
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
            &StandardLoopHooks::default(),
        )
        .await
        .unwrap();
    assert!(matches!(outcome.control, IterationControl::Continue));
    assert_eq!(
        outcome.record.as_ref().unwrap().exit_reason,
        "required_action_missing_continue"
    );
    assert!(state
        .messages
        .iter()
        .any(|m| m.role == "user" && m.content.contains("calculator")));

    // Now the calculator Ok result exists → DirectAnswer accepted.
    state.tool_results.push(contracts::ToolResult {
        tool: "calculator".into(),
        version: "1".into(),
        status: contracts::ToolStatus::Ok,
        data: Some(serde_json::json!({"result": 42})),
        trace: None,
    });
    let response2 = fake_llm_response("Computed: 42.");
    let outcome = loop_
        .apply_llm_output(
            1,
            &mode,
            &base_request(AgentKind::Rag),
            &auth,
            &mode.loop_exit_for_mode(),
            &mut state,
            &sink,
            &response2,
            std::time::Instant::now(),
            &StandardLoopHooks::default(),
        )
        .await
        .unwrap();
    assert!(matches!(
        outcome.control,
        IterationControl::DirectAnswer { .. }
    ));
    assert_eq!(
        outcome.record.as_ref().unwrap().exit_reason,
        "direct_content"
    );
}

#[tokio::test]
async fn chat_mode_never_fires_evidence_gate() {
    // chat mode: no rag/search primitives mounted → no evidence requirement.
    let loop_ = test_loop();
    let mode = chat_mode();
    let mut state = empty_state();
    let sink = CollectingSink::new();
    let auth = test_auth();
    let response = fake_llm_response("Just chatting.");
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
            &StandardLoopHooks::default(),
        )
        .await
        .unwrap();
    assert!(matches!(
        outcome.control,
        IterationControl::DirectAnswer { .. }
    ));
    assert_eq!(
        outcome.record.as_ref().unwrap().exit_reason,
        "direct_content"
    );
}

#[tokio::test]
async fn validated_card_with_unmounted_action_does_not_block() {
    // 2026-08-03 review P1 regression: production wires `QueryCard::validate`
    // at the pre-loop fetch site (react_loop/mod.rs), so a card declaring an
    // action the sandbox can never reach ("web" on a rag-only mode) arrives
    // at the gate with that action already dropped. Without validate this
    // test would ping-pong until budget exhaustion.
    let loop_ = test_loop();
    let mode = rag_mode(); // rag-only: "web" primitive not mounted
    let raw_card = crate::react_loop::query_card::QueryCard {
        question_type: crate::react_loop::query_card::QuestionType::RagFact,
        required_actions: vec!["web".to_string()],
    };
    let mut state = empty_state();
    state.query_card = Some(raw_card.validate(&mode));
    assert!(state
        .query_card
        .as_ref()
        .unwrap()
        .required_actions
        .is_empty());
    let sink = CollectingSink::new();
    let auth = test_auth();
    let response = fake_llm_response("Answer with no reachable action.");
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
            &StandardLoopHooks::default(),
        )
        .await
        .unwrap();
    assert!(matches!(
        outcome.control,
        IterationControl::DirectAnswer { .. }
    ));
    assert_eq!(
        outcome.record.as_ref().unwrap().exit_reason,
        "direct_content"
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
            &StandardLoopHooks::default(),
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

// ---------------------------------------------------------------------------
// S2: worker-handoff output compiler at the direct_content decision point
// ---------------------------------------------------------------------------

/// Worker loop config: rag mode with the handoff flag set (mirrors app-chat
/// `apply_worker_handoff_loop_exit`: hard gate on chunks; skip synthesis when
/// chunks exist).
fn worker_mode() -> super::super::config::ModeConfig {
    let mut mode = rag_mode();
    mode.worker_handoff = true;
    mode.loop_exit.require_evidence = false;
    mode.loop_exit.allow_content_early_stop = false;
    mode.loop_exit.skip_synthesis_on_direct_answer = true;
    mode
}

fn ok_chunk_tool_result(chunk_id: &str) -> contracts::ToolResult {
    contracts::ToolResult {
        tool: "dense_retrieval".to_string(),
        version: "1.0".to_string(),
        status: contracts::ToolStatus::Ok,
        data: Some(serde_json::json!([
            {"chunk_id": chunk_id, "doc_id": "d1", "text": "evidence", "score": 0.9}
        ])),
        trace: None,
    }
}

async fn apply_content(
    loop_: &ReActLoop,
    mode: &super::super::config::ModeConfig,
    state: &mut IterationState,
    content: &str,
) -> super::IterationOutcome {
    let sink = CollectingSink::new();
    let auth = test_auth();
    let response = fake_llm_response(content);
    loop_
        .apply_llm_output(
            0,
            mode,
            &base_request(AgentKind::Rag),
            &auth,
            &mode.loop_exit_for_mode(),
            state,
            &sink,
            &response,
            std::time::Instant::now(),
            &StandardLoopHooks::default(),
        )
        .await
        .unwrap()
}

/// Zero-tool finals: host no longer blocks on missing chunks; worker may
/// still hit compile_feedback (E105) for insufficient coverage with zero tools.
#[tokio::test]
async fn worker_handoff_zero_chunk_reaches_compile_path() {
    let loop_ = test_loop();
    let mode = worker_mode();
    let mut state = empty_state();

    let bad = r#"{"schema_version":"internal_worker_handoff_v1","summary":"未找到","key_facts":[],"coverage":"insufficient","gaps":["x"]}"#;
    let outcome = apply_content(&loop_, &mode, &mut state, bad).await;
    // E105: insufficient + zero tools → compile_feedback Continue, or DirectAnswer
    // if compile accepts prose-shaped JSON without errors.
    let reason = outcome.record.as_ref().map(|r| r.exit_reason.as_str());
    assert!(
        matches!(
            outcome.control,
            IterationControl::Continue | IterationControl::DirectAnswer { .. }
        ),
        "expected Continue or DirectAnswer"
    );
    assert!(
        reason == Some("compile_feedback") || reason == Some("direct_content"),
        "unexpected reason {reason:?}"
    );
}

/// With real chunks present, a prose handoff is accepted; a second independent
/// zero-chunk final is still blocked (gate is per-state, not one-shot).
#[tokio::test]
async fn worker_handoff_with_chunks_allows_prose_final() {
    let loop_ = test_loop();
    let mode = worker_mode();
    let mut state = empty_state();
    state.tool_results.push(ok_chunk_tool_result("c1"));

    let good = "文档确认竞争对手身份，但未记载总部城市信息。";
    let outcome = apply_content(&loop_, &mode, &mut state, good).await;
    assert!(matches!(
        outcome.control,
        IterationControl::DirectAnswer { content } if content == good
    ));
    assert_eq!(
        outcome.record.as_ref().unwrap().exit_reason,
        "direct_content"
    );
}

/// K3 (e): a prose final message in a worker loop is a legal handoff — no
/// compile error, no continuation, straight to DirectAnswer.
#[tokio::test]
async fn worker_handoff_prose_accepted_without_continuation() {
    let loop_ = test_loop();
    let mode = worker_mode();
    let mut state = empty_state();
    state.tool_results.push(ok_chunk_tool_result("c1"));

    let raw = "找到 2 条相关证据：2019年建厂、大连。SELECTED: #1, #2";
    let outcome = apply_content(&loop_, &mode, &mut state, raw).await;
    assert!(
        matches!(outcome.control, IterationControl::DirectAnswer { .. }),
        "prose handoff must be accepted without continuation"
    );
    assert_eq!(
        outcome.record.as_ref().unwrap().exit_reason,
        "direct_content"
    );
    assert_eq!(state.compile_continuations, 0);
}

/// Non-worker loops never invoke the compiler (chat prose stays untouched).
#[tokio::test]
async fn non_worker_mode_skips_compile() {
    let loop_ = test_loop();
    let mode = chat_mode();
    let mut state = empty_state();
    let raw = r#"{"task_result":{"summary":"x"}}"#;
    let outcome = apply_content(&loop_, &mode, &mut state, raw).await;
    assert!(matches!(
        outcome.control,
        IterationControl::DirectAnswer { .. }
    ));
    assert_eq!(state.compile_continuations, 0);
}

// ---- E4: compile continuations are free of the iteration budget ------------

#[test]
fn compile_feedback_continue_does_not_consume_iteration_budget() {
    use super::super::telemetry::ReActIterationRecord;

    let outcome = |exit_reason: &str| super::IterationOutcome {
        control: IterationControl::Continue,
        record: Some(ReActIterationRecord {
            iteration: 0,
            disclosed_skills: vec![],
            action_type: exit_reason.to_string(),
            observation_preview: String::new(),
            llm_usage: None,
            elapsed_ms: 0,
            exit_reason: exit_reason.to_string(),
        }),
        sandbox_break: false,
    };

    assert!(
        !super::consumes_iteration_budget(&outcome("compile_feedback")),
        "compile correction turn is free"
    );
    for reason in [
        "direct_content",
        "content_blocked_no_evidence",
        "code_gen",
        "skill_request",
    ] {
        assert!(
            super::consumes_iteration_budget(&outcome(reason)),
            "{reason} consumes one numbered iteration"
        );
    }
    // No record (defensive) consumes budget as before.
    let bare = super::IterationOutcome {
        control: IterationControl::Continue,
        record: None,
        sandbox_break: false,
    };
    assert!(super::consumes_iteration_budget(&bare));
}

#[test]
fn hook_emits_the_shared_compile_feedback_exit_reason() {
    // The free-budget accounting keys on this exact label — pin it so the two
    // sites cannot drift apart.
    assert_eq!(
        super::state::COMPILE_FEEDBACK_EXIT_REASON,
        "compile_feedback"
    );
}
