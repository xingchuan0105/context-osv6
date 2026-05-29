//! E2E tests for format output across strategies and format skills.
//!
//! Run with: cargo test --ignored -p app --test strategy_format_output

#[path = "strategy_e2e/config.rs"]
mod config;
#[path = "strategy_e2e/recording_llm.rs"]
mod recording_llm;
#[path = "strategy_e2e/assertions.rs"]
mod assertions;
#[path = "strategy_e2e/playwright_helper.rs"]
mod playwright_helper;
#[path = "strategy_e2e/result_serializer.rs"]
mod result_serializer;

use app::agents::events::CollectingSink;
use app::agents::react_loop::{LoopBudget, UserTier};
use app::agents::runtime::AgentRequest;
use app::agents::AgentKind;
use common::ChatTurnInput;
use std::collections::BTreeMap;
use std::sync::Arc;

use config::E2EConfig;
use recording_llm::RecordingLlmProvider;

#[derive(Debug, Clone, Copy)]
enum StrategyKind {
    Chat,
    Rag,
    Search,
}

struct FormatScenario {
    strategy: StrategyKind,
    format_skill: &'static str,
    query: &'static str,
    expected_markers: &'static [&'static str],
}

const SCENARIOS: &[FormatScenario] = &[
    FormatScenario {
        strategy: StrategyKind::Chat,
        format_skill: "presentation-html",
        query: "生成一个 PPT 总结 Rust 所有权机制",
        expected_markers: &["slide", "presentation"],
    },
    FormatScenario {
        strategy: StrategyKind::Rag,
        format_skill: "presentation-html",
        query: "根据文档，生成一个 PPT 总结其核心观点",
        expected_markers: &["slide", "presentation"],
    },
    FormatScenario {
        strategy: StrategyKind::Chat,
        format_skill: "html-renderer",
        query: "用 HTML 页面展示 Rust 错误处理最佳实践",
        expected_markers: &["<html", "<body"],
    },
    FormatScenario {
        strategy: StrategyKind::Chat,
        format_skill: "step-by-step-tutor",
        query: "教我理解 Rust 生命周期",
        expected_markers: &["Step", "step", "第一步", "## 1.", "## 2.", "##"],
    },
    FormatScenario {
        strategy: StrategyKind::Search,
        format_skill: "html-renderer",
        query: "用 HTML 页面展示 Rust 并发模型",
        expected_markers: &["<html", "<body"],
    },
    FormatScenario {
        strategy: StrategyKind::Search,
        format_skill: "presentation-html",
        query: "生成一个 PPT 总结 Rust 所有权机制",
        expected_markers: &["slide", "presentation"],
    },
];

fn build_request(strategy: StrategyKind, query: &str) -> AgentRequest {
    let kind = match strategy {
        StrategyKind::Chat => AgentKind::Chat,
        StrategyKind::Rag => AgentKind::Rag,
        StrategyKind::Search => AgentKind::Search,
    };

    AgentRequest {
        kind,
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
        auth_context: serde_json::json!({
            "org_id": "00000000-0000-0000-0000-000000000001",
            "subject_kind": "User",
            "permissions": []
        }),
        docscope_metadata: None,
        metadata: BTreeMap::new(),
        cancellation_token: None,
        guard_pipeline: None,
    }
}

