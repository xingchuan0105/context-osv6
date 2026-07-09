//! Real-LLM HeavyTail write-mode E2E (multi-phase pipeline).
//!
//! Expensive (~10–20 LLM calls). Gated by `E2E_MODE=nightly` and `#[ignore]`.

use crate::product_e2e::{
    ChatStreamParams, TestContext,
    assertions::assert_answer_substantive,
    llm_real::{
        REAL_LLM_STREAM_DEADLINE, REAL_LLM_STREAM_MAX_EVENTS, collect_observability_from_events,
        load_env_from_repo_dotenv, merge_llm_real_extra, parse_chat_response_from_stream_events,
        require_nightly_suite, require_real_llm_config,
    },
};

#[tokio::test]
#[ignore = "requires real LLM API; multi-phase write pipeline (~10+ calls); run with --ignored --test-threads=1"]
async fn real_llm_write_mode_produces_article_with_fingerprint() {
    require_nightly_suite();
    load_env_from_repo_dotenv();
    require_real_llm_config();

    let ctx = TestContext::new_with_real_llm().await;
    let notebook = ctx.create_notebook("write-real").await.expect("notebook");

    let params = ChatStreamParams {
        query: "用三百字左右介绍量子纠缠的基本概念，面向普通读者。",
        agent_type: "write",
        notebook_id: &notebook.id,
        doc_scope: &[],
        session_id: None,
        format_hint: None,
        debug: true,
        pin_mock_chunk_ids: false,
    };

    let events = ctx
        .chat_stream_with_params(
            params,
            REAL_LLM_STREAM_MAX_EVENTS,
            REAL_LLM_STREAM_DEADLINE,
        )
        .await
        .expect("write stream");

    let capture = collect_observability_from_events(&events);
    let resp = parse_chat_response_from_stream_events(&events).expect("terminal done payload");

    assert_eq!(resp.agent_type, "write");
    assert_answer_substantive(&resp, 80);

    let debug = resp
        .mode_debug
        .as_ref()
        .or(resp.planner_output.as_ref().map(|_| &serde_json::Value::Null));
    let _ = debug;

    ctx.save_llm_artifact(
        "real_llm_write_mode_produces_article_with_fingerprint",
        &resp,
        merge_llm_real_extra(
            &crate::product_e2e::llm_real::LlmRealChatResult {
                resp: resp.clone(),
                reasoning: capture.clone(),
                stream_error_with_done: false,
            },
            Some(serde_json::json!({
                "write_activity_stages": events
                    .iter()
                    .filter(|e| e.event == "activity")
                    .filter_map(|e| e.data.get("stage").and_then(|v| v.as_str()))
                    .collect::<Vec<_>>(),
            })),
        ),
        Some(capture),
    );
}
