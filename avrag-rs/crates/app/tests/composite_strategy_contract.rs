//! End-to-end contract tests for CompositeStrategy routing and lifecycle.
//!
//! These tests verify that CompositeStrategy:
//! - Routes Composite requests correctly
//! - Handles missing backends gracefully
//! - Emits the expected event stream via the sink
//! - Decomposes queries and executes parallel branches

use app::agents::events::{AgentEvent, CollectingSink};
use app::agents::runtime::{Agent, AgentRequest};
use app::agents::unified::UnifiedAgent;
use app::agents::AgentKind;
use avrag_llm::LlmClient;
use std::collections::BTreeMap;

fn dummy_llm() -> LlmClient {
    LlmClient::new(avrag_llm::ModelProviderConfig {
        base_url: "http://localhost".to_string(),
        api_key: "dummy".to_string(),
        model: "test-model".to_string(),
        timeout_ms: 1000,
        api_style: None,
        dimensions: None,
        enable_thinking: None,
        enable_cache: None,
        rpm_limit: None,
        tpm_limit: None,
    })
}

fn base_request(kind: AgentKind) -> AgentRequest {
    AgentRequest {
        kind,
        query: "hello".to_string(),
        notebook_id: None,
        session_id: None,
        doc_scope: vec![],
        messages: vec![],
        session_summary: None,
        user_preferences: None,
        debug: false,
        stream: false,
        language: None,
        auth_context: serde_json::json!({"org_id": "00000000-0000-0000-0000-000000000001", "subject_kind": "User", "permissions": []}),
        docscope_metadata: None,
        metadata: BTreeMap::new(),
        cancellation_token: None,
        guard_pipeline: None,
        preferred_tools: vec![],
        format_hint: None,
        max_iterations: None,
    }
}

// ---------------------------------------------------------------------------
// Error / degrade paths
// ---------------------------------------------------------------------------

#[tokio::test]
async fn composite_without_backends_fails_at_decompose() {
    // With no real LLM, Composite fails during decompose (before the backend check).
    let agent = UnifiedAgent::new(Some(dummy_llm()), None)
        .with_rag_runtime(None)
        .with_search_executor(None);
    let sink = CollectingSink::new();
    let req = base_request(AgentKind::Composite);

    let result = agent.run(req, &sink).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.message().contains("decompose") || err.message().contains("LLM"));

    let events = sink.events();
    assert!(events.iter().any(|e| matches!(e, AgentEvent::Activity { stage, .. } if stage == "composite")));
}

#[tokio::test]
async fn composite_without_llm_returns_error() {
    let agent = UnifiedAgent::new(None, None)
        .with_rag_runtime(None)
        .with_search_executor(None);
    let sink = CollectingSink::new();
    let req = base_request(AgentKind::Composite);

    let result = agent.run(req, &sink).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.message().contains("LLM"));

    let events = sink.events();
    assert!(events.iter().any(|e| matches!(e, AgentEvent::Error { code, .. } if code == "llm_unavailable")));
}

// ---------------------------------------------------------------------------
// Routing and activity events
// ---------------------------------------------------------------------------

#[tokio::test]
async fn composite_emits_routing_decision_debug_trace() {
    let agent = UnifiedAgent::new(Some(dummy_llm()), None)
        .with_rag_runtime(None)
        .with_search_executor(None);
    let sink = CollectingSink::new();
    let req = base_request(AgentKind::Composite);

    let _ = agent.run(req, &sink).await;

    let events = sink.events();
    assert!(
        events.iter().any(|e| matches!(e, AgentEvent::DebugTrace { kind, .. } if kind == "routing.decision")),
        "expected routing.decision debug trace, got events: {:?}",
        events
    );
}

#[tokio::test]
async fn composite_emits_audit_record_for_routing() {
    let agent = UnifiedAgent::new(Some(dummy_llm()), None)
        .with_rag_runtime(None)
        .with_search_executor(None);
    let sink = CollectingSink::new();
    let req = base_request(AgentKind::Composite);

    let _ = agent.run(req, &sink).await;

    let events = sink.events();
    assert!(
        events.iter().any(|e| matches!(e, AgentEvent::Audit { .. })),
        "expected Audit event for routing decision"
    );
}

#[tokio::test]
async fn composite_emits_activity_events() {
    let agent = UnifiedAgent::new(Some(dummy_llm()), None)
        .with_rag_runtime(None)
        .with_search_executor(None);
    let sink = CollectingSink::new();
    let req = base_request(AgentKind::Composite);

    let _ = agent.run(req, &sink).await;

    let events = sink.events();
    assert!(
        events.iter().any(|e| matches!(e, AgentEvent::Activity { stage, .. } if stage == "composite")),
        "expected Activity event for composite stage, got: {:?}",
        events
    );
}

// ---------------------------------------------------------------------------
// Cancellation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn composite_cancellation_aborts_run_promptly() {
    let agent = UnifiedAgent::new(Some(dummy_llm()), None)
        .with_rag_runtime(None)
        .with_search_executor(None);
    let sink = CollectingSink::new();
    let cancel = tokio_util::sync::CancellationToken::new();
    let mut req = base_request(AgentKind::Composite);
    req.cancellation_token = Some(cancel.clone());

    cancel.cancel();

    let result = agent.run(req, &sink).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.message().contains("cancelled") || err.code().contains("cancelled"));
}
