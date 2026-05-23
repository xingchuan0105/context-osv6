//! E2E tests for Chat strategy state machine + progressive disclosure.
//!
//! Run with: cargo test --ignored -p app --test e2e_chat

#[path = "e2e/config.rs"]
mod config;
#[path = "e2e/recording_llm.rs"]
mod recording_llm;
#[path = "e2e/assertions.rs"]
mod assertions;

use app::agents::events::CollectingSink;
use app::agents::react_loop::{LoopBudget, UserTier};
use app::agents::runtime::AgentRequest;
use app::agents::strategy::chat::ChatContext;
use app::agents::strategy::Strategy;
use app::agents::AgentKind;
use common::ChatTurnInput;
use std::collections::BTreeMap;
use std::sync::Arc;

use config::E2EConfig;
use recording_llm::RecordingLlmProvider;

fn test_auth_context() -> serde_json::Value {
    serde_json::json!({
        "org_id": "00000000-0000-0000-0000-000000000001",
        "subject_kind": "User",
        "permissions": []
    })
}

fn chat_request(query: &str) -> AgentRequest {
    AgentRequest {
        kind: AgentKind::Chat,
        query: query.to_string(),
        notebook_id: None,
        session_id: None,
        doc_scope: vec![],
        messages: vec![ChatTurnInput {
            role: "user".to_string(),
            content: query.to_string(),
        }],
        session_summary: None,
        user_preferences: None,
        debug: false,
        stream: false,
        language: None,
        preferred_tools: vec![],
        format_hint: None,
        max_iterations: None,
        auth_context: test_auth_context(),
        docscope_metadata: None,
        metadata: BTreeMap::new(),
        cancellation_token: None,
        guard_pipeline: None,
    }
}

/// Test: Chat simple conversation traverses correct state machine
/// and injects correct skill bodies + tool catalogs into LLM prompts.
#[tokio::test]
#[ignore = "requires staging environment (E2E_LLM_BASE_URL, E2E_LLM_API_KEY, E2E_LLM_MODEL)"]
async fn chat_simple_conversation_state_machine() {
    let config = E2EConfig::from_env().expect("E2E config not set — set E2E_LLM_* env vars");
    let llm_client = config.llm_client();

    // Wrap real LLM in recording provider
    let recording = RecordingLlmProvider::new(Arc::new(llm_client.clone()));
    let recording_arc = Arc::new(recording);

    // Build context
    let ctx = ChatContext::from_request(
        chat_request("What is the capital of France?"),
        "test-chat-simple".to_string(),
        LoopBudget::chat(UserTier::Pro),
        Box::new(CollectingSink::new()),
        tokio_util::sync::CancellationToken::new(),
    )
    .unwrap();

    // Build strategy with recording provider
    let strategy = app::agents::strategy::chat::ChatStrategy {
        llm: recording_arc.clone(),
        llm_client: Some(llm_client),
        temperature: None,
    };

    // Run
    let executor = app::agents::strategy::executor::StrategyExecutor;
    let result = executor.run(&strategy, ctx).await.unwrap();

    // --- State machine assertions ---
    let schema = app::agents::strategy::chat::ChatStrategy::schema();
    let history = result.state_history.as_ref().expect("state_history missing");
    assertions::assert_valid_transitions(&schema, history);
    assertions::assert_state_kinds(history);

    // Expected: Plan → Answer or Plan → ExecuteAtomic → Answer
    assert!(
        history.len() >= 2,
        "Expected at least 2 states, got {}",
        history.len()
    );

    // --- Progressive disclosure assertions ---
    let calls = recording_arc.calls();
    assert!(
        calls.len() >= 2,
        "Expected at least 2 LLM calls (plan + answer), got {}",
        calls.len()
    );

    // First call = Plan: should contain chat-plan skill body + tool catalog
    let plan_call = &calls[0];
    assertions::assert_prompt_contains_skill(&plan_call.system_prompt, "chat-plan");
    assertions::assert_prompt_has_tool_catalog(&plan_call.system_prompt, "chat");

    // Last call = Answer: should contain chat skill body + format skills
    let answer_call = calls.last().unwrap();
    assertions::assert_prompt_contains_skill(&answer_call.system_prompt, "chat");
    assertions::assert_prompt_has_format_skills(&answer_call.system_prompt);

    // Budget: chat max_budget is 1
    if let Some(budget) = &result.budget_used {
        assertions::assert_budget_usage(budget.current, 1);
    }
}

/// Test: Chat with tool call — Plan selects a tool, ExecuteAtomic runs it,
/// Answer incorporates the tool result.
#[tokio::test]
#[ignore = "requires staging environment (E2E_LLM_BASE_URL, E2E_LLM_API_KEY, E2E_LLM_MODEL)"]
async fn chat_with_tool_call_state_machine() {
    let config = E2EConfig::from_env().expect("E2E config not set");
    let llm_client = config.llm_client();

    let recording = RecordingLlmProvider::new(Arc::new(llm_client.clone()));
    let recording_arc = Arc::new(recording);

    // Use preferred_tools to nudge Plan toward calculator
    let mut request = chat_request("What is 2 + 2?");
    request.preferred_tools = vec!["calculator".to_string()];

    let ctx = ChatContext::from_request(
        request,
        "test-chat-tool".to_string(),
        LoopBudget::chat(UserTier::Pro),
        Box::new(CollectingSink::new()),
        tokio_util::sync::CancellationToken::new(),
    )
    .unwrap();

    let strategy = app::agents::strategy::chat::ChatStrategy {
        llm: recording_arc.clone(),
        llm_client: Some(llm_client),
        temperature: None,
    };

    let executor = app::agents::strategy::executor::StrategyExecutor;
    let result = executor.run(&strategy, ctx).await.unwrap();

    // State machine: Plan → ExecuteAtomic → Answer
    let schema = app::agents::strategy::chat::ChatStrategy::schema();
    let history = result.state_history.as_ref().expect("state_history missing");
    assertions::assert_valid_transitions(&schema, history);
    assertions::assert_state_kinds(history);

    // Should have at least 3 states (Plan → ExecuteAtomic → Answer)
    assert!(
        history.len() >= 3,
        "Expected at least 3 states with tool call, got {}: {:?}",
        history.len(),
        history.iter().map(|s| &s.state_id).collect::<Vec<_>>()
    );

    // Verify execute_atomic state was visited
    assert!(
        history.iter().any(|s| s.state_id == "execute_atomic"),
        "Expected execute_atomic state in history"
    );

    // LLM calls: plan + answer (ExecuteAtomic doesn't call LLM, it runs tools)
    let calls = recording_arc.calls();
    assert!(
        calls.len() >= 2,
        "Expected at least 2 LLM calls, got {}",
        calls.len()
    );

    // Plan prompt should contain chat-plan skill
    assertions::assert_prompt_contains_skill(&calls[0].system_prompt, "chat-plan");

    // Answer prompt should contain chat skill + format skills
    let answer_call = calls.last().unwrap();
    assertions::assert_prompt_contains_skill(&answer_call.system_prompt, "chat");
    assertions::assert_prompt_has_format_skills(&answer_call.system_prompt);
}
