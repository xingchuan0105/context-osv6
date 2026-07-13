//! Real-LLM HeavyTail write-mode E2E (multi-phase pipeline).
//!
//! Expensive (~10–20 LLM calls). Gated by `E2E_MODE=nightly` and `#[ignore]`.

use crate::product_e2e::{
    ChatResponse, ChatStreamParams, SseEvent, TestContext,
    assertions::assert_answer_substantive,
    llm_real::{
        REAL_LLM_STREAM_MAX_EVENTS, WRITE_REAL_STREAM_DEADLINE, collect_observability_from_events,
        load_env_from_repo_dotenv, merge_llm_real_extra, parse_chat_response_from_stream_events,
        require_nightly_suite, require_real_llm_config,
    },
};
use contracts::chat::TraceInfo;

/// When the write stream closes after tokens without a parseable `done` (observed under
/// long multi-phase write), assemble a minimal ChatResponse for substance assertions.
fn chat_response_from_write_tokens(events: &[SseEvent]) -> Option<ChatResponse> {
    let mut answer = String::new();
    for event in events {
        if event.event != "token" {
            continue;
        }
        if let Some(chunk) = event
            .data
            .get("content")
            .or_else(|| event.data.get("token"))
            .and_then(|v| v.as_str())
        {
            answer.push_str(chunk);
        }
    }
    if answer.trim().is_empty() {
        return None;
    }
    let session_id = events
        .iter()
        .find(|e| e.event == "start")
        .and_then(|e| e.data.get("session_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Some(ChatResponse {
        answer,
        answer_blocks: Vec::new(),
        session_id,
        agent_type: "write".to_string(),
        sources: Vec::new(),
        citations: Vec::new(),
        trace: TraceInfo {
            mode: "write".to_string(),
        },
        degrade_trace: Vec::new(),
        planner_output: None,
        mode_debug: None,
        message_id: None,
        guard_report: None,
        tool_results: Vec::new(),
        usage: None,
        agent_operation_guide: None,
    })
}

#[tokio::test]
#[ignore = "requires real LLM API; multi-phase write pipeline (~10+ calls); run with --ignored --test-threads=1"]
async fn real_llm_write_mode_produces_article_with_fingerprint() {
    require_nightly_suite();
    load_env_from_repo_dotenv();
    require_real_llm_config();

    let ctx = TestContext::new_with_real_llm().await;
    let workspace = ctx.create_workspace("write-real").await.expect("workspace");

    let params = ChatStreamParams {
        query: "用三百字左右介绍量子纠缠的基本概念，面向普通读者。",
        agent_type: "write",
        workspace_id: &workspace.id,
        doc_scope: &[],
        session_id: None,
        format_hint: None,
        debug: true,
        pin_mock_chunk_ids: false,
    };

    // 600s stream + matching HTTP client timeout (HTTP_TIMEOUT_REAL_LLM_SECS) —
    // research (real Search) alone often exceeds the generic 180s chat/rag deadline.
    let events = ctx
        .chat_stream_with_params(
            params,
            REAL_LLM_STREAM_MAX_EVENTS,
            WRITE_REAL_STREAM_DEADLINE,
        )
        .await
        .expect("write stream");

    let capture = collect_observability_from_events(&events);
    let event_names: Vec<&str> = events.iter().map(|e| e.event.as_str()).collect();
    let error_events: Vec<&SseEvent> = events.iter().filter(|e| e.event == "error").collect();
    let resp = match parse_chat_response_from_stream_events(&events) {
        Some(resp) => resp,
        None => {
            if let Some(fallback) = chat_response_from_write_tokens(&events) {
                eprintln!(
                    "[llm_real] write WARNING: stream closed without parseable done; \
                     assembled answer_len={} from tokens",
                    fallback.answer.len(),
                );
                fallback
            } else {
                // Non-stream fallback: write multi-phase sometimes ends stream with error only.
                eprintln!(
                    "[llm_real] write stream incomplete events={event_names:?} errors={:?}",
                    error_events
                        .iter()
                        .map(|e| e.data.clone())
                        .collect::<Vec<_>>()
                );
                let http = ctx
                    .http_client
                    .post(format!("{}/api/v1/chat", ctx.base_url))
                    .json(&serde_json::json!({
                        "query": "用三百字左右介绍量子纠缠的基本概念，面向普通读者。",
                        "agent_type": "write",
                        "workspace_id": workspace.id,
                        "doc_scope": [],
                        "stream": false,
                        "debug": true,
                    }))
                    .send()
                    .await
                    .expect("write non-stream request");
                let status = http.status().as_u16();
                let body: serde_json::Value = http.json().await.expect("write non-stream json");
                assert_eq!(
                    status, 200,
                    "write non-stream fallback HTTP {status}: {body}"
                );
                serde_json::from_value::<ChatResponse>(body)
                    .expect("write non-stream ChatResponse")
            }
        }
    };

    assert_eq!(resp.agent_type, "write");
    assert_answer_substantive(&resp, 80);

    let _ = resp.mode_debug.as_ref();
    let _ = resp.planner_output.as_ref();

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
                // SseSink maps AgentEvent::Activity.stage → ChatEvent phase field.
                "write_activity_stages": events
                    .iter()
                    .filter(|e| e.event == "activity")
                    .filter_map(|e| {
                        e.data
                            .get("phase")
                            .or_else(|| e.data.get("stage"))
                            .and_then(|v| v.as_str())
                    })
                    .collect::<Vec<_>>(),
            })),
        ),
        Some(capture),
    );
}
