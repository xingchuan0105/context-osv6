//! E2E tests for ingestion → answer pipeline.
//!
//! Run with: cargo test --ignored -p app --test strategy_ingestion_answer -- --test-threads=1

#[path = "strategy_e2e/assertions.rs"]
mod assertions;
#[path = "strategy_e2e/config.rs"]
mod config;
#[path = "strategy_e2e/playwright_helper.rs"]
mod playwright_helper;
#[path = "strategy_e2e/recording_llm.rs"]
mod recording_llm;
#[path = "strategy_e2e/result_serializer.rs"]
mod result_serializer;

use app::agents::AgentKind;
use app::agents::events::CollectingSink;
use app::agents::react_loop::{LoopBudget, UserTier};
use app::agents::runtime::AgentRequest;
use app::agents::strategy::rag::{RagContext, RagStrategy};
use common::ChatTurnInput;
use std::collections::BTreeMap;
use std::sync::Arc;

use config::E2EConfig;
use recording_llm::RecordingLlmProvider;

static INGESTION_PERMITS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);

fn test_auth_context() -> serde_json::Value {
    serde_json::json!({
        "org_id": "00000000-0000-0000-0000-000000000001",
        "subject_kind": "User",
        "permissions": []
    })
}

struct MilvusTestGuard {
    collection_prefix: String,
}

impl MilvusTestGuard {
    fn new(collection_prefix: String) -> Self {
        Self { collection_prefix }
    }
}

impl Drop for MilvusTestGuard {
    fn drop(&mut self) {
        eprintln!(
            "[WARN] Milvus collection with prefix '{}' may not have been cleaned up",
            self.collection_prefix
        );
    }
}

/// Read fixture file and parse it through the ingestion text parser.
async fn read_fixture(filename: &str) -> Vec<u8> {
    let path = std::path::PathBuf::from("tests/fixtures").join(filename);
    tokio::fs::read(&path)
        .await
        .unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", path.display(), e))
}

/// Build run-scoped RAG components with a unique collection prefix.
fn build_run_scoped_rag_components(
    config: &E2EConfig,
    collection_prefix: &str,
) -> Option<(
    Arc<avrag_rag_core::RagRuntime>,
    Arc<dyn avrag_retrieval_data_plane::RetrievalDataPlane>,
    Arc<avrag_llm::EmbeddingClient>,
)> {
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
        collection_prefix: collection_prefix.to_string(),
        text_vector_dim: 1024,
        multimodal_vector_dim: 1024,
        metric_type: "COSINE".to_string(),
    };
    let data_plane: Arc<dyn RetrievalDataPlane> = Arc::new(MilvusDataPlane::new(milvus_config));

    let rag_config = avrag_rag_core::RagConfig::new_for_data_plane(embedding_client.clone(), None);
    let rag_runtime = Arc::new(avrag_rag_core::RagRuntime::with_data_plane(
        rag_config,
        data_plane.clone(),
    ));

    Some((rag_runtime, data_plane, embedding_client))
}

