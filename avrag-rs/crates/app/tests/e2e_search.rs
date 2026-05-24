//! E2E tests for Search strategy state machine + progressive disclosure.
//!
//! Run with: cargo test --ignored -p app --test e2e_search
//! Requires: E2E_LLM_* + E2E_BRAVE_API_KEY env vars.
//!
//! Mock mode: set E2E_BRAVE_API_KEY=mock to use MockSearchProvider (fast,
//! no external HTTP calls). Production mode: use a real Brave API key.

#[path = "e2e/config.rs"]
mod config;
#[path = "e2e/recording_llm.rs"]
mod recording_llm;
#[path = "e2e/assertions.rs"]
mod assertions;

use app::agents::events::CollectingSink;
use app::agents::react_loop::{LoopBudget, UserTier};
use app::agents::runtime::AgentRequest;
use app::agents::strategy::search::{
    LlmSearchAnswerSynthesizer, SearchAnswerSynthesizer, SearchContext, SearchStrategy,
};
use app::agents::strategy::Strategy;
use app::agents::AgentKind;
use common::ChatTurnInput;
use std::collections::BTreeMap;
use std::sync::Arc;

use config::E2EConfig;
use recording_llm::RecordingLlmProvider;

// ---------------------------------------------------------------------------
// MockSearchProvider — eliminates external API dependency for fast E2E tests
// ---------------------------------------------------------------------------

struct MockSearchProvider;

#[async_trait::async_trait]
impl avrag_search::SearchProvider for MockSearchProvider {
    async fn execute_search(
        &self,
        _query: &str,
        _vertical: Option<&str>,
    ) -> anyhow::Result<avrag_search::SearchResponse> {
        Ok(avrag_search::SearchResponse {
            query_type: "brave".to_string(),
            sub_queries: vec![_query.to_string()],
            results: vec![
                avrag_search::SearchResult {
                    title: "Rust Programming Language".to_string(),
                    url: "https://www.rust-lang.org".to_string(),
                    snippet: "Rust is a systems programming language that runs blazingly fast, prevents segfaults, and guarantees thread safety.".to_string(),
                    citation_index: Some(1),
                },
                avrag_search::SearchResult {
                    title: "The Rust Programming Language - Wikipedia".to_string(),
                    url: "https://en.wikipedia.org/wiki/Rust_(programming_language)".to_string(),
                    snippet: "Rust is a general-purpose programming language emphasizing performance, type safety, and concurrency.".to_string(),
                    citation_index: Some(2),
                },
            ],
            synthesized_answer: String::new(),
            llm_usage: None,
        })
    }
}

fn test_auth_context() -> serde_json::Value {
    serde_json::json!({
        "org_id": "00000000-0000-0000-0000-000000000001",
        "subject_kind": "User",
        "permissions": ["external_network"]
    })
}

