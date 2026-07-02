//! SSE streaming chat coverage — observability and mock-agent behaviour.
//!
//! Protocol invariants (`start` first, `done` terminal, payload shape) live in
//! `transport-http` contract tests. This module exercises the full mock RAG
//! pipeline and loop telemetry with a module-scoped ingested fixture.

use std::time::Duration;

use crate::product_e2e::{
    ChatStreamParams, fixtures::shared_ready_rag_context,
    llm_real::collect_observability_from_events,
};

const STREAM_DEADLINE: Duration = Duration::from_secs(60);
const MAX_EVENTS: usize = 512;
const QUERY: &str = "What is antifragility?";

/// Stream RAG chat events via shared ingested infra + a per-test API runtime.
async fn stream_rag_events(debug: bool) -> anyhow::Result<Vec<crate::product_e2e::SseEvent>> {
    let fixture = crate::product_e2e::fixtures::shared_rag_fixture().await;
    let upload = &fixture.upload;
    let ctx = shared_ready_rag_context().await;
    ctx.chat_stream_with_params(
        ChatStreamParams {
            query: QUERY,
            agent_type: "rag",
            notebook_id: &upload.notebook_id,
            doc_scope: &[upload.document_id.clone()],
            session_id: None,
            format_hint: None,
            debug,
            pin_mock_chunk_ids: true,
        },
        MAX_EVENTS,
        STREAM_DEADLINE,
    )
    .await
}

/// Mock LLM synthesis emits `reasoning_summary_delta` when the provider
/// returns `reasoning_content`. RAG synthesis uses non-stream `complete()`;
/// chat direct-answer skips synthesis, so we exercise the RAG path here.
#[tokio::test]
async fn chat_stream_collects_reasoning_delta_from_mock() {
    super::require_integration_suite();

    let events = stream_rag_events(false).await.expect("rag stream");

    let capture = collect_observability_from_events(&events);
    assert!(
        capture.delta_count > 0 || !capture.summary.is_empty(),
        "mock RAG synthesis should surface reasoning_summary_delta from non-stream reasoning_content"
    );
}

/// Loop telemetry trace events (`plan_decision` / `evaluation`) are emitted
/// without `debug: true`; only `prompt_snapshot` requires debug.
#[tokio::test]
async fn chat_stream_collects_trace_telemetry_without_debug() {
    super::require_integration_suite();

    let events = stream_rag_events(false).await.expect("rag stream");

    let capture = collect_observability_from_events(&events);
    assert!(
        !capture.trace_reasoning.is_empty(),
        "trace telemetry should not require debug, stages: {:?}",
        events
            .iter()
            .filter(|e| e.event == "trace")
            .map(|e| e.data.get("stage").and_then(|v| v.as_str()))
            .collect::<Vec<_>>()
    );
    assert!(
        capture.prompt_snapshots.is_empty(),
        "prompt_snapshot must stay gated behind debug: true"
    );
}

/// With `debug: true`, the stream must include loop telemetry trace events
/// (`plan_decision` / `evaluation`) for offline trace_reasoning capture.
#[tokio::test]
async fn chat_stream_debug_collects_trace_telemetry() {
    super::require_integration_suite();

    let events = stream_rag_events(true).await.expect("rag stream");

    let capture = collect_observability_from_events(&events);
    assert!(
        !capture.trace_reasoning.is_empty(),
        "debug stream should emit plan_decision/evaluation trace reasoning, stages: {:?}",
        events
            .iter()
            .filter(|e| e.event == "trace")
            .map(|e| e.data.get("stage").and_then(|v| v.as_str()))
            .collect::<Vec<_>>()
    );
}

/// With `debug: true`, the stream must include `prompt_snapshot` trace events
/// for offline prompt-compliance analysis.
#[tokio::test]
async fn chat_stream_debug_emits_prompt_snapshot() {
    super::require_integration_suite();

    let events = stream_rag_events(true).await.expect("rag stream");

    let capture = collect_observability_from_events(&events);
    assert!(
        !capture.prompt_snapshots.is_empty(),
        "debug stream should emit prompt_snapshot traces, got events: {:?}",
        events
            .iter()
            .filter(|e| e.event == "trace")
            .map(|e| e.data.get("stage").and_then(|v| v.as_str()))
            .collect::<Vec<_>>()
    );
    assert!(
        capture.prompt_snapshots[0]
            .get("system_content")
            .and_then(|v| v.as_str())
            .map(|s| !s.is_empty())
            .unwrap_or(false),
        "prompt_snapshot should include non-empty system_content"
    );
}

/// Client disconnect during SSE should not hang the test harness; we expect at
/// least one early event before the connection is dropped.
#[tokio::test]
async fn chat_stream_client_disconnect_aborts_without_hang() {
    super::require_integration_suite();

    let events = stream_rag_events(false).await.expect("baseline stream");
    assert!(
        events
            .iter()
            .any(|e| e.event == "start" || e.event == "trace"),
        "baseline stream should emit early events"
    );

    let fixture = crate::product_e2e::fixtures::shared_rag_fixture().await;
    let upload = &fixture.upload;
    let ctx = shared_ready_rag_context().await;
    let aborted = ctx
        .chat_stream_abort_after_start(QUERY, &upload.notebook_id, &[upload.document_id.clone()])
        .await
        .expect("abort stream");
    assert!(
        !aborted.is_empty(),
        "abort path should read at least one SSE event before drop"
    );
}

/// After an SSE disconnect, the client can resume the same chat session via a
/// new HTTP request carrying the `session_id` from the `start` event.
#[tokio::test]
async fn chat_stream_disconnect_reconnect_continues_session() {
    super::require_integration_suite();

    let fixture = crate::product_e2e::fixtures::shared_rag_fixture().await;
    let upload = &fixture.upload;
    let ctx = shared_ready_rag_context().await;

    let (events, session_id) = ctx
        .chat_stream_abort_capture_session(
            QUERY,
            &upload.notebook_id,
            &[upload.document_id.clone()],
        )
        .await
        .expect("abort capture session");
    assert!(
        events
            .iter()
            .any(|e| e.event == "start" || e.event == "trace"),
        "disconnect test should observe early SSE events"
    );
    let session_id = session_id.expect("start event should include session_id");

    let follow_up = "Can you elaborate on that in one sentence?";
    let http_resp = ctx
        .chat_with_session(
            follow_up,
            &upload.notebook_id,
            &[upload.document_id.clone()],
            &session_id,
        )
        .await
        .expect("reconnect chat with session_id");
    assert_eq!(
        http_resp.status, 200,
        "session reconnect must return HTTP 200, body={}",
        http_resp.body_json
    );
    let resp: crate::product_e2e::ChatResponse = http_resp.into_business().unwrap();
    assert_eq!(
        resp.session_id, session_id,
        "reconnected chat should stay on the same session"
    );
    assert!(
        !resp.answer.is_empty(),
        "reconnected chat should return a non-empty answer"
    );
}
