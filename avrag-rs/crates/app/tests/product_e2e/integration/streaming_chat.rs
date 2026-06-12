//! SSE streaming chat coverage — observability and mock-agent behaviour.
//!
//! Protocol invariants (`start` first, `done` terminal, payload shape) live in
//! `transport-http` contract tests. This module exercises the full mock RAG
//! pipeline and loop telemetry with a module-scoped [`shared_ready_rag`] fixture.

use std::time::Duration;

use crate::product_e2e::{
    ChatStreamParams, fixtures::shared_ready_rag, llm_real::collect_observability_from_events,
};

const STREAM_DEADLINE: Duration = Duration::from_secs(60);
const MAX_EVENTS: usize = 512;
const QUERY: &str = "What is antifragility?";

async fn stream_rag_events(
    params: ChatStreamParams<'_>,
) -> Vec<crate::product_e2e::SseEvent> {
    let shared = shared_ready_rag().await;
    let ctx = shared.0.lock().expect("shared ready_rag lock");
    ctx.chat_stream_with_params(params, MAX_EVENTS, STREAM_DEADLINE)
        .await
        .unwrap()
}

/// Mock LLM synthesis emits `reasoning_summary_delta` when the provider
/// returns `reasoning_content`. RAG synthesis uses non-stream `complete()`;
/// chat direct-answer skips synthesis, so we exercise the RAG path here.
#[tokio::test]
async fn chat_stream_collects_reasoning_delta_from_mock() {
    let shared = shared_ready_rag().await;
    let upload = &shared.1;

    let events = stream_rag_events(ChatStreamParams {
        query: QUERY,
        agent_type: "rag",
        notebook_id: &upload.notebook_id,
        doc_scope: &[upload.document_id.clone()],
        session_id: None,
        format_hint: None,
        debug: false,
    })
    .await;

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
    let shared = shared_ready_rag().await;
    let upload = &shared.1;

    let events = stream_rag_events(ChatStreamParams {
        query: QUERY,
        agent_type: "rag",
        notebook_id: &upload.notebook_id,
        doc_scope: &[upload.document_id.clone()],
        session_id: None,
        format_hint: None,
        debug: false,
    })
    .await;

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
    let shared = shared_ready_rag().await;
    let upload = &shared.1;

    let events = stream_rag_events(ChatStreamParams {
        query: QUERY,
        agent_type: "rag",
        notebook_id: &upload.notebook_id,
        doc_scope: &[upload.document_id.clone()],
        session_id: None,
        format_hint: None,
        debug: true,
    })
    .await;

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
    let shared = shared_ready_rag().await;
    let upload = &shared.1;

    let events = stream_rag_events(ChatStreamParams {
        query: QUERY,
        agent_type: "rag",
        notebook_id: &upload.notebook_id,
        doc_scope: &[upload.document_id.clone()],
        session_id: None,
        format_hint: None,
        debug: true,
    })
    .await;

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