fn search_request(query: &str) -> AgentRequest {
    AgentRequest {
        kind: AgentKind::Search,
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

fn build_search_executor(api_key: &str) -> Arc<dyn avrag_search::SearchProvider> {
    if api_key == "mock" {
        return Arc::new(MockSearchProvider);
    }
    Arc::new(avrag_search::SearchExecutor::new(
        avrag_search::SearchConfig {
            provider: "brave_llm_context".to_string(),
            base_url: "https://api.search.brave.com".to_string(),
            api_key: api_key.to_string(),
            max_results: 10,
            search_lang: None,
            country: None,
            freshness: None,
        },
    ))
}

/// Test: Search single-pass — Decompose → ParallelSearch → Aggregate → Evaluate → Answer
#[tokio::test]
#[ignore = "requires staging: E2E_LLM_* + E2E_BRAVE_API_KEY"]
async fn search_single_pass_state_machine() {
    let config = E2EConfig::from_env().expect("E2E config not set");
    let llm_client = config.llm_client();
    let brave_api_key = config.brave_api_key.as_deref().unwrap_or("mock");

    let recording = RecordingLlmProvider::new(Arc::new(llm_client.clone()));
    let recording_arc = Arc::new(recording);

    let search_executor = build_search_executor(brave_api_key);
    let llm: Arc<dyn avrag_llm::LlmProvider> = recording_arc.clone();
    let search_synthesizer: Option<Arc<dyn SearchAnswerSynthesizer>> = Some(Arc::new(
        LlmSearchAnswerSynthesizer {
            llm,
            llm_client: Some(llm_client.clone()),
        },
    ));

    let ctx = SearchContext::from_request(
        search_request("What is the latest Rust release?"),
        "test-search-single-pass".to_string(),
        LoopBudget::search(UserTier::Pro),
        Box::new(CollectingSink::new()),
        tokio_util::sync::CancellationToken::new(),
    )
    .unwrap();

    let strategy = SearchStrategy {
        llm: recording_arc.clone(),
        llm_client: Some(llm_client),
        temperature: None,
        search_executor,
        search_synthesizer,
    };

    let executor = app::agents::strategy::executor::StrategyExecutor;
    let result = executor.run(&strategy, ctx).await.unwrap();

    // --- State machine assertions ---
    let schema = SearchStrategy::schema();
    let history = result.state_history.as_ref().expect("state_history missing");
    assertions::assert_valid_transitions(&schema, history);
    assertions::assert_state_kinds(history);

    // Expected: Decompose → ParallelSearch → Aggregate → Evaluate → Answer (5 states)
    assert_eq!(
        history.len(),
        5,
        "Expected 5 states (Decompose→ParallelSearch→Aggregate→Evaluate→Answer), got {}: {:?}",
        history.len(),
        history.iter().map(|s| &s.state_id).collect::<Vec<_>>()
    );

    // --- Progressive disclosure ---
    let calls = recording_arc.calls();
    assert!(
        calls.len() >= 3,
        "Expected at least 3 LLM calls (decompose + eval + answer), got {}",
        calls.len()
    );

    // Decompose (Plan) call: search-plan skill + web_search tool catalog
    assertions::assert_prompt_contains_skill(&calls[0].system_prompt, "search-plan");
    assertions::assert_prompt_has_tool_catalog(&calls[0].system_prompt, "search");

    // Evaluate call: search-eval skill
    let eval_call = calls
        .iter()
        .find(|c| {
            c.user_messages
                .iter()
                .any(|m| m.content.contains("evaluate") || m.content.contains("Evaluate"))
        })
        .or_else(|| calls.get(1))
        .expect("evaluate call not found");
    assertions::assert_prompt_contains_skill(&eval_call.system_prompt, "search-eval");

    // Answer call: search-answer skill + format skills
    let answer_call = calls.last().expect("no answer call");
    assertions::assert_prompt_contains_skill(&answer_call.system_prompt, "search-answer");
    assertions::assert_prompt_has_format_skills(&answer_call.system_prompt);

    // Budget: 1 iteration
    if let Some(budget) = &result.budget_used {
        assertions::assert_budget_usage(budget.current, 1);
    }
}

/// Test: Search vertical escalation — evaluation finds results insufficient,
/// triggers another search with a different vertical.
#[tokio::test]
#[ignore = "requires staging: E2E_LLM_* + E2E_BRAVE_API_KEY"]
async fn search_vertical_escalation_state_machine() {
    let config = E2EConfig::from_env().expect("E2E config not set");
    let llm_client = config.llm_client();
    let brave_api_key = config.brave_api_key.as_deref().unwrap_or("mock");

    let recording = RecordingLlmProvider::new(Arc::new(llm_client.clone()));
    let recording_arc = Arc::new(recording);

    let search_executor = build_search_executor(brave_api_key);
    let llm: Arc<dyn avrag_llm::LlmProvider> = recording_arc.clone();
    let search_synthesizer: Option<Arc<dyn SearchAnswerSynthesizer>> = Some(Arc::new(
        LlmSearchAnswerSynthesizer {
            llm,
            llm_client: Some(llm_client.clone()),
        },
    ));

    // Time-sensitive query more likely to trigger vertical escalation
    let ctx = SearchContext::from_request(
        search_request("latest AI news from today"),
        "test-search-escalation".to_string(),
        LoopBudget::search(UserTier::Pro),
        Box::new(CollectingSink::new()),
        tokio_util::sync::CancellationToken::new(),
    )
    .unwrap();

    let strategy = SearchStrategy {
        llm: recording_arc.clone(),
        llm_client: Some(llm_client),
        temperature: None,
        search_executor,
        search_synthesizer,
    };

    let executor = app::agents::strategy::executor::StrategyExecutor;
    let result = executor.run(&strategy, ctx).await.unwrap();

    // --- State machine assertions ---
    let schema = SearchStrategy::schema();
    let history = result.state_history.as_ref().expect("state_history missing");
    assertions::assert_valid_transitions(&schema, history);
    assertions::assert_state_kinds(history);

    // May or may not escalate — if it does, verify valid transitions
    // Budget: within max (3)
    if let Some(budget) = &result.budget_used {
        assertions::assert_budget_usage(budget.current, 3);
    }

    // Check if replan/escalation occurred
    let has_replan = history.windows(2).any(|w| {
        w[0].state_id == "evaluate" && w[1].state_id == "parallel_search"
    });

    if has_replan {
        // With replan: Decompose → ParallelSearch → Aggregate → Evaluate → ParallelSearch → ...
        // Should have at least 5 states (original 5 + replan loop adds more)
        assert!(
            history.len() >= 5,
            "Expected >= 5 states with replan, got {}: {:?}",
            history.len(),
            history.iter().map(|s| &s.state_id).collect::<Vec<_>>()
        );
    }
}
