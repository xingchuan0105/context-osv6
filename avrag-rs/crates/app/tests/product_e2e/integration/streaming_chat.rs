//! SSE streaming chat coverage.
//!
//! These tests verify the streaming `chat` path (the production
//! `ChatEvent` JSON-over-SSE protocol) end-to-end. Non-streaming tests
//! only cover the `Done` payload, leaving 8 other event variants
//! (`start`, `activity`, `answer_start`, `trace`, `token`,
//! `reasoning_summary_delta`, `citations`, `error`) untested.
//!
//! The streaming RAG path now correctly emits `start → ... → done`.
//! The mock LLM server supports SSE responses for `"stream": true`
//! requests, so `LlmClient::complete_stream` works end-to-end.

use std::time::Duration;

use crate::product_e2e::{
    ChatStreamParams, DocumentStatus, SseEvent, TestContext,
    llm_real::collect_observability_from_events,
};

const STREAM_DEADLINE: Duration = Duration::from_secs(60);
/// The mock LLM emits one `token` event per character of the canned
/// answer. The longest canned answer is the RAG answer (≈260 chars),
/// so 512 is comfortably above the expected event count for any
/// single chat run.
const MAX_EVENTS: usize = 512;

/// A streaming chat run must always begin with a `start` event,
/// regardless of whether it later completes successfully or errors.
#[tokio::test]
async fn chat_stream_emits_start_event_first() {
    let mut ctx = TestContext::new_smoke_with_rag().await;
    let upload = ctx.upload_document("antifragile.txt").await.unwrap();
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    let events = ctx
        .chat_stream(
            "What is antifragility?",
            &upload.notebook_id,
            &[upload.document_id.clone()],
            MAX_EVENTS,
            STREAM_DEADLINE,
        )
        .await
        .unwrap();

    assert!(
        !events.is_empty(),
        "stream should emit at least one event, got nothing"
    );
    let first_event = &events[0];
    assert_eq!(
        first_event.event, "start",
        "first event must be 'start', got '{}' with data: {}",
        first_event.event, first_event.data
    );
}

/// A streaming chat run must terminate with a `done` event.
#[tokio::test]
async fn chat_stream_terminates_with_done_or_error() {
    let mut ctx = TestContext::new_smoke_with_rag().await;
    let upload = ctx.upload_document("antifragile.txt").await.unwrap();
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    let events = ctx
        .chat_stream(
            "What is antifragility?",
            &upload.notebook_id,
            &[upload.document_id.clone()],
            MAX_EVENTS,
            STREAM_DEADLINE,
        )
        .await
        .unwrap();

    let names: Vec<&str> = events.iter().map(|e| e.event.as_str()).collect();
    let last = names.last().copied();
    assert!(
        last == Some("done"),
        "stream must terminate with 'done', got last event: {last:?}, full: {names:?}"
    );
}

/// The `done` event payload must contain the full `ChatResponse` shape
/// that a non-streaming chat would return.
#[tokio::test]
async fn chat_stream_done_payload_shape_when_present() {
    let mut ctx = TestContext::new_smoke_with_rag().await;
    let upload = ctx.upload_document("antifragile.txt").await.unwrap();
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    let events = ctx
        .chat_stream(
            "What is antifragility?",
            &upload.notebook_id,
            &[upload.document_id.clone()],
            MAX_EVENTS,
            STREAM_DEADLINE,
        )
        .await
        .unwrap();

    let done = events.iter().find(|e| e.event == "done");
    assert!(
        done.is_some(),
        "stream must contain a 'done' event, got events: {:?}",
        events.iter().map(|e| e.event.as_str()).collect::<Vec<_>>()
    );
    let done = done.unwrap();

    // The production SseSink wraps AgentEvent::Done in a ChatEvent::Done
    // whose `payload` is a flat object (not a wrapped `response` object):
    //
    //   { "session_id", "message_id", "agent_type", "answer",
    //     "final_message", "usage" }
    //
    // The full ChatResponse (citations, degrade_trace, sources) is
    // delivered separately by the HTTP layer's final `done` payload
    // rather than embedded in this SSE event. See handlers.rs
    // `sse_response_from_receiver` + `chat_done_payload`.
    let payload = done
        .data
        .get("payload")
        .expect("done.data.payload must exist");
    assert!(
        payload.get("answer").and_then(|v| v.as_str()).is_some(),
        "done.payload.answer must be a string, got: {payload}"
    );
    assert!(
        payload.get("agent_type").and_then(|v| v.as_str()).is_some(),
        "done.payload.agent_type must be a string, got: {payload}"
    );
    assert!(
        payload.get("session_id").and_then(|v| v.as_str()).is_some(),
        "done.payload.session_id must be a string, got: {payload}"
    );
}