async fn run_format_scenario(
    scenario: &FormatScenario,
    run_id: &str,
    output_dir: &std::path::Path,
) -> result_serializer::TestResult {
    use result_serializer::*;
    use playwright_helper::*;

    let start = std::time::Instant::now();
    let test_name = format!(
        "{}__{}__{}",
        match scenario.strategy {
            StrategyKind::Chat => "chat",
            StrategyKind::Rag => "rag",
            StrategyKind::Search => "search",
        },
        scenario.format_skill,
        sanitize_filename(scenario.query)
    );

    let mut result = TestResult {
        run_id: run_id.to_string(),
        test_name: test_name.clone(),
        query: scenario.query.to_string(),
        strategy: format!("{:?}", scenario.strategy),
        format_skill: Some(scenario.format_skill.to_string()),
        status: TestStatus::Failed,
        answer_text: String::new(),
        answer_html: None,
        screenshot_path: None,
        llm_calls: vec![],
        tool_calls: vec![],
        retrieval_hits: None,
        token_usage: None,
        duration_ms: 0,
        timestamp: chrono::Utc::now().to_rfc3339(),
        error_message: None,
        diagnostics: None,
        failure_kind: None,
    };

    // Check Playwright availability
    if !check_playwright_available().await {
        result.status = TestStatus::Skipped;
        result.error_message = Some("Playwright not available".to_string());
        result.failure_kind = Some(TestFailureKind::DependencyMissing);
        result.duration_ms = start.elapsed().as_millis() as u64;
        return result;
    }

    // Run agent
    let config = match E2EConfig::from_env() {
        Some(c) => c,
        None => {
            result.status = TestStatus::Skipped;
            result.error_message = Some("E2E config not available".to_string());
            result.failure_kind = Some(TestFailureKind::DependencyMissing);
            result.duration_ms = start.elapsed().as_millis() as u64;
            return result;
        }
    };

    let llm_client = config.llm_client();
    let llm_arc: Arc<dyn avrag_llm::LlmProvider> = Arc::new(llm_client);
    let recording = RecordingLlmProvider::new(llm_arc);
    let recording_arc = Arc::new(recording);

    let request = build_request(scenario.strategy, scenario.query);
    let sink = Box::new(CollectingSink::new());
    let trace_id = format!("{}-{}", run_id, test_name);

    // Build context and strategy based on strategy kind
    let agent_result = match scenario.strategy {
        StrategyKind::Chat => {
            let ctx = app::agents::strategy::chat::ChatContext::from_request(
                request,
                trace_id,
                LoopBudget::chat(UserTier::Pro),
                sink,
                tokio_util::sync::CancellationToken::new(),
            )
            .unwrap();
            let strategy = app::agents::strategy::chat::ChatStrategy {
                llm: recording_arc.clone(),
                llm_client: Some(config.llm_client()),
                temperature: None,
                search_provider: None,
            };
            let executor = app::agents::strategy::executor::StrategyExecutor;
            executor.run(&strategy, ctx).await
        }
        StrategyKind::Rag => {
            if let Err(missing) = config.validate_for_rag() {
                result.status = TestStatus::Skipped;
                result.error_message = Some(format!("RAG env missing: {}", missing.join(", ")));
                result.failure_kind = Some(TestFailureKind::DependencyMissing);
                result.duration_ms = start.elapsed().as_millis() as u64;
                return result;
            }

            let components = match build_rag_components(&config).await {
                Some(c) => c,
                None => {
                    result.status = TestStatus::Skipped;
                    result.error_message = Some("Failed to build RAG components".to_string());
                    result.failure_kind = Some(TestFailureKind::SetupFailed);
                    result.duration_ms = start.elapsed().as_millis() as u64;
                    return result;
                }
            };

            let chunks = vec![
                "Antifragility is a property of systems that increase in capability, resilience, or robustness as a result of stressors, shocks, volatility, noise, mistakes, faults, attacks, or failures.",
                "The concept was developed by Nassim Nicholas Taleb, a professor and former trader, and is the central theme of his book Antifragile: Things That Gain from Disorder.",
                "Taleb defines antifragility as the opposite of fragility. While a fragile object breaks under stress, and a robust object resists stress, an antifragile object actually benefits from stress.",
            ];
            let doc_id = match ingest_chunks(
                components.data_plane.as_ref(),
                &components.embedding_client,
                chunks,
            ).await {
                Ok(id) => id,
                Err(e) => {
                    result.error_message = Some(format!("Ingestion failed: {}", e));
                    result.failure_kind = Some(TestFailureKind::SetupFailed);
                    result.duration_ms = start.elapsed().as_millis() as u64;
                    return result;
                }
            };

            let mut rag_request = request;
            rag_request.doc_scope = vec![doc_id.to_string()];
            rag_request.docscope_metadata = Some(common::DocScopeMetadata {
                documents: vec![common::SummaryMetadata {
                    doc_id: doc_id.to_string(),
                    filename: "antifragile.pdf".to_string(),
                    docname: "Antifragile".to_string(),
                    language: "en".to_string(),
                    domain: common::Domain::Business,
                    genre: common::Genre::Book,
                    era: common::Era::Contemporary,
                }],
                profile: common::DocScopeProfile {
                    languages: vec!["en".to_string()],
                    domains: vec![common::Domain::Business],
                    genres: vec![common::Genre::Book],
                    eras: vec![common::Era::Contemporary],
                },
            });

            let ctx = app::agents::strategy::rag::RagContext::from_request(
                rag_request,
                trace_id,
                LoopBudget::new(6),
                sink,
                tokio_util::sync::CancellationToken::new(),
                components.rag_runtime,
            )
            .unwrap();
            let strategy = app::agents::strategy::rag::RagStrategy {
                llm: recording_arc.clone(),
                llm_client: Some(config.llm_client()),
                temperature: None,
            };
            let executor = app::agents::strategy::executor::StrategyExecutor;
            executor.run(&strategy, ctx).await
        }
        StrategyKind::Search => {
            let search_executor: Arc<dyn avrag_search::SearchProvider> = Arc::new(SimpleMockSearchProvider);
            let llm_for_synth: Arc<dyn avrag_llm::LlmProvider> = recording_arc.clone();
            let search_synthesizer: Option<Arc<dyn app::agents::strategy::search::SearchAnswerSynthesizer>> = Some(Arc::new(
                app::agents::strategy::search::LlmSearchAnswerSynthesizer {
                    llm: llm_for_synth,
                    llm_client: Some(config.llm_client()),
                },
            ));

            let ctx = app::agents::strategy::search::SearchContext::from_request(
                request,
                trace_id,
                LoopBudget::search(UserTier::Pro),
                sink,
                tokio_util::sync::CancellationToken::new(),
            )
            .unwrap();
            let strategy = app::agents::strategy::search::SearchStrategy {
                llm: recording_arc.clone(),
                llm_client: Some(config.llm_client()),
                temperature: None,
                search_executor,
                search_synthesizer,
            };
            let executor = app::agents::strategy::executor::StrategyExecutor;
            executor.run(&strategy, ctx).await
        }
    };

    match agent_result {
        Ok(run_result) => {
            result.answer_text = run_result.answer.clone();
            result.llm_calls = recording_arc.calls();

            // Check if HTML output
            if run_result.answer.contains("<html") || run_result.answer.contains("<!DOCTYPE") {
                result.answer_html = Some(run_result.answer.clone());

                // Screenshot
                match screenshot_webpage(&run_result.answer).await {
                    Ok(artifact) => {
                        let screenshot_path = output_dir.join(&test_name).join("screenshot.png");
                        std::fs::create_dir_all(screenshot_path.parent().unwrap()).ok();
                        std::fs::write(&screenshot_path, &artifact.png_bytes).ok();
                        result.screenshot_path = Some(screenshot_path);
                        result.diagnostics = Some(artifact.diagnostics);

                        // Assert HTML markers (non-panicking) — any marker matches
                        let html_lower = run_result.answer.to_lowercase();
                        let has_marker = scenario.expected_markers.iter().any(|m| html_lower.contains(&m.to_lowercase()));
                        if !has_marker {
                            result.error_message = Some(format!(
                                "HTML missing expected markers: {:?}",
                                scenario.expected_markers
                            ));
                            result.failure_kind = Some(TestFailureKind::AssertionFailed);
                            result.duration_ms = start.elapsed().as_millis() as u64;
                            return result;
                        }
                        result.status = TestStatus::Passed;
                    }
                    Err(e) => {
                        result.error_message = Some(format!("Screenshot failed: {:?}", e));
                        result.failure_kind = Some(TestFailureKind::AssertionFailed);
                    }
                }
            } else {
                // Text-only answer: assert markers in text — any marker matches
                let answer_lower = run_result.answer.to_lowercase();
                let has_marker = scenario
                    .expected_markers
                    .iter()
                    .any(|m| answer_lower.contains(&m.to_lowercase()));
                if !has_marker {
                    result.error_message = Some(format!(
                        "Answer missing expected markers: {:?}",
                        scenario.expected_markers
                    ));
                    result.failure_kind = Some(TestFailureKind::AssertionFailed);
                    result.duration_ms = start.elapsed().as_millis() as u64;
                    return result;
                }
                result.status = TestStatus::Passed;
            }
        }
        Err(e) => {
            result.error_message = Some(format!("Agent execution failed: {}", e));
            result.failure_kind = Some(TestFailureKind::ExecutionFailed);
        }
    }

    result.duration_ms = start.elapsed().as_millis() as u64;
    result
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' { c } else { '_' })
        .collect::<String>()
        .replace(' ', "_")
        .to_lowercase()
}

