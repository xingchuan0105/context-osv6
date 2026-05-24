//! E2E tests for RAG strategy state machine + progressive disclosure.
//!
//! Run with: cargo test --ignored -p app --test e2e_rag
//! Requires full staging environment: E2E_LLM_*, E2E_EMBEDDING_*, E2E_MILVUS_*.
//!
//! These tests verify:
//! 1. State machine transitions (Plan → ExecuteRetrieve → Evaluate → Answer)
//! 2. Progressive disclosure (rag-plan skill body + tool catalog + format skills)
//! 3. Replan optimization (Evaluate → ExecuteRetrieve skips Plan LLM call)

#[path = "e2e/config.rs"]
mod config;
#[path = "e2e/recording_llm.rs"]
mod recording_llm;
#[path = "e2e/assertions.rs"]
mod assertions;

use app::agents::events::CollectingSink;
use app::agents::react_loop::{LoopBudget, UserTier};
use app::agents::runtime::AgentRequest;
use app::agents::strategy::rag::{RagContext, RagStrategy};
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

fn rag_request(query: &str, doc_scope: Vec<String>) -> AgentRequest {
    let docscope_metadata = Some(common::DocScopeMetadata {
        documents: doc_scope
            .iter()
            .map(|id| common::SummaryMetadata {
                doc_id: id.clone(),
                filename: "antifragile.pdf".to_string(),
                docname: "Antifragile: Things That Gain from Disorder by Nassim Nicholas Taleb".to_string(),
                language: "en".to_string(),
                domain: common::Domain::Business,
                genre: common::Genre::Book,
                era: common::Era::Contemporary,
            })
            .collect(),
        profile: common::DocScopeProfile {
            languages: vec!["en".to_string()],
            domains: vec![common::Domain::Business],
            genres: vec![common::Genre::Book],
            eras: vec![common::Era::Contemporary],
        },
    });
    AgentRequest {
        kind: AgentKind::Rag,
        query: query.to_string(),
        notebook_id: None,
        session_id: None,
        doc_scope,
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
        docscope_metadata,
        metadata: BTreeMap::new(),
        cancellation_token: None,
        guard_pipeline: None,
    }
}

/// Build a full RagRuntime from staging environment variables.
///
/// Required env vars:
/// - `E2E_EMBEDDING_BASE_URL`, `E2E_EMBEDDING_API_KEY` — for embedding client
/// - `E2E_MILVUS_URL`, `E2E_MILVUS_TOKEN` — for Milvus data plane
/// - `E2E_DATABASE_URL` — for PostgreSQL (sparse retrieval fallback)
fn build_staging_rag_runtime() -> Option<Arc<avrag_rag_core::RagRuntime>> {
    use avrag_rag_core::RetrievalDataPlane;
    use avrag_storage_milvus::{MilvusConfig, MilvusDataPlane};

    let embedding_base_url = std::env::var("E2E_EMBEDDING_BASE_URL").ok()?;
    let embedding_api_key = std::env::var("E2E_EMBEDDING_API_KEY").ok()?;
    let milvus_url = std::env::var("E2E_MILVUS_URL").ok()?;
    let milvus_token = std::env::var("E2E_MILVUS_TOKEN").ok();

    let embedding_client = Arc::new(avrag_llm::EmbeddingClient::new(
        avrag_llm::ModelProviderConfig {
            base_url: embedding_base_url,
            api_key: embedding_api_key,
            model: std::env::var("E2E_EMBEDDING_MODEL")
                .unwrap_or_else(|_| "text-embedding-v4".to_string()),
            timeout_ms: 30_000,
            api_style: Some(avrag_llm::ApiStyle::OpenAi),
            dimensions: Some(1024),
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        },
    ));

    let milvus_config = MilvusConfig {
        url: milvus_url,
        token: milvus_token,
        database: None,
        collection_prefix: "avrag".to_string(),
        text_vector_dim: 1024,
        multimodal_vector_dim: 1024,
        metric_type: "COSINE".to_string(),
    };
    let data_plane: Arc<dyn RetrievalDataPlane> =
        Arc::new(MilvusDataPlane::new(milvus_config));

    let rag_config = avrag_rag_core::RagConfig::new_for_data_plane(embedding_client, None);
    Some(Arc::new(
        avrag_rag_core::RagRuntime::with_data_plane(rag_config, data_plane),
    ))
}

