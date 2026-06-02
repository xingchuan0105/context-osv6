//! End-to-end contract tests for UnifiedAgent routing and lifecycle.
//!
//! These tests verify that UnifiedAgent:
//! - Routes requests to the correct strategy based on `AgentKind`
//! - Validates prerequisites (doc_scope for RAG, runtime configs)
//! - Emits the expected event stream via the sink
//! - Handles cancellation and error paths correctly

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
// Error paths
// ---------------------------------------------------------------------------

#[tokio::test]
async fn chat_without_llm_returns_error() {
    let agent = UnifiedAgent::new(None, None);
    let sink = CollectingSink::new();
    let req = base_request(AgentKind::Chat);

    let result = agent.run(req, &sink).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.message().contains("LLM"));

    let events = sink.events();
    assert!(events.iter().any(|e| matches!(e, AgentEvent::Error { code, .. } if code == "llm_unavailable")));
}

#[tokio::test]
async fn rag_without_doc_scope_returns_validation_error() {
    let agent = UnifiedAgent::new(Some(dummy_llm()), None);
    let sink = CollectingSink::new();
    let req = base_request(AgentKind::Rag);

    let result = agent.run(req, &sink).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.code().contains("missing_doc_scope") || err.message().contains("doc_scope"));

    let events = sink.events();
    assert!(events.iter().any(|e| matches!(e, AgentEvent::Error { code, .. } if code == "missing_doc_scope")));
}

#[tokio::test]
async fn rag_without_runtime_returns_error() {
    let agent = UnifiedAgent::new(Some(dummy_llm()), None);
    let sink = CollectingSink::new();
    let mut req = base_request(AgentKind::Rag);
    req.doc_scope = vec!["doc-1".to_string()];

    let result = agent.run(req, &sink).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.message().contains("RAG runtime"));

    let events = sink.events();
    assert!(events.iter().any(|e| matches!(e, AgentEvent::Error { code, .. } if code == "rag_unavailable")));
}

#[tokio::test]
async fn search_without_executor_returns_error() {
    let agent = UnifiedAgent::new(Some(dummy_llm()), None);
    let sink = CollectingSink::new();
    let req = base_request(AgentKind::Search);

    let result = agent.run(req, &sink).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.message().contains("Search executor"));

    let events = sink.events();
    assert!(events.iter().any(|e| matches!(e, AgentEvent::Error { code, .. } if code == "search_unavailable")));
}

// ---------------------------------------------------------------------------
// Routing decision events
// ---------------------------------------------------------------------------

#[tokio::test]
async fn chat_emits_routing_decision_event() {
    let agent = UnifiedAgent::new(Some(dummy_llm()), None);
    let sink = CollectingSink::new();
    let req = base_request(AgentKind::Chat);

    // Will fail at LLM call, but routing decision should be emitted first.
    let _ = agent.run(req, &sink).await;

    let events = sink.events();
    assert!(
        events.iter().any(|e| matches!(e, AgentEvent::RoutingDecision { strategy_id, .. } if strategy_id == "chat")),
        "expected RoutingDecision event for chat strategy, got events: {:?}",
        events
    );
}

#[tokio::test]
async fn chat_emits_audit_record_for_routing() {
    let agent = UnifiedAgent::new(Some(dummy_llm()), None);
    let sink = CollectingSink::new();
    let req = base_request(AgentKind::Chat);

    let _ = agent.run(req, &sink).await;

    let events = sink.events();
    assert!(
        events.iter().any(|e| matches!(e, AgentEvent::Audit { .. })),
        "expected Audit event for routing decision"
    );
}

#[tokio::test]
async fn chat_emits_activity_event() {
    let agent = UnifiedAgent::new(Some(dummy_llm()), None);
    let sink = CollectingSink::new();
    let req = base_request(AgentKind::Chat);

    let _ = agent.run(req, &sink).await;

    let events = sink.events();
    assert!(
        events.iter().any(|e| matches!(e, AgentEvent::Activity { stage, .. } if stage == "chat")),
        "expected Activity event for chat stage"
    );
}

// ---------------------------------------------------------------------------
// Cancellation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cancellation_aborts_run_promptly() {
    let agent = UnifiedAgent::new(Some(dummy_llm()), None);
    let sink = CollectingSink::new();
    let cancel = tokio_util::sync::CancellationToken::new();
    let mut req = base_request(AgentKind::Chat);
    req.cancellation_token = Some(cancel.clone());

    // Cancel immediately before the run starts.
    cancel.cancel();

    let result = agent.run(req, &sink).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    println!("cancellation error: code={} message={}", err.code(), err.message());
    assert!(err.message().contains("cancelled") || err.code().contains("cancelled"), "expected cancellation error, got code={} message={}", err.code(), err.message());
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

#[test]
fn unified_agent_builder_chain_compiles() {
    let llm = dummy_llm();
    let _agent = UnifiedAgent::new(Some(llm.clone()), Some(0.5))
        .with_rag_runtime(None)
        .with_search_executor(None);
    // Builder chain compiles; field access is not tested because fields are private.
}
