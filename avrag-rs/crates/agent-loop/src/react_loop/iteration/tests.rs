use std::sync::Arc;

use super::{IterationControl, IterationState};
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
        answer_deltas_streamed: false,
        compile_continuations: 0,
        retrieval_aliases: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
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

/// Q55 pollution: model emits dense_search as native tool — hard-reject with
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
            tool: "dense_search".to_string(),
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
        assert_eq!(err, "not_a_native_tool", "{r:?}");
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
        state.messages.iter().any(|m| m.content.contains("not_a_native_tool")
            || m.content.contains("client.dense_search")
            || m.content.contains("await client.")),
        "state messages should carry rejection: {:?}",
        state.messages.iter().map(|m| &m.content).collect::<Vec<_>>()
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
    assert!(observation.contains("跳过了 2 个"), "{observation}");
    assert!(observation.contains("每轮只输出一个"), "{observation}");
    // Only the executed block counts as a tool call.
    assert_eq!(state.total_tool_calls, 1);
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
    // Policy guard coverage with early-stop disallowed. Note: modes/rag.yaml
    // itself now ALLOWS no-evidence early stop (PR-A 2026-07-20) so empty-result
    // workers can still finish their handoff JSON; this test pins the guard
    // under the strict config (require evidence + no early stop).
    let mut mode = rag_mode();
    mode.loop_exit.allow_content_early_stop = false;
    mode.loop_exit.skip_synthesis_on_direct_answer = false;
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

/// PR-A (2026-07-20): rag.yaml default allows a no-evidence early stop so a
/// worker that found nothing can still emit its handoff JSON as the final
/// message (coverage=gaps), instead of being force-looped.
#[tokio::test]
async fn content_without_evidence_in_rag_early_stops_by_default() {
    let loop_ = test_loop();
    let mode = rag_mode();
    assert!(mode.loop_exit.allow_content_early_stop);
    assert!(mode.loop_exit.skip_synthesis_on_direct_answer);
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
        )
        .await
        .unwrap();

    assert!(
        !matches!(outcome.control, IterationControl::Continue),
        "no-evidence content must not be blocked under PR-A defaults"
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


// ---------------------------------------------------------------------------
// S2: worker-handoff output compiler at the direct_content decision point
// ---------------------------------------------------------------------------

/// Worker loop config: rag mode with the handoff flag set (mirrors app-chat
/// `apply_worker_handoff_loop_exit`: early stop allowed, skip synthesis).
fn worker_mode() -> super::super::config::ModeConfig {
    let mut mode = rag_mode();
    mode.worker_handoff = true;
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
        )
        .await
        .unwrap()
}

/// K3 (a): zero-retrieval insufficient JSON → E105 compile feedback, loop
/// continues; a prose final message is then accepted (prose is a legal
/// handoff now — no JSON envelope needed).
#[tokio::test]
async fn worker_handoff_e105_triggers_feedback_then_prose_accepted() {
    let loop_ = test_loop();
    let mode = worker_mode();
    let mut state = empty_state();

    let bad = r#"{"schema_version":"internal_worker_handoff_v1","summary":"未找到","key_facts":[],"coverage":"insufficient","gaps":["x"]}"#;
    let outcome = apply_content(&loop_, &mode, &mut state, bad).await;
    assert!(matches!(outcome.control, IterationControl::Continue));
    assert_eq!(
        outcome.record.as_ref().unwrap().exit_reason,
        "compile_feedback"
    );
    assert_eq!(state.compile_continuations, 1);
    let feedback = state.messages.last().unwrap();
    assert_eq!(feedback.role, "user");
    assert!(feedback.content.contains("编译失败"), "{feedback:?}");
    assert!(feedback.content.contains("E105"), "{feedback:?}");
    assert!(feedback.content.contains("请按契约重新输出"), "{feedback:?}");

    // Next output: plain prose — legal handoff under K3, accepted directly.
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

/// (b) one-continuation limit: a second E105 output is NOT continued again
/// (no infinite compile loop) — it falls through as the final direct answer
/// and the post-loop compile marks it degraded with codes.
#[tokio::test]
async fn worker_handoff_compile_feedback_only_once() {
    let loop_ = test_loop();
    let mode = worker_mode();
    let mut state = empty_state();

    let bad = r#"{"schema_version":"internal_worker_handoff_v1","summary":"未找到","key_facts":[],"coverage":"insufficient","gaps":[]}"#;
    let first = apply_content(&loop_, &mode, &mut state, bad).await;
    assert!(matches!(first.control, IterationControl::Continue));
    assert_eq!(state.compile_continuations, 1);

    let second = apply_content(&loop_, &mode, &mut state, bad).await;
    assert!(
        matches!(second.control, IterationControl::DirectAnswer { .. }),
        "second bad output must not continue again"
    );
    assert_eq!(
        second.record.as_ref().unwrap().exit_reason,
        "direct_content"
    );
    assert_eq!(state.compile_continuations, 1, "counter stays at the cap");
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
    assert!(matches!(outcome.control, IterationControl::DirectAnswer { .. }));
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
    for reason in ["direct_content", "content_blocked_no_evidence", "code_gen", "skill_request"] {
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
    assert_eq!(super::COMPILE_FEEDBACK_EXIT_REASON, "compile_feedback");
}