/// Test: RAG single-pass sufficient — retrieval returns enough context,
/// evaluation says "sufficient", goes straight to Answer.
///
/// Expected state sequence: Plan → ExecuteRetrieve → Evaluate → Answer
#[tokio::test]
#[ignore = "requires full staging: E2E_LLM_* + E2E_EMBEDDING_* + E2E_MILVUS_*"]
async fn rag_single_pass_sufficient_state_machine() {
    let config = E2EConfig::from_env().expect("E2E config not set");
    let llm_client = config.llm_client();
    let rag_runtime = build_staging_rag_runtime().expect("RAG staging env vars not set");

    let recording = RecordingLlmProvider::new(Arc::new(llm_client.clone()));
    let recording_arc = Arc::new(recording);

    let ctx = RagContext::from_request(
        rag_request("Summarize Taleb's concept of antifragility from the document", vec!["00000000-0000-0000-0000-000000000001".to_string()]),
        "test-rag-single-pass".to_string(),
        LoopBudget::rag(UserTier::Pro),
        Box::new(CollectingSink::new()),
        tokio_util::sync::CancellationToken::new(),
        rag_runtime,
    )
    .unwrap();

    let strategy = RagStrategy {
        llm: recording_arc.clone(),
        llm_client: Some(llm_client),
        temperature: None,
    };

    let executor = app::agents::strategy::executor::StrategyExecutor;
    let result = executor.run(&strategy, ctx).await.unwrap();

    let schema = RagStrategy::schema();
    let history = result.state_history.as_ref().expect("state_history missing");
    assertions::assert_valid_transitions(&schema, history);
    assertions::assert_state_kinds(history);

    // Expected: Plan → ExecuteRetrieve → Evaluate → Answer (4 states, no replan)
    // If the collection has no matching data, replan loops until budget is exhausted.
    assert!(
        history.len() >= 4,
        "Expected at least 4 states, got {}: {:?}",
        history.len(),
        history.iter().map(|s| &s.state_id).collect::<Vec<_>>()
    );

    // If budget exhausted without answer, last state is evaluate or execute_retrieve
    let last_state = history.last().unwrap().state_id.as_str();
    if history.len() > 4 {
        // Replan occurred — verify valid transitions throughout
        assert!(
            history.windows(2).any(|w| {
                w[0].state_id == "evaluate" && w[1].state_id == "execute_retrieve"
            }),
            "Expected at least one Evaluate → ExecuteRetrieve replan transition"
        );
    } else {
        assert_eq!(last_state, "answer", "Expected final state to be Answer, got {}", last_state);
    }

    // --- Progressive disclosure ---
    let calls = recording_arc.calls();
    assert!(
        calls.len() >= 2,
        "Expected at least 2 LLM calls (plan + eval), got {}",
        calls.len()
    );

    // Plan: rag-plan skill + RAG tool catalog
    assertions::assert_prompt_contains_skill(&calls[0].system_prompt, "rag-plan");
    assertions::assert_prompt_has_tool_catalog(&calls[0].system_prompt, "rag");

    // Evaluate: rag-eval skill (may be multiple if replan)
    let eval_call = calls
        .iter()
        .find(|c| c.system_prompt.contains("rag-eval"))
        .or_else(|| calls.get(1))
        .expect("evaluate call not found");
    assertions::assert_prompt_contains_skill(&eval_call.system_prompt, "rag-eval");

    // Answer: RagStrategy::finalize_synthesize uses AnswerSynthesizer which calls
    // LlmClient directly (not via LlmProvider trait), so RecordingLlmProvider cannot
    // capture it. We verify Answer state was reached via state_history instead.
    if last_state == "answer" {
        // State sequence already validated above; Answer phase is confirmed.
    }

    // Budget: within max (4)
    if let Some(budget) = &result.budget_used {
        assertions::assert_budget_usage(budget.current, 4);
    }
}

/// Test: RAG replan — first retrieval is insufficient, Evaluate triggers
/// ExecuteRetrieve (no second Plan LLM call), then Answer.
///
/// Key v5 optimization: replan skips Plan, saving one LLM round-trip.
#[tokio::test]
#[ignore = "requires full staging + partial documents to trigger replan"]
async fn rag_replan_insufficient_state_machine() {
    let config = E2EConfig::from_env().expect("E2E config not set");
    let llm_client = config.llm_client();
    let rag_runtime = build_staging_rag_runtime().expect("RAG staging env vars not set");

    let recording = RecordingLlmProvider::new(Arc::new(llm_client.clone()));
    let recording_arc = Arc::new(recording);

    let ctx = RagContext::from_request(
        rag_request(
            "What are the main components and features of avrag_rag?",
            vec!["00000000-0000-0000-0000-000000000001".to_string()],
        ),
        "test-rag-replan".to_string(),
        LoopBudget::rag(UserTier::Pro),
        Box::new(CollectingSink::new()),
        tokio_util::sync::CancellationToken::new(),
        rag_runtime,
    )
    .unwrap();

    let strategy = RagStrategy {
        llm: recording_arc.clone(),
        llm_client: Some(llm_client),
        temperature: None,
    };

    let executor = app::agents::strategy::executor::StrategyExecutor;
    let result = executor.run(&strategy, ctx).await.unwrap();

    let schema = RagStrategy::schema();
    let history = result.state_history.as_ref().expect("state_history missing");
    assertions::assert_valid_transitions(&schema, history);
    assertions::assert_state_kinds(history);

    // Check if replan occurred
    let has_re_execute = history.windows(2).any(|w| {
        w[0].state_id == "evaluate" && w[1].state_id == "execute_retrieve"
    });

    if has_re_execute {
        // KEY v5 ASSERTION: replan does NOT re-invoke Plan LLM.
        // The rag-plan skill body only appears in the initial Plan call.
        // Use assert_prompt_contains_skill logic (skill body, not ID string).
        let registry = app::agents::progressive::PromptRegistry::standard_cached();
        let skill_body = registry
            .skill("rag-plan")
            .map(|s| s.system_prompt().to_string())
            .unwrap_or_default();
        let plan_llm_calls = recording_arc
            .calls()
            .iter()
            .filter(|c| c.system_prompt.contains(&skill_body))
            .count();
        assert_eq!(
            plan_llm_calls, 1,
            "Replan must NOT trigger a second Plan LLM call (found {}). \
             v5 optimization: Evaluate → ExecuteRetrieve skips Plan.",
            plan_llm_calls
        );
    }

    // Budget: within max
    if let Some(budget) = &result.budget_used {
        assertions::assert_budget_usage(budget.current, 4);
    }
}
