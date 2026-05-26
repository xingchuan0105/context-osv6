//! E2E tests for Search strategy state machine + progressive disclosure.
//!
//! Run with: cargo test --ignored -p app --test e2e_search
//! Requires: E2E_LLM_* + E2E_BRAVE_API_KEY env vars.
//!
//! Mock mode: set E2E_BRAVE_API_KEY=mock to use MockSearchProvider (fast,
//! no external HTTP calls). Production mode: use a real Brave API key.
//!
//! ## Brave Search API rate limits (2026-05)
//!
//! | Tier | QPS | Monthly volume |
//! |------|-----|----------------|
//! | Free (credit) | 1 | ~1,000 queries |
//! | Base (paid) | 20 | up to 20M |
//! | Pro (paid) | 50 | unlimited |
//!
//! The Search strategy dispatches all sub-queries in parallel via `join_all`.
//! If your key is on the Free tier, running multiple tests concurrently will
//! trigger HTTP 429. Use `--test-threads=1` for stable Free-tier runs.

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

/// MockSearchProvider — eliminates external API dependency for fast E2E tests.
///
/// Results are matched to the query keywords so that the LLM evaluator sees
/// relevant content and does not trigger spurious replan loops.
struct MockSearchProvider;

impl MockSearchProvider {
    fn results_for(query: &str, vertical: Option<&str>) -> Vec<avrag_search::SearchResult> {
        let q = query.to_lowercase();

        // Vertical-aware results: news vertical should return news-flavoured snippets.
        match vertical {
            Some("news") => {
                if q.contains("ai") || q.contains("artificial") {
                    vec![
                        avrag_search::SearchResult {
                            title: "AI News Today: Breakthrough in Large Language Models".to_string(),
                            url: "https://ainews.example.com/today".to_string(),
                            snippet: "Researchers announced a new AI model today that achieves state-of-the-art results on reasoning benchmarks. The model is expected to be deployed widely across industries within months.".to_string(),
                            citation_index: Some(1),
                        },
                        avrag_search::SearchResult {
                            title: "Tech Giants Unveil Latest AI Tools at Annual Conference".to_string(),
                            url: "https://technews.example.com/ai-conference".to_string(),
                            snippet: "Major technology companies revealed their latest artificial intelligence products today, including autonomous agents and real-time translation systems.".to_string(),
                            citation_index: Some(2),
                        },
                    ]
                } else {
                    vec![
                        avrag_search::SearchResult {
                            title: "Breaking News Today".to_string(),
                            url: "https://news.example.com".to_string(),
                            snippet: "Latest breaking news and top stories from around the world, updated in real time.".to_string(),
                            citation_index: Some(1),
                        },
                        avrag_search::SearchResult {
                            title: "World News Headlines".to_string(),
                            url: "https://worldnews.example.com".to_string(),
                            snippet: "Comprehensive coverage of global events, politics, business, and technology news.".to_string(),
                            citation_index: Some(2),
                        },
                    ]
                }
            }
            _ => {
                if q.contains("rust") || q.contains("programming") {
                    vec![
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
                    ]
                } else if q.contains("ai") || q.contains("artificial intelligence") {
                    vec![
                        avrag_search::SearchResult {
                            title: "Artificial Intelligence: A Modern Approach".to_string(),
                            url: "https://aima.cs.berkeley.edu".to_string(),
                            snippet: "Artificial intelligence (AI) is the study of agents that perceive the environment and take actions to maximize success.".to_string(),
                            citation_index: Some(1),
                        },
                        avrag_search::SearchResult {
                            title: "What is AI? Everything you need to know".to_string(),
                            url: "https://www.technologyreview.com".to_string(),
                            snippet: "AI systems can learn from data, identify patterns, and make decisions with minimal human intervention.".to_string(),
                            citation_index: Some(2),
                        },
                    ]
                } else {
                    vec![
                        avrag_search::SearchResult {
                            title: "Search Results".to_string(),
                            url: "https://example.com".to_string(),
                            snippet: format!("Information related to: {}", query),
                            citation_index: Some(1),
                        },
                    ]
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl avrag_search::SearchProvider for MockSearchProvider {
    async fn execute_search(
        &self,
        query: &str,
        vertical: Option<&str>,
    ) -> anyhow::Result<avrag_search::SearchResponse> {
        Ok(avrag_search::SearchResponse {
            query_type: "brave".to_string(),
            sub_queries: vec![query.to_string()],
            results: Self::results_for(query, vertical),
            synthesized_answer: String::new(),
            llm_usage: None,
        })
    }
}

// ---------------------------------------------------------------------------
// FirstCallEmptySearchProvider — first iteration returns empty, second
// iteration delegates to the inner real provider.
//
// State-machine logic:
//   Unused      -> first call locks into FirstBatch and returns empty.
//   FirstBatch  -> subsequent calls within 200 ms return empty (same batch).
//                  After 200 ms elapsed, state transitions to Open.
//   Open        -> all calls pass through to inner provider.
//
// The 200 ms window covers all parallel sub-query calls of one iteration;
// the Evaluate LLM call creates a ~1-3 s gap before the next iteration.
// ---------------------------------------------------------------------------

enum ProviderState {
    Unused,
    FirstBatch { start: std::time::Instant },
    Open,
}

struct FirstCallEmptySearchProvider {
    inner: Arc<dyn avrag_search::SearchProvider>,
    state: std::sync::Mutex<ProviderState>,
}

impl FirstCallEmptySearchProvider {
    fn new(inner: Arc<dyn avrag_search::SearchProvider>) -> Self {
        Self {
            inner,
            state: std::sync::Mutex::new(ProviderState::Unused),
        }
    }
}

#[async_trait::async_trait]
impl avrag_search::SearchProvider for FirstCallEmptySearchProvider {
    async fn execute_search(
        &self,
        query: &str,
        vertical: Option<&str>,
    ) -> anyhow::Result<avrag_search::SearchResponse> {
        let pass_through = {
            let mut state = self.state.lock().unwrap();
            match *state {
                ProviderState::Unused => {
                    *state = ProviderState::FirstBatch {
                        start: std::time::Instant::now(),
                    };
                    false
                }
                ProviderState::FirstBatch { start } => {
                    if start.elapsed() > std::time::Duration::from_millis(200) {
                        *state = ProviderState::Open;
                        true
                    } else {
                        false
                    }
                }
                ProviderState::Open => true,
            }
        };

        if pass_through {
            return self.inner.execute_search(query, vertical).await;
        }

        Ok(avrag_search::SearchResponse {
            query_type: "brave".to_string(),
            sub_queries: vec![query.to_string()],
            results: vec![],
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
///
/// ⚠️ Brave API rate limits (as of 2026-05):
///   Free tier: 1 QPS  |  Base: 20 QPS  |  Pro: 50 QPS
///   Running multiple Search tests in parallel may trigger 429.
///   Use `--test-threads=1` if your key is on Free tier.
#[tokio::test]
#[ignore = "requires staging: E2E_LLM_* + E2E_BRAVE_API_KEY"]
async fn search_single_pass_state_machine() {
    let config = E2EConfig::from_env().expect("E2E config not set");
    if let Err(missing) = config.validate_for_search() {
        panic!(
            "Search E2E missing environment variables: {}",
            missing.join(", ")
        );
    }
    let llm_client = config.llm_client();
    let brave_api_key = config.brave_api_key.as_deref().expect("E2E_BRAVE_API_KEY not set");

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

    // Single-pass expects Decompose → ParallelSearch → Aggregate → Evaluate → Answer (5 states).
    // In practice:
    //   - Evaluator may trigger a second iteration (replan/escalate) before Answer.
    //   - Brave API rate limit can cause web_search to fail mid-iteration, ending
    //     the state machine in parallel_search with a Degraded decision.
    // We verify the run completed (Synthesized or Degraded) and state transitions
    // were valid throughout, without requiring a specific final state.
    assert!(
        history.len() >= 3,
        "Expected at least 3 states (decompose→parallel_search→evaluate), got {}: {:?}",
        history.len(),
        history.iter().map(|s| &s.state_id).collect::<Vec<_>>()
    );
    assert!(
        matches!(
            result.final_decision,
            Some(app::agents::runtime::FinalDecision::Synthesized)
                | Some(app::agents::runtime::FinalDecision::Degraded { .. })
        ),
        "Expected final decision to be Synthesized or Degraded, got {:?}",
        result.final_decision
    );

    // --- Progressive disclosure ---
    let calls = recording_arc.calls();
    assert!(
        calls.len() >= 2,
        "Expected at least 2 LLM calls (decompose + answer), got {}",
        calls.len()
    );

    // Decompose (Plan) call: search-plan skill + web_search tool catalog
    assertions::assert_prompt_contains_skill(&calls[0].system_prompt, "search-plan");
    assertions::assert_prompt_has_tool_catalog(&calls[0].system_prompt, "search");

    // Evaluate call: may be present (LLM eval) or absent (code-based eval for sufficient results)
    if let Some(eval_call) = calls
        .iter()
        .find(|c| {
            c.user_messages
                .iter()
                .any(|m| m.content.contains("evaluate") || m.content.contains("Evaluate"))
        })
    {
        assertions::assert_prompt_contains_skill(&eval_call.system_prompt, "search-eval");
    }

    // Answer call: search-answer skill + format skills
    let answer_call = calls.last().expect("no answer call");
    assertions::assert_prompt_contains_skill(&answer_call.system_prompt, "search-answer");
    assertions::assert_prompt_has_format_skills(&answer_call.system_prompt);

    // Budget: within max (3); may be >1 if evaluator triggered a second iteration.
    if let Some(budget) = &result.budget_used {
        assertions::assert_budget_usage(budget.current, 3);
    }
}

/// Test: Search vertical escalation — evaluation finds results insufficient,
/// triggers another search with a different vertical.
///
/// ⚠️ See `search_single_pass_state_machine` for Brave API rate-limit notes.
#[tokio::test]
#[ignore = "requires staging: E2E_LLM_* + E2E_BRAVE_API_KEY"]
async fn search_vertical_escalation_state_machine() {
    let config = E2EConfig::from_env().expect("E2E config not set");
    if let Err(missing) = config.validate_for_search() {
        panic!(
            "Search E2E missing environment variables: {}",
            missing.join(", ")
        );
    }
    let llm_client = config.llm_client();
    let brave_api_key = config.brave_api_key.as_deref().expect("E2E_BRAVE_API_KEY not set");

    let recording = RecordingLlmProvider::new(Arc::new(llm_client.clone()));
    let recording_arc = Arc::new(recording);

    let real_executor = build_search_executor(brave_api_key);
    // Wrap real executor: first call returns empty to force EscalateVertical,
    // second call (news vertical) hits real Brave API.
    let search_executor: Arc<dyn avrag_search::SearchProvider> =
        Arc::new(FirstCallEmptySearchProvider::new(real_executor));
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

    // Budget: within max (3)
    if let Some(budget) = &result.budget_used {
        assertions::assert_budget_usage(budget.current, 3);
    }

    // Escalation MUST have occurred (first call returns empty → EscalateVertical)
    let has_escalation = history.windows(2).any(|w| {
        w[0].state_id == "evaluate" && w[1].state_id == "parallel_search"
    });
    assert!(
        has_escalation,
        "Expected vertical escalation (evaluate → parallel_search). History: {:?}",
        history.iter().map(|s| &s.state_id).collect::<Vec<_>>()
    );

    // With escalation: at least 7 states
    // Decompose → ParallelSearch → Aggregate → Evaluate → ParallelSearch → Aggregate → Evaluate → Answer
    assert!(
        history.len() >= 7,
        "Expected >= 7 states with escalation, got {}: {:?}",
        history.len(),
        history.iter().map(|s| &s.state_id).collect::<Vec<_>>()
    );

    // Final decision should be Synthesized (second search via news vertical succeeds)
    assert!(
        matches!(result.final_decision, Some(app::agents::runtime::FinalDecision::Synthesized)),
        "Expected Synthesized after escalation, got {:?}",
        result.final_decision
    );
}