// ---------------------------------------------------------------------------
// RAG helpers (copied from e2e_rag.rs for independence)
// ---------------------------------------------------------------------------

struct RagComponents {
    rag_runtime: Arc<avrag_rag_core::RagRuntime>,
    data_plane: Arc<dyn avrag_retrieval_data_plane::RetrievalDataPlane>,
    embedding_client: Arc<avrag_llm::EmbeddingClient>,
}

async fn build_rag_components(config: &E2EConfig) -> Option<RagComponents> {
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

    // Ensure Milvus collections exist
    if let Err(e) = data_plane.ensure_schema().await {
        eprintln!("[E2E] Failed to ensure Milvus schema: {}", e);
        return None;
    }

    let rag_config =
        avrag_rag_core::RagConfig::new_for_data_plane(embedding_client.clone(), None);
    let rag_runtime = Arc::new(
        avrag_rag_core::RagRuntime::with_data_plane(rag_config, data_plane.clone()),
    );

    Some(RagComponents {
        rag_runtime,
        data_plane,
        embedding_client,
    })
}

async fn ingest_chunks(
    data_plane: &dyn avrag_retrieval_data_plane::RetrievalDataPlane,
    embedding_client: &avrag_llm::EmbeddingClient,
    chunks: Vec<&str>,
) -> anyhow::Result<uuid::Uuid> {
    use avrag_retrieval_data_plane::{DocumentIndexBatch, TextChunkIndexRecord};

    let doc_id = uuid::Uuid::new_v4();
    let org_id = avrag_auth::OrgId::from(uuid::Uuid::parse_str(
        "00000000-0000-0000-0000-000000000001",
    )?);
    let parse_run_id = uuid::Uuid::new_v4();

    let text_refs: Vec<&str> = chunks.iter().copied().collect();
    let vectors = embedding_client.embed(&text_refs).await?;

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

// ---------------------------------------------------------------------------
// Search mock provider
// ---------------------------------------------------------------------------

struct SimpleMockSearchProvider;

#[async_trait::async_trait]
impl avrag_search::SearchProvider for SimpleMockSearchProvider {
    async fn execute_search(
        &self,
        query: &str,
        _vertical: Option<&str>,
    ) -> anyhow::Result<avrag_search::SearchResponse> {
        Ok(avrag_search::SearchResponse {
            query_type: "mock".to_string(),
            sub_queries: vec![query.to_string()],
            results: vec![avrag_search::SearchResult {
                title: "Mock Search Result".to_string(),
                url: "https://example.com".to_string(),
                snippet: format!("Information about: {}", query),
                citation_index: Some(1),
            }],
            synthesized_answer: String::new(),
            llm_usage: None,
        })
    }
}

#[tokio::test]
#[ignore = "requires staging environment (E2E_LLM_*)"]
async fn format_output_golden_scenarios() {
    let run_id = E2EConfig::generate_run_id();
    let output_dir = std::path::PathBuf::from("tests/e2e_output").join(&run_id);
    std::fs::create_dir_all(&output_dir).unwrap();

    let env_snapshot = E2EConfig::environment_snapshot();
    std::fs::write(
        output_dir.join("metadata.json"),
        serde_json::json!({
            "run_id": run_id,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "environment": env_snapshot,
        })
        .to_string(),
    )
    .unwrap();

    let mut all_results = Vec::new();

    for scenario in SCENARIOS {
        let result = run_format_scenario(scenario, &run_id, &output_dir).await;
        result_serializer::save_test_result(
            &output_dir,
            &result,
            result_serializer::ArtifactRetentionPolicy::OnFailure,
        )
        .ok();
        all_results.push(result);
    }

    // Generate report
    let report = result_serializer::generate_markdown_report(&output_dir, &all_results).unwrap();
    std::fs::write(output_dir.join("report.md"), report).unwrap();

    // Final assertions
    let failures: Vec<_> = all_results
        .iter()
        .filter(|r| r.status == result_serializer::TestStatus::Failed)
        .collect();
    assert!(
        failures.is_empty(),
        "{} format output tests failed",
        failures.len()
    );
}