/// Mock LLM synthesis emits `reasoning_summary_delta` when the provider
/// returns `reasoning_content`. RAG synthesis uses non-stream `complete()`;
/// chat direct-answer skips synthesis, so we exercise the RAG path here.
#[tokio::test]
async fn chat_stream_collects_reasoning_delta_from_mock() {
    let mut ctx = TestContext::new_smoke_with_rag().await;
    let upload = ctx.upload_document("antifragile.txt").await.unwrap();
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    let events = ctx
        .chat_stream_with_params(
            ChatStreamParams {
                query: "What is antifragility?",
                agent_type: "rag",
                notebook_id: &upload.notebook_id,
                doc_scope: &[upload.document_id.clone()],
                session_id: None,
                format_hint: None,
                debug: false,
            },
            MAX_EVENTS,
            STREAM_DEADLINE,
        )
        .await
        .unwrap();

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
    let mut ctx = TestContext::new_smoke_with_rag().await;
    let upload = ctx.upload_document("antifragile.txt").await.unwrap();
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    let events = ctx
        .chat_stream_with_params(
            ChatStreamParams {
                query: "What is antifragility?",
                agent_type: "rag",
                notebook_id: &upload.notebook_id,
                doc_scope: &[upload.document_id.clone()],
                session_id: None,
                format_hint: None,
                debug: false,
            },
            MAX_EVENTS,
            STREAM_DEADLINE,
        )
        .await
        .unwrap();

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
    let mut ctx = TestContext::new_smoke_with_rag().await;
    let upload = ctx.upload_document("antifragile.txt").await.unwrap();
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    let events = ctx
        .chat_stream_with_params(
            ChatStreamParams {
                query: "What is antifragility?",
                agent_type: "rag",
                notebook_id: &upload.notebook_id,
                doc_scope: &[upload.document_id.clone()],
                session_id: None,
                format_hint: None,
                debug: true,
            },
            MAX_EVENTS,
            STREAM_DEADLINE,
        )
        .await
        .unwrap();

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
    let mut ctx = TestContext::new_smoke_with_rag().await;
    let upload = ctx.upload_document("antifragile.txt").await.unwrap();
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    let events = ctx
        .chat_stream_with_params(
            ChatStreamParams {
                query: "What is antifragility?",
                agent_type: "rag",
                notebook_id: &upload.notebook_id,
                doc_scope: &[upload.document_id.clone()],
                session_id: None,
                format_hint: None,
                debug: true,
            },
            MAX_EVENTS,
            STREAM_DEADLINE,
        )
        .await
        .unwrap();

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

/// If a streaming run produces an `error` event, the event must include
/// structured `code` and `message` fields. This path is no longer the
/// expected terminal state (streams should end with `done`), but if an
/// error event does appear we validate its shape.
#[tokio::test]
async fn chat_stream_error_event_has_code_and_message() {
    let mut ctx = TestContext::new_smoke_with_rag().await;
    let upload = ctx.upload_document("antifragile.txt").await.unwrap();
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    let events = ctx
        .chat_stream(
            "What is antifragility?",
            &upload.notebook_id,
            &[upload.document_id.clone()],
            MAX_EVENTS,
            STREAM_DEADLINE,
        )
        .await
        .unwrap();

    let error = events.iter().find(|e| e.event == "error");
    let Some(error) = error else {
        // Stream succeeded with `done` — error-event shape not exercised here.
        return;
    };

    let code = error
        .data
        .get("code")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let message = error
        .data
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    assert!(
        !code.is_empty(),
        "error event must have non-empty 'code', got: {}",
        error.data
    );
    assert!(
        !message.is_empty(),
        "error event must have non-empty 'message', got: {}",
        error.data
    );
}

#[allow(dead_code)]
fn event_summary(events: &[SseEvent]) -> Vec<String> {
    events
        .iter()
        .map(|e| {
            let data_preview = e.data.to_string();
            let preview = if data_preview.len() > 80 {
                format!("{}…", &data_preview[..80])
            } else {
                data_preview
            };
            format!("{}({})", e.event, preview)
        })
        .collect()
}
