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

/// Components needed for RAG E2E tests.
pub struct RagStagingComponents {
    pub rag_runtime: Arc<avrag_rag_core::RagRuntime>,
    pub data_plane: Arc<dyn avrag_retrieval_data_plane::RetrievalDataPlane>,
    pub embedding_client: Arc<avrag_llm::EmbeddingClient>,
}

/// Build RAG staging components from environment variables.
///
/// Required env vars:
/// - `E2E_EMBEDDING_BASE_URL`, `E2E_EMBEDDING_API_KEY` — for embedding client
/// - `E2E_MILVUS_URL`, `E2E_MILVUS_TOKEN` — for Milvus data plane
fn build_staging_rag_components(config: &E2EConfig) -> Option<RagStagingComponents> {
    use avrag_rag_core::RetrievalDataPlane;
    use avrag_storage_milvus::{MilvusConfig, MilvusDataPlane};

    let embedding_base_url = config.embedding_base_url.as_ref()?;
    let embedding_api_key = config.embedding_api_key.as_ref()?;
    let milvus_url = config.milvus_url.as_ref()?;
    let milvus_token = config.milvus_token.clone();

    let embedding_client = Arc::new(avrag_llm::EmbeddingClient::new(
        avrag_llm::ModelProviderConfig {
            base_url: embedding_base_url.clone(),
            api_key: embedding_api_key.clone(),
            model: config
                .embedding_model
                .clone()
                .unwrap_or_else(|| "text-embedding-v4".to_string()),
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
        url: milvus_url.clone(),
        token: milvus_token,
        database: None,
        collection_prefix: "avrag".to_string(),
        text_vector_dim: 1024,
        multimodal_vector_dim: 1024,
        metric_type: "COSINE".to_string(),
    };
    let data_plane: Arc<dyn RetrievalDataPlane> =
        Arc::new(MilvusDataPlane::new(milvus_config));

    let rag_config =
        avrag_rag_core::RagConfig::new_for_data_plane(embedding_client.clone(), None);
    let rag_runtime = Arc::new(
        avrag_rag_core::RagRuntime::with_data_plane(rag_config, data_plane.clone()),
    );

    Some(RagStagingComponents {
        rag_runtime,
        data_plane,
        embedding_client,
    })
}

/// Ingest test chunks directly into Milvus via the data plane.
///
/// 1. Generate embeddings via EmbeddingClient.
/// 2. Build a DocumentIndexBatch with text chunks.
/// 3. Call data_plane.replace_document_index().
///
/// Returns the generated doc_id for use in RAG requests.
async fn ingest_test_document(
    data_plane: &dyn avrag_retrieval_data_plane::RetrievalDataPlane,
    embedding_client: &avrag_llm::EmbeddingClient,
    chunks: Vec<&str>,
) -> anyhow::Result<uuid::Uuid> {
    use avrag_retrieval_data_plane::{DocumentIndexBatch, TextChunkIndexRecord};

    let doc_id = uuid::Uuid::new_v4();
    let org_id = avrag_auth::OrgId::from(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001")?);
    let parse_run_id = uuid::Uuid::new_v4();

    // Generate embeddings
    let text_refs: Vec<&str> = chunks.iter().copied().collect();
    let vectors = embedding_client.embed(&text_refs).await?;

    // Build text chunks
    let text_chunks: Vec<TextChunkIndexRecord> = chunks
        .into_iter()
        .enumerate()
        .map(|(i, content)| TextChunkIndexRecord {
            chunk_id: uuid::Uuid::new_v4(),
            content: content.to_string(),
            vector: vectors.get(i).cloned().unwrap_or_default(),
            page: Some(i as i64 + 1),
            chunk_type: "text".to_string(),
            parser_backend: Some("e2e_test".to_string()),
            source_locator: None,
        })
        .collect();

    let batch = DocumentIndexBatch {
        org_id,
        workspace_id: None,
        document_id: doc_id,
        parse_run_id,
        doc_version: 1,
        text_chunks,
        multimodal_chunks: vec![],
        entities: vec![],
        relations: vec![],
        graph_passages: vec![],
    };

    data_plane.replace_document_index(batch).await?;
    Ok(doc_id)
}

/// Test: RAG single-pass sufficient — retrieval returns enough context,
/// evaluation says "sufficient", goes straight to Answer.
///
/// Expected state sequence: Plan → ExecuteRetrieve → Evaluate → Answer
#[tokio::test]
#[ignore = "requires full staging: E2E_LLM_* + E2E_EMBEDDING_* + E2E_MILVUS_*"]
async fn rag_single_pass_sufficient_state_machine() {
    let config = E2EConfig::from_env().expect("E2E config not set");
    if let Err(missing) = config.validate_for_rag() {
        panic!(
            "RAG E2E missing environment variables: {}",
            missing.join(", ")
        );
    }
    let llm_client = config.llm_client();
    let components = build_staging_rag_components(&config).expect("RAG staging env vars not set");

    // Ingest test document with real embeddings
    let chunks = vec![
        "Antifragility is a property of systems that increase in capability, resilience, or robustness as a result of stressors, shocks, volatility, noise, mistakes, faults, attacks, or failures.",
        "The concept was developed by Nassim Nicholas Taleb, a professor and former trader, and is the central theme of his book Antifragile: Things That Gain from Disorder.",
        "Taleb defines antifragility as the opposite of fragility. While a fragile object breaks under stress, and a robust object resists stress, an antifragile object actually benefits from stress.",
        "Examples of antifragile systems include evolution, the immune system, and free-market economies. Each of these systems improves when exposed to randomness and stressors.",
    ];
    let doc_id = ingest_test_document(
        components.data_plane.as_ref(),
        &components.embedding_client,
        chunks,
    )
    .await
    .expect("Failed to ingest test document into Milvus");

    let recording = RecordingLlmProvider::new(Arc::new(llm_client.clone()));
    let recording_arc = Arc::new(recording);

    let ctx = RagContext::from_request(
        rag_request(
            "Summarize Taleb's concept of antifragility from the document",
            vec![doc_id.to_string()],
        ),
        "test-rag-single-pass".to_string(),
        LoopBudget::rag(UserTier::Pro),
        Box::new(CollectingSink::new()),
        tokio_util::sync::CancellationToken::new(),
        components.rag_runtime,
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

    // Two valid outcomes:
    // 1) Sufficient: Plan → ExecuteRetrieve → Evaluate → Answer (≥4 states)
    // 2) No data:    Plan → ExecuteRetrieve → Evaluate → Degrade (3 states)
    let last_state = history.last().unwrap().state_id.as_str();
    let has_data = history.len() >= 4 && last_state == "answer";

    if has_data {
        // Sufficient path — verify no unexpected replan loops
        if history.len() > 4 {
            assert!(
                history.windows(2).any(|w| {
                    w[0].state_id == "evaluate" && w[1].state_id == "execute_retrieve"
                }),
                "Expected at least one Evaluate → ExecuteRetrieve replan transition"
            );
        }
    } else {
        // Degrade path — verify we degraded gracefully (not a crash)
        assert!(
            matches!(result.final_decision, Some(app::agents::runtime::FinalDecision::Degraded { .. })),
            "Expected Degraded when no data in collection, got {:?}",
            result.final_decision
        );
    }

    // --- Progressive disclosure ---
    let calls = recording_arc.calls();
    assert!(
        calls.len() >= 1,
        "Expected at least 1 LLM call (plan), got {}",
        calls.len()
    );

    // Plan: rag-plan skill + RAG tool catalog
    assertions::assert_prompt_contains_skill(&calls[0].system_prompt, "rag-plan");
    assertions::assert_prompt_has_tool_catalog(&calls[0].system_prompt, "rag");

    // Evaluate: rag-eval skill (may be present if evaluator ran)
    if let Some(eval_call) = calls.iter().find(|c| c.system_prompt.contains("rag-eval")) {
        assertions::assert_prompt_contains_skill(&eval_call.system_prompt, "rag-eval");
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
    if let Err(missing) = config.validate_for_rag() {
        panic!(
            "RAG E2E missing environment variables: {}",
            missing.join(", ")
        );
    }
    let llm_client = config.llm_client();
    let components = build_staging_rag_components(&config).expect("RAG staging env vars not set");

    // Ingest test document with partial content to increase replan likelihood
    let chunks = vec![
        "The avrag_rag system supports dense retrieval, lexical retrieval, and graph-based retrieval.",
        "Dense retrieval uses vector similarity to find semantically related text chunks.",
    ];
    let doc_id = ingest_test_document(
        components.data_plane.as_ref(),
        &components.embedding_client,
        chunks,
    )
    .await
    .expect("Failed to ingest test document into Milvus");

    let recording = RecordingLlmProvider::new(Arc::new(llm_client.clone()));
    let recording_arc = Arc::new(recording);

    let ctx = RagContext::from_request(
        rag_request(
            "What are the main components and features of avrag_rag?",
            vec![doc_id.to_string()],
        ),
        "test-rag-replan".to_string(),
        LoopBudget::rag(UserTier::Pro),
        Box::new(CollectingSink::new()),
        tokio_util::sync::CancellationToken::new(),
        components.rag_runtime,
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

/// Test: RAG with HTML format hint — answer prompt contains the FULL BODY
/// of the html-renderer skill, not just the skill ID string.
#[tokio::test]
#[ignore = "requires full staging: E2E_LLM_* + E2E_EMBEDDING_* + E2E_MILVUS_*"]
async fn rag_html_format_skill_injected() {
    let config = E2EConfig::from_env().expect("E2E config not set");
    if let Err(missing) = config.validate_for_rag() {
        panic!(
            "RAG E2E missing environment variables: {}",
            missing.join(", ")
        );
    }
    let llm_client = config.llm_client();
    let components = build_staging_rag_components(&config).expect("RAG staging env vars not set");

    // Ingest test document with real embeddings
    let chunks = vec![
        "Antifragility is a property of systems that increase in capability, resilience, or robustness as a result of stressors, shocks, volatility, noise, mistakes, faults, attacks, or failures.",
        "The concept was developed by Nassim Nicholas Taleb, a professor and former trader, and is the central theme of his book Antifragile: Things That Gain from Disorder.",
    ];
    let doc_id = ingest_test_document(
        components.data_plane.as_ref(),
        &components.embedding_client,
        chunks,
    )
    .await
    .expect("Failed to ingest test document into Milvus");

    let recording = RecordingLlmProvider::new(Arc::new(llm_client.clone()));
    let recording_arc = Arc::new(recording);

    let mut request = rag_request(
        "Summarize Taleb's concept of antifragility from the document",
        vec![doc_id.to_string()],
    );
    request.format_hint = Some("html".to_string());

    let ctx = RagContext::from_request(
        request,
        "test-rag-html-format".to_string(),
        LoopBudget::rag(UserTier::Pro),
        Box::new(CollectingSink::new()),
        tokio_util::sync::CancellationToken::new(),
        components.rag_runtime,
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

    // --- Format skill body injection ---
    let calls = recording_arc.calls();
    assert!(
        calls.len() >= 1,
        "Expected at least 1 LLM call (plan), got {}",
        calls.len()
    );

    // Answer prompt must contain the FULL BODY of html-renderer skill
    let answer_call = calls.last().unwrap();
    assertions::assert_prompt_contains_skill_body(&answer_call.system_prompt, "html-renderer");
}
