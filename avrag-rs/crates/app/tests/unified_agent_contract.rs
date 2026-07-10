//! End-to-end contract tests for UnifiedAgent routing and lifecycle.
//!
//! These tests verify that UnifiedAgent:
//! - Routes requests to the correct strategy based on `AgentKind`
//! - Validates prerequisites (doc_scope for RAG, runtime configs)
//! - Emits the expected event stream via the sink
//! - Handles cancellation and error paths correctly

use agent_loop::events::{AgentEvent, CollectingSink};
use agent_loop::runtime::{Agent, AgentRequest};
use app::agents::AgentKind;
use app::agents::unified::UnifiedAgent;
use avrag_llm::LlmClient;
use contracts::auth_runtime::{AuthContext, OrgId, SubjectKind};
use std::collections::BTreeMap;
use uuid::Uuid;

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
        workspace_id: None,
        session_id: None,
        doc_scope: vec![],
        messages: vec![],
        user_preferences: None,
        debug: false,
        stream: false,
        language: None,
        auth: AuthContext::new(
            OrgId::from(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()),
            SubjectKind::User,
        ),
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
    let agent = UnifiedAgent::new(None, None, None);
    let sink = CollectingSink::new();
    let req = base_request(AgentKind::Chat);

    let result = agent.run(req, &sink).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.message().contains("LLM"));

    let events = sink.events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::Error { code, .. } if code == "llm_unavailable"))
    );
}

#[tokio::test]
async fn rag_without_doc_scope_returns_validation_error() {
    let agent = UnifiedAgent::new(Some(dummy_llm()), None, None);
    let sink = CollectingSink::new();
    let req = base_request(AgentKind::Rag);

    let result = agent.run(req, &sink).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.code().contains("missing_doc_scope") || err.message().contains("doc_scope"));

    let events = sink.events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::Error { code, .. } if code == "missing_doc_scope"))
    );
}

#[tokio::test]
async fn rag_without_runtime_returns_error() {
    let agent = UnifiedAgent::new(Some(dummy_llm()), None, None);
    let sink = CollectingSink::new();
    let mut req = base_request(AgentKind::Rag);
    req.doc_scope = vec!["doc-1".to_string()];

    let result = agent.run(req, &sink).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.message().contains("RAG runtime"));

    let events = sink.events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::Error { code, .. } if code == "rag_unavailable"))
    );
}

#[tokio::test]
async fn search_without_executor_returns_error() {
    let agent = UnifiedAgent::new(Some(dummy_llm()), None, None);
    let sink = CollectingSink::new();
    let req = base_request(AgentKind::Search);

    let result = agent.run(req, &sink).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.message().contains("Search executor"));

    let events = sink.events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::Error { code, .. } if code == "search_unavailable"))
    );
}

#[tokio::test]
async fn write_through_unified_agent_returns_not_implemented() {
    let agent = UnifiedAgent::new(Some(dummy_llm()), None, None);
    let sink = CollectingSink::new();
    let req = base_request(AgentKind::Write);

    let result = agent.run(req, &sink).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.code().contains("write_routed_outside_unified_agent")
            || err.code().contains("write_mode_not_implemented"),
        "expected write_routed_outside_unified_agent, got code={} message={}",
        err.code(),
        err.message()
    );
}

// ---------------------------------------------------------------------------
// Routing decision events
// ---------------------------------------------------------------------------

#[tokio::test]
async fn chat_emits_routing_decision_event() {
    let agent = UnifiedAgent::new(Some(dummy_llm()), None, None);
    let sink = CollectingSink::new();
    let req = base_request(AgentKind::Chat);

    // Will fail at LLM call, but routing decision should be emitted first.
    let _ = agent.run(req, &sink).await;

    let events = sink.events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::RoutingDecision { mode_id, .. } if mode_id == "chat")),
        "expected RoutingDecision event for chat mode, got events: {:?}",
        events
    );
}

#[tokio::test]
async fn chat_emits_audit_record_for_routing() {
    let agent = UnifiedAgent::new(Some(dummy_llm()), None, None);
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
    let agent = UnifiedAgent::new(Some(dummy_llm()), None, None);
    let sink = CollectingSink::new();
    let req = base_request(AgentKind::Chat);

    let _ = agent.run(req, &sink).await;

    let events = sink.events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::Activity { stage, .. } if stage == "chat")),
        "expected Activity event for chat stage"
    );
}

// ---------------------------------------------------------------------------
// Cancellation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cancellation_aborts_run_promptly() {
    let agent = UnifiedAgent::new(Some(dummy_llm()), None, None);
    let sink = CollectingSink::new();
    let cancel = tokio_util::sync::CancellationToken::new();
    let mut req = base_request(AgentKind::Chat);
    req.cancellation_token = Some(cancel.clone());

    // Cancel immediately before the run starts.
    cancel.cancel();

    let result = agent.run(req, &sink).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    println!(
        "cancellation error: code={} message={}",
        err.code(),
        err.message()
    );
    assert!(
        err.message().contains("cancelled") || err.code().contains("cancelled"),
        "expected cancellation error, got code={} message={}",
        err.code(),
        err.message()
    );
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

#[test]
fn unified_agent_builder_chain_compiles() {
    let llm = dummy_llm();
    let _agent = UnifiedAgent::new(Some(llm.clone()), None, None)
        .with_rag_runtime(None)
        .with_search_executor(None);
    // Builder chain compiles; field access is not tested because fields are private.
}