/// Parse fixture, chunk it, embed, and write to Milvus via data plane.
async fn ingest_fixture_document(
    data_plane: &dyn avrag_retrieval_data_plane::RetrievalDataPlane,
    embedding_client: &avrag_llm::EmbeddingClient,
    fixture_bytes: &[u8],
    fixture_name: &str,
    run_id: &str,
) -> anyhow::Result<uuid::Uuid> {
    use avrag_retrieval_data_plane::{DocumentIndexBatch, TextChunkIndexRecord};

    // 1. Parse via ingestion TextParser
    use ingestion::parser::DocumentParser;
    let parser = ingestion::parser::TextParser;
    let parsed = parser.parse(fixture_bytes, fixture_name).await?;

    // 2. Chunk via ingestion chunker
    let policy = ingestion::chunker::ChunkPolicy::default();
    let chunk_items = ingestion::chunker::build_chunk_items(&parsed, fixture_name, &policy);

    let doc_id = uuid::Uuid::new_v4();
    let org_id = avrag_auth::OrgId::from(uuid::Uuid::parse_str(
        "00000000-0000-0000-0000-000000000001",
    )?);
    let parse_run_id = uuid::Uuid::new_v4();

    // 3. Generate embeddings for chunks
    let texts: Vec<&str> = chunk_items.iter().map(|item| item.text.as_str()).collect();
    let vectors = embedding_client.embed(&texts).await?;

    // 4. Build text chunks with run-scoped metadata
    let text_chunks: Vec<TextChunkIndexRecord> = chunk_items
        .into_iter()
        .enumerate()
        .map(|(i, item)| TextChunkIndexRecord {
            chunk_id: uuid::Uuid::new_v4(),
            content: item.text,
            vector: vectors.get(i).cloned().unwrap_or_default(),
            page: Some(item.page as i64),
            chunk_type: item.kind,
            parser_backend: Some("text".to_string()),
            source_locator: Some(serde_json::json!({
                "run_id": run_id,
                "fixture": fixture_name,
            })),
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

fn rag_request(query: &str, doc_scope: Vec<String>) -> AgentRequest {
    let docscope_metadata = Some(common::DocScopeMetadata {
        documents: doc_scope
            .iter()
            .map(|id| common::SummaryMetadata {
                doc_id: id.clone(),
                filename: "antifragile.txt".to_string(),
                docname: "Antifragility Explained".to_string(),
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

#[tokio::test]
#[ignore = "requires staging environment (E2E_LLM_*, E2E_EMBEDDING_*, E2E_MILVUS_*)"]
async fn ingestion_answer_pipeline() {
    let _permit = INGESTION_PERMITS.acquire().await.unwrap();

    let start = std::time::Instant::now();
    let config = E2EConfig::from_env().expect("E2E config required");
    if let Err(missing) = config.validate_for_rag() {
        panic!("RAG E2E missing env vars: {}", missing.join(", "));
    }

    let run_id = E2EConfig::generate_run_id();
    let collection_prefix = format!("e2e_ingestion_{}", &run_id[..16]).replace("-", "_");
    let output_dir = std::path::PathBuf::from("tests/e2e_output")
        .join(&run_id)
        .join("ingestion_answer");
    std::fs::create_dir_all(&output_dir).unwrap();

    let _guard = MilvusTestGuard::new(collection_prefix.clone());

    let llm_client = config.llm_client();

    // 1. Build run-scoped RAG components
    let (rag_runtime, data_plane, embedding_client) =
        build_run_scoped_rag_components(&config, &collection_prefix)
            .expect("Failed to build RAG components");

    // Ensure Milvus collections exist before ingesting
    data_plane
        .ensure_schema()
        .await
        .expect("Failed to ensure Milvus schema");

    // 2. Read and ingest fixture
    let fixture_bytes = read_fixture("antifragile.txt").await;
    let doc_id = ingest_fixture_document(
        data_plane.as_ref(),
        &embedding_client,
        &fixture_bytes,
        "antifragile.txt",
        &run_id,
    )
    .await
    .expect("Failed to ingest fixture document");

    // 3. Small delay to allow Milvus segment flush
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // 4. Run RAG query
    let recording = RecordingLlmProvider::new(Arc::new(llm_client.clone()));
    let recording_arc = Arc::new(recording);

    let ctx = RagContext::from_request(
        rag_request(
            "What is antifragility and who coined the term?",
            vec![doc_id.to_string()],
        ),
        format!("{}-ingestion-answer", run_id),
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
    let agent_result = executor.run(&strategy, ctx).await;

    // 5. Build result
    let mut result = result_serializer::TestResult {
        run_id: run_id.clone(),
        test_name: "ingestion_answer_pipeline".to_string(),
        query: "What is antifragility and who coined the term?".to_string(),
        strategy: "Rag".to_string(),
        format_skill: None,
        status: result_serializer::TestStatus::Failed,
        answer_text: String::new(),
        answer_html: None,
        screenshot_path: None,
        llm_calls: recording_arc.calls(),
        tool_calls: vec![],
        retrieval_hits: None,
        token_usage: None,
        duration_ms: start.elapsed().as_millis() as u64,
        timestamp: chrono::Utc::now().to_rfc3339(),
        error_message: None,
        diagnostics: None,
        failure_kind: None,
    };

    match agent_result {
        Ok(run_result) => {
            result.answer_text = run_result.answer.clone();

            // Verify answer contains document-relevant content
            let answer_lower = run_result.answer.to_lowercase();
            let has_citation = answer_lower.contains("taleb")
                || answer_lower.contains("antifragile")
                || answer_lower.contains("disorder");

            if has_citation {
                result.status = result_serializer::TestStatus::Passed;
            } else {
                result.error_message = Some(format!(
                    "Answer missing expected citations. Got: {}",
                    run_result.answer
                ));
                result.failure_kind = Some(result_serializer::TestFailureKind::AssertionFailed);
            }
        }
        Err(e) => {
            result.error_message = Some(format!("Agent execution failed: {}", e));
            result.failure_kind = Some(result_serializer::TestFailureKind::ExecutionFailed);
        }
    }

    // 6. Persist result
    result_serializer::save_test_result(
        &output_dir,
        &result,
        result_serializer::ArtifactRetentionPolicy::OnFailure,
    )
    .ok();

    // Final assertion
    assert!(
        result.status == result_serializer::TestStatus::Passed,
        "Ingestion-answer test failed: {:?}",
        result.error_message
    );
}
