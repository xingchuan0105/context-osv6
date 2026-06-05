use super::execute::{
    ChannelCandidateBudgets, default_channel_candidate_budgets, graph_final_context_budget,
};
use super::planner;
use super::response;
use super::retrieval;
use super::*;
use crate::context::SessionContext;
use crate::retrieval::ScoredChunk;
use async_trait::async_trait;
use avrag_auth::{AuthContext, OrgId, SubjectKind};
use avrag_retrieval_data_plane::{
    Bm25SearchOutput, Bm25SearchRequest, Bm25SearchTrace, GraphSearchOutput, GraphSearchRequest,
    MultimodalSearchRequest, RelationPathCandidate, TextDenseSearchRequest,
};
use common::{
    BackendTrace, ChatMessage, ChatRequest, Citation, Coverage, ExecutePlanResponse, RagPlan,
    RagPlanItem, RetrievalBundle, RetrievedChunk,
};
use std::sync::Arc;
use tokio::sync::Barrier;
use uuid::Uuid;

fn make_request(query: &str, agent_type: &str) -> ChatRequest {
    ChatRequest {
        query: query.to_string(),
        notebook_id: None,
        session_id: None,
        agent_type: agent_type.to_string(),
        source_type: None,
        source_token: None,
        doc_scope: Vec::new(),
        messages: Vec::new(),
        stream: false,
        language: None,
    format_hint: None,
    }
}

fn test_config() -> RagConfig {
    let embedding = Arc::new(avrag_llm::EmbeddingClient::new(
        avrag_llm::ModelProviderConfig {
            base_url: "http://localhost:9999".to_string(),
            api_key: "test".to_string(),
            model: "test-model".to_string(),
            timeout_ms: 5000,
            api_style: None,
            dimensions: None,
            enable_thinking: None,
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        },
    ));
    // Stage-level unit tests do not need a PostgreSQL repository.
    RagConfig::new_for_data_plane(embedding, None)
}

fn make_session_context() -> SessionContext {
    SessionContext {
        summary: Some("The user is discussing Rust ownership rules.".to_string()),
        messages: vec![
            ChatMessage {
                id: 1,
                session_id: "s1".to_string(),
                role: "user".to_string(),
                content: "Can you explain ownership?".to_string(),
                answer_blocks: Vec::new(),
                agent_id: None,
                agent_name: None,
                agent_icon: None,
                citations: Vec::new(),
                tool_results: Vec::new(),
                created_at: "2026-03-22T00:00:00Z".to_string(),
            },
            ChatMessage {
                id: 2,
                session_id: "s1".to_string(),
                role: "assistant".to_string(),
                content: "Ownership controls who frees memory.".to_string(),
                answer_blocks: Vec::new(),
                agent_id: None,
                agent_name: None,
                agent_icon: None,
                citations: Vec::new(),
                tool_results: Vec::new(),
                created_at: "2026-03-22T00:01:00Z".to_string(),
            },
        ],
    }
}

fn make_scored_chunk(seed: u128, source: &str) -> ScoredChunk {
    ScoredChunk {
        chunk_id: Uuid::from_u128(seed),
        doc_id: Uuid::from_u128(seed + 10_000),
        content: format!("chunk-{seed}"),
        score: 1.0 - (seed as f32 * 0.01),
        source: source.to_string(),
        page: None,
        chunk_type: "text".to_string(),
        asset_id: None,
        caption: None,
        image_path: None,
        parser_backend: None,
        source_locator: None,
        parse_run_id: None,
    }
}

struct StubRetrievalDataPlane;

#[async_trait]
impl RetrievalDataPlane for StubRetrievalDataPlane {
    async fn search_text_dense(
        &self,
        _request: TextDenseSearchRequest,
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        Ok(Vec::new())
    }

    async fn search_bm25(&self, request: Bm25SearchRequest) -> anyhow::Result<Bm25SearchOutput> {
        assert_eq!(request.query, "exact term");
        Ok(Bm25SearchOutput {
            chunks: vec![make_scored_chunk(42, "stub_bm25")],
            trace: Bm25SearchTrace {
                backend: "stub".to_string(),
                raw_hit_count: 1,
                hydrated_hit_count: 1,
                fallback_reason: None,
            },
        })
    }

    async fn search_multimodal(
        &self,
        _request: MultimodalSearchRequest,
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        Ok(Vec::new())
    }
}

struct GraphStubRetrievalDataPlane;

#[async_trait]
impl RetrievalDataPlane for GraphStubRetrievalDataPlane {
    async fn search_text_dense(
        &self,
        _request: TextDenseSearchRequest,
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        Ok(Vec::new())
    }

    async fn search_bm25(&self, request: Bm25SearchRequest) -> anyhow::Result<Bm25SearchOutput> {
        assert_eq!(request.query, "exact term");
        Ok(Bm25SearchOutput {
            chunks: vec![make_scored_chunk(42, "stub_bm25")],
            trace: Bm25SearchTrace {
                backend: "stub".to_string(),
                raw_hit_count: 1,
                hydrated_hit_count: 1,
                fallback_reason: None,
            },
        })
    }

    async fn search_multimodal(
        &self,
        _request: MultimodalSearchRequest,
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        Ok(Vec::new())
    }

    async fn search_graph(&self, request: GraphSearchRequest) -> anyhow::Result<GraphSearchOutput> {
        assert_eq!(
            request.entity_names,
            vec!["Atlas".to_string(), "rollback checklist".to_string()]
        );
        assert_eq!(request.relation_limit, 2);
        Ok(GraphSearchOutput {
            relation_paths: vec![RelationPathCandidate {
                subject: "Atlas".to_string(),
                predicate: "uses".to_string(),
                object: "rollback checklist".to_string(),
                score: 0.91,
                supporting_chunk_ids: vec![Uuid::from_u128(90)],
            }],
            supporting_chunks: vec![ScoredChunk {
                chunk_id: Uuid::from_u128(90),
                doc_id: Uuid::from_u128(10_090),
                content: "Atlas uses the rollback checklist".to_string(),
                score: 0.91,
                source: "stub_graph".to_string(),
                page: Some(3),
                chunk_type: "graph_relation".to_string(),
                asset_id: None,
                caption: None,
                image_path: None,
                parser_backend: None,
                source_locator: None,
                parse_run_id: Some(Uuid::from_u128(91)),
            }],
        })
    }
}

struct PlaceholderTripletGraphDataPlane;

#[async_trait]
impl RetrievalDataPlane for PlaceholderTripletGraphDataPlane {
    async fn search_text_dense(
        &self,
        _request: TextDenseSearchRequest,
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        Ok(Vec::new())
    }

    async fn search_bm25(&self, _request: Bm25SearchRequest) -> anyhow::Result<Bm25SearchOutput> {
        Ok(Bm25SearchOutput {
            chunks: Vec::new(),
            trace: Bm25SearchTrace {
                backend: "stub".to_string(),
                raw_hit_count: 0,
                hydrated_hit_count: 0,
                fallback_reason: None,
            },
        })
    }

    async fn search_multimodal(
        &self,
        _request: MultimodalSearchRequest,
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        Ok(Vec::new())
    }

    async fn search_graph(&self, request: GraphSearchRequest) -> anyhow::Result<GraphSearchOutput> {
        assert_eq!(request.entity_names, vec!["Atlas".to_string()]);
        assert_eq!(request.relation_hints.len(), 2);
        assert_eq!(request.relation_hints[0].subject.as_deref(), Some("Atlas"));
        assert_eq!(request.relation_hints[0].predicate.as_deref(), Some("uses"));
        assert_eq!(request.relation_hints[0].object.as_deref(), None);
        assert_eq!(request.relation_hints[1].subject.as_deref(), None);
        assert_eq!(request.relation_hints[1].predicate.as_deref(), Some("uses"));
        assert_eq!(request.relation_hints[1].object.as_deref(), None);
        Ok(GraphSearchOutput {
            relation_paths: vec![RelationPathCandidate {
                subject: "Atlas".to_string(),
                predicate: "uses".to_string(),
                object: "rollback checklist".to_string(),
                score: 0.9,
                supporting_chunk_ids: vec![Uuid::from_u128(92)],
            }],
            supporting_chunks: vec![make_scored_chunk(92, "stub_graph")],
        })
    }
}

struct BarrierGraphBm25DataPlane {
    barrier: Arc<Barrier>,
}

#[async_trait]
impl RetrievalDataPlane for BarrierGraphBm25DataPlane {
    async fn search_text_dense(
        &self,
        _request: TextDenseSearchRequest,
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        Ok(Vec::new())
    }

    async fn search_bm25(&self, _request: Bm25SearchRequest) -> anyhow::Result<Bm25SearchOutput> {
        self.barrier.wait().await;
        Ok(Bm25SearchOutput {
            chunks: vec![make_scored_chunk(12, "stub_bm25")],
            trace: Bm25SearchTrace {
                backend: "stub".to_string(),
                raw_hit_count: 1,
                hydrated_hit_count: 1,
                fallback_reason: None,
            },
        })
    }

    async fn search_multimodal(
        &self,
        _request: MultimodalSearchRequest,
    ) -> anyhow::Result<Vec<ScoredChunk>> {
        Ok(Vec::new())
    }

    async fn search_graph(
        &self,
        _request: GraphSearchRequest,
    ) -> anyhow::Result<GraphSearchOutput> {
        self.barrier.wait().await;
        Ok(GraphSearchOutput {
            relation_paths: vec![RelationPathCandidate {
                subject: "Atlas".to_string(),
                predicate: "uses".to_string(),
                object: "checklist".to_string(),
                score: 0.9,
                supporting_chunk_ids: vec![Uuid::from_u128(22)],
            }],
            supporting_chunks: vec![make_scored_chunk(22, "stub_graph")],
        })
    }
}

#[tokio::test]
async fn runtime_bm25_stage_uses_injected_data_plane() {
    let runtime = RagRuntime::with_data_plane(test_config(), Arc::new(StubRetrievalDataPlane));
    let request = make_request("fallback query", "rag");
    let auth = AuthContext::new(OrgId::new(Uuid::from_u128(9)), SubjectKind::System);
    let plan = RagPlan {
        plan_version: "rag-item-v2".to_string(),
        plan_confidence: 0.8,
        clarify_needed: false,
        clarify_message: String::new(),
        items: vec![RagPlanItem {
            priority: 1.0,
            query: None,
            bm25_terms: Some(vec!["exact".to_string(), "term".to_string()]),
            summary: None,
        }],
    };

    let (lists, degrade_trace) = runtime
        .retrieve_bm25_stage_with_budget(&request, &auth, &plan, 5)
        .await
        .unwrap();

    assert!(degrade_trace.is_empty());
    assert_eq!(lists.len(), 1);
    assert_eq!(lists[0].chunks[0].source, "stub_bm25");
}

#[test]
fn default_channel_candidate_budgets_follow_expected_weights() {
    assert_eq!(
        default_channel_candidate_budgets(100),
        ChannelCandidateBudgets {
            text_dense: 35,
            bm25: 25,
            multimodal_dense: 15,
            graph: 25,
        }
    );
}

#[test]
fn graph_final_context_budget_reserves_twenty_percent_when_available() {
    assert_eq!(graph_final_context_budget(30, 20), 6);
    assert_eq!(graph_final_context_budget(4, 10), 1);
    assert_eq!(graph_final_context_budget(30, 2), 2);
}

#[tokio::test]
async fn execute_plan_includes_graph_relation_paths_and_supporting_chunks() {
    let runtime = RagRuntime::with_data_plane(test_config(), Arc::new(GraphStubRetrievalDataPlane));
    let auth = AuthContext::new(OrgId::new(Uuid::from_u128(9)), SubjectKind::System);
    let request = common::ExecutePlanRequest {
        plan_version: "rag-execute-v1".to_string(),
        doc_scope: vec![Uuid::from_u128(10_090).to_string()],
        items: vec![common::ExecutePlanItem {
            priority: 1.0,
            query: None,
            bm25_terms: Some(vec!["exact".to_string(), "term".to_string()]),
        }],
        summary_mode: common::ExecutePlanSummaryMode::None,
        budget: Some(common::ExecutePlanBudget {
            total_candidate_budget: Some(8),
            final_chunk_budget: Some(4),
            graph_hop_limit: None,
            graph_fan_out_limit: None,
        }),
        channel_budget: None,
        query_entities: Vec::new(),
        graph_hints: vec![common::GraphHint {
            subject: Some("Atlas".to_string()),
            predicate: Some("uses".to_string()),
            object: Some("rollback checklist".to_string()),
        }],
        placeholder_triplets: Vec::new(),
        trace: None,
    };

    let response = runtime.execute_plan(&request, &auth).await.unwrap();

    assert_eq!(response.bundle.relation_paths.len(), 1);
    assert_eq!(response.bundle.relation_paths[0].relations, vec!["uses"]);
    assert_eq!(response.bundle.graph_supported_chunks.len(), 1);
    assert_eq!(
        response.bundle.graph_supported_chunks[0]
            .parse_run_id
            .as_deref(),
        Some("00000000-0000-0000-0000-00000000005b")
    );
    assert_eq!(response.coverage.channel_coverage.graph, 1);
    assert!(
        response
            .backend_trace
            .channel_trace
            .iter()
            .any(|trace| trace.channel == "graph" && trace.selected_count == 1)
    );
    assert_eq!(response.bundle.citations.len(), 2);
}

#[tokio::test]
async fn execute_plan_starts_bm25_and_graph_channels_in_parallel() {
    let runtime = RagRuntime::with_data_plane(
        test_config(),
        Arc::new(BarrierGraphBm25DataPlane {
            barrier: Arc::new(Barrier::new(2)),
        }),
    );
    let auth = AuthContext::new(OrgId::new(Uuid::from_u128(9)), SubjectKind::System);
    let request = common::ExecutePlanRequest {
        plan_version: "rag-execute-v1".to_string(),
        doc_scope: vec![Uuid::from_u128(10_022).to_string()],
        items: vec![common::ExecutePlanItem {
            priority: 1.0,
            query: None,
            bm25_terms: Some(vec!["exact".to_string(), "term".to_string()]),
        }],
        summary_mode: common::ExecutePlanSummaryMode::None,
        budget: Some(common::ExecutePlanBudget {
            total_candidate_budget: Some(20),
            final_chunk_budget: Some(5),
            graph_hop_limit: None,
            graph_fan_out_limit: None,
        }),
        channel_budget: Some(common::ChannelBudget {
            text_dense: Some(0),
            bm25: Some(5),
            multimodal_dense: Some(0),
            graph: Some(5),
        }),
        query_entities: Vec::new(),
        graph_hints: vec![common::GraphHint {
            subject: Some("Atlas".to_string()),
            predicate: Some("uses".to_string()),
            object: Some("checklist".to_string()),
        }],
        placeholder_triplets: Vec::new(),
        trace: None,
    };

    let response = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        runtime.execute_plan(&request, &auth),
    )
    .await
    .expect("bm25 and graph channels should rendezvous concurrently")
    .unwrap();

    assert_eq!(response.bundle.chunks.len(), 1);
    assert_eq!(response.bundle.graph_supported_chunks.len(), 1);
}

#[tokio::test]
async fn execute_plan_maps_traceable_placeholder_triplets_to_graph_hints() {
    let runtime =
        RagRuntime::with_data_plane(test_config(), Arc::new(PlaceholderTripletGraphDataPlane));
    let auth = AuthContext::new(OrgId::new(Uuid::from_u128(9)), SubjectKind::System);
    let request = common::ExecutePlanRequest {
        plan_version: "rag-execute-v1".to_string(),
        doc_scope: vec![Uuid::from_u128(10_092).to_string()],
        items: vec![common::ExecutePlanItem {
            priority: 1.0,
            query: Some("how does Atlas use the checklist?".to_string()),
            bm25_terms: None,
        }],
        summary_mode: common::ExecutePlanSummaryMode::None,
        budget: Some(common::ExecutePlanBudget {
            total_candidate_budget: Some(8),
            final_chunk_budget: Some(4),
            graph_hop_limit: None,
            graph_fan_out_limit: None,
        }),
        channel_budget: None,
        query_entities: Vec::new(),
        graph_hints: Vec::new(),
        placeholder_triplets: vec![
            common::PlaceholderTriplet {
                subject: "Atlas".to_string(),
                predicate: "uses".to_string(),
                object: "?checklist".to_string(),
            },
            common::PlaceholderTriplet {
                subject: "?system".to_string(),
                predicate: "uses".to_string(),
                object: "?artifact".to_string(),
            },
        ],
        trace: None,
    };

    let response = runtime.execute_plan(&request, &auth).await.unwrap();

    assert_eq!(response.bundle.relation_paths.len(), 1);
    assert_eq!(response.coverage.channel_coverage.graph, 1);
}

#[tokio::test]
async fn plan_without_llm_planner_falls_back_to_default_query_item() {
    let runtime = RagRuntime::with_data_plane(test_config(), Arc::new(StubRetrievalDataPlane));
    let request = make_request("latest AI news", "rag");
    let mut degrade_trace = Vec::new();

    let (plan, planner_usage) = runtime
        .plan(&request, None, None, &mut degrade_trace)
        .await
        .unwrap();

    assert!(degrade_trace.is_empty());
    assert!(planner_usage.is_none());
    assert!(!plan.clarify_needed);
    assert_eq!(plan.items.len(), 1);
    assert_eq!(plan.items[0].query.as_deref(), Some("latest AI news"));
    assert!(plan.items[0].bm25_terms.is_none());
}

#[test]
fn planner_session_context_includes_summary_and_recent_messages() {
    let context = planner::planner_session_context(Some(&make_session_context())).unwrap();
    assert!(context.contains("Conversation summary:"));
    assert!(context.contains("Rust ownership rules"));
    assert!(context.contains("user: Can you explain ownership?"));
    assert!(context.contains("assistant: Ownership controls who frees memory."));
}

#[test]
fn candidate_budget_allocation_sums_to_total_budget() {
    let items = vec![
        RagPlanItem {
            priority: 0.7,
            query: Some("alpha".to_string()),
            bm25_terms: None,
            summary: None,
        },
        RagPlanItem {
            priority: 0.2,
            query: None,
            bm25_terms: Some(vec!["beta".to_string()]),
            summary: None,
        },
        RagPlanItem {
            priority: 0.1,
            query: None,
            bm25_terms: None,
            summary: Some("related".to_string()),
        },
    ];

    let budgets = planner::allocate_item_candidate_budgets(&items);

    assert_eq!(budgets.len(), 3);
    assert_eq!(budgets[2], 0);
    assert_eq!(budgets[0] + budgets[1], TOTAL_CANDIDATE_BUDGET);
    assert!(budgets[0] > budgets[1]);
}

#[test]
fn candidate_budget_allocation_falls_back_to_even_split_for_zero_weights() {
    let items = vec![
        RagPlanItem {
            priority: 0.0,
            query: Some("alpha".to_string()),
            bm25_terms: None,
            summary: None,
        },
        RagPlanItem {
            priority: 0.0,
            query: None,
            bm25_terms: Some(vec!["beta".to_string()]),
            summary: None,
        },
    ];

    let budgets = planner::allocate_item_candidate_budgets(&items);

    assert_eq!(budgets[0] + budgets[1], TOTAL_CANDIDATE_BUDGET);
    assert!((budgets[0] as isize - budgets[1] as isize).abs() <= 1);
}

#[test]
fn final_candidate_pool_interleaves_and_caps_total_budget() {
    let text_pool = vec![
        make_scored_chunk(1, "dense"),
        make_scored_chunk(2, "dense"),
        make_scored_chunk(3, "dense"),
    ];
    let multimodal_pool = vec![
        make_scored_chunk(101, "multimodal_dense"),
        make_scored_chunk(102, "multimodal_dense"),
        make_scored_chunk(103, "multimodal_dense"),
    ];

    let merged = retrieval::build_final_candidate_pool(text_pool, multimodal_pool, 4);

    assert_eq!(merged.len(), 4);
    assert_eq!(merged[0].chunk_id, Uuid::from_u128(1));
    assert_eq!(merged[1].chunk_id, Uuid::from_u128(101));
    assert_eq!(merged[2].chunk_id, Uuid::from_u128(2));
    assert_eq!(merged[3].chunk_id, Uuid::from_u128(102));
}

#[test]
fn multimodal_rerank_documents_preserve_text_and_image_modalities() {
    let mut text_chunk = make_scored_chunk(1, "dense");
    text_chunk.content = "plain text".to_string();

    let mut image_chunk = make_scored_chunk(2, "multimodal_dense");
    image_chunk.image_path = Some("https://example.com/image.png".to_string());
    image_chunk.content = "image context".to_string();

    let documents = retrieval::build_multimodal_rerank_documents(&[text_chunk, image_chunk]);

    match &documents[0] {
        avrag_llm::MultiModalRerankDocument::Text(text) => assert_eq!(text, "plain text"),
        _ => panic!("expected text rerank document"),
    }
    match &documents[1] {
        avrag_llm::MultiModalRerankDocument::Image(path) => {
            assert_eq!(path, "https://example.com/image.png")
        }
        _ => panic!("expected image rerank document"),
    }
}

#[test]
fn normalize_plan_prefers_query_and_fills_empty_payloads() {
    let mut plan = RagPlan {
        plan_version: "rag-item-v2".to_string(),
        plan_confidence: 0.6,
        clarify_needed: false,
        clarify_message: String::new(),
        items: vec![
            RagPlanItem {
                priority: 1.2,
                query: Some("semantic query".to_string()),
                bm25_terms: Some(vec!["exact".to_string()]),
                summary: Some("related".to_string()),
            },
            RagPlanItem {
                priority: -0.5,
                query: None,
                bm25_terms: None,
                summary: None,
            },
        ],
    };

    planner::normalize_rag_plan(&mut plan, "fallback query");

    assert_eq!(plan.items[0].query.as_deref(), Some("semantic query"));
    assert!(plan.items[0].bm25_terms.is_none());
    assert!(plan.items[0].summary.is_none());
    assert_eq!(plan.items[0].priority, 1.0);

    assert_eq!(plan.items[1].query.as_deref(), Some("fallback query"));
    assert!(plan.items[1].bm25_terms.is_none());
    assert!(plan.items[1].summary.is_none());
    assert_eq!(plan.items[1].priority, 0.0);
}

#[test]
fn normalize_plan_drops_invalid_summary_payloads() {
    let mut plan = RagPlan {
        plan_version: "rag-item-v2".to_string(),
        plan_confidence: 0.7,
        clarify_needed: false,
        clarify_message: String::new(),
        items: vec![
            RagPlanItem {
                priority: 0.9,
                query: Some("semantic query".to_string()),
                bm25_terms: None,
                summary: Some("global".to_string()),
            },
            RagPlanItem {
                priority: 0.3,
                query: None,
                bm25_terms: None,
                summary: Some("whatever".to_string()),
            },
        ],
    };

    planner::normalize_rag_plan(&mut plan, "fallback query");

    assert_eq!(plan.items[0].query.as_deref(), Some("semantic query"));
    assert!(plan.items[0].summary.is_none());
    assert_eq!(planner::item_payload_kind(&plan.items[0]), "query");

    assert_eq!(plan.items[1].query.as_deref(), Some("fallback query"));
    assert!(plan.items[1].summary.is_none());
    assert_eq!(planner::item_payload_kind(&plan.items[1]), "query");
}

#[test]
fn build_item_trace_reflects_query_bm25_and_summary_payloads() {
    let request = make_request("fallback query", "rag");
    let plan = RagPlan {
        plan_version: "rag-item-v2".to_string(),
        plan_confidence: 0.9,
        clarify_needed: false,
        clarify_message: String::new(),
        items: vec![
            RagPlanItem {
                priority: 0.7,
                query: Some("semantic query".to_string()),
                bm25_terms: None,
                summary: None,
            },
            RagPlanItem {
                priority: 0.2,
                query: None,
                bm25_terms: Some(vec!["atlas".to_string(), "rollback".to_string()]),
                summary: None,
            },
            RagPlanItem {
                priority: 0.1,
                query: None,
                bm25_terms: None,
                summary: Some("related".to_string()),
            },
        ],
    };

    let trace = planner::build_item_trace(&request, &plan);

    assert_eq!(trace.len(), 3);
    assert_eq!(trace[0].payload_kind, "query");
    assert_eq!(trace[0].query.as_deref(), Some("semantic query"));
    assert!(trace[0].dense_k > 0);

    assert_eq!(trace[1].payload_kind, "bm25_terms");
    assert_eq!(
        trace[1].bm25_terms,
        vec!["atlas".to_string(), "rollback".to_string()]
    );
    assert!(trace[1].bm25_k > 0);

    assert_eq!(trace[2].payload_kind, "summary");
    assert_eq!(trace[2].summary.as_deref(), Some("related"));
    assert_eq!(trace[2].recall_budget, 0);
    assert_eq!(trace[2].bm25_k, 0);
    assert_eq!(trace[2].dense_k, 0);
}

#[test]
fn build_answer_context_chunks_puts_retrieval_before_summary_chunks() {
    let runtime = RagRuntime::with_data_plane(test_config(), Arc::new(StubRetrievalDataPlane));
    let retrieval_chunks = vec![make_scored_chunk(1, "dense"), make_scored_chunk(2, "bm25")];
    let summary_chunks = vec![(Uuid::from_u128(9000), "summary context".to_string())];

    let context_chunks = runtime.build_answer_context_chunks(&summary_chunks, &retrieval_chunks);

    assert!(context_chunks.len() >= 3);
    assert_ne!(context_chunks[0].chunk_type, "summary");
    assert_ne!(context_chunks[1].chunk_type, "summary");
    assert_eq!(
        context_chunks.last().map(|item| item.chunk_type.as_str()),
        Some("summary")
    );
}

#[test]
fn cut_final_candidates_backfills_to_minimum_when_threshold_matches_too_few() {
    let runtime = RagRuntime::with_data_plane(test_config(), Arc::new(StubRetrievalDataPlane));
    let chunks = (0..35)
        .map(|i| {
            let mut chunk = make_scored_chunk(i as u128 + 1, "dense");
            chunk.score = match i {
                0 => 0.95,
                1 => 0.82,
                _ => 0.69 - (i as f32 * 0.001),
            };
            chunk
        })
        .collect::<Vec<_>>();

    let kept = runtime.cut_final_candidates_stage(chunks);

    assert_eq!(kept.len(), FINAL_MIN_CHUNKS);
    assert!(kept[0].score >= FINAL_SCORE_THRESHOLD);
    assert!(kept[1].score >= FINAL_SCORE_THRESHOLD);
}

#[test]
fn cut_final_candidates_keeps_all_chunks_above_threshold_even_past_minimum() {
    let runtime = RagRuntime::with_data_plane(test_config(), Arc::new(StubRetrievalDataPlane));
    let chunks = (0..32)
        .map(|i| {
            let mut chunk = make_scored_chunk(i as u128 + 100, "dense");
            chunk.score = 0.95 - (i as f32 * 0.001);
            chunk
        })
        .collect::<Vec<_>>();

    let kept = runtime.cut_final_candidates_stage(chunks);

    assert_eq!(kept.len(), 32);
    assert!(
        kept.iter()
            .all(|chunk| chunk.score >= FINAL_SCORE_THRESHOLD)
    );
}

#[test]
fn materialize_answer_markup_converts_chunk_placeholders_to_citation_tokens() {
    let citations = vec![
        Citation {
            citation_id: 1,
            doc_id: "doc-1".to_string(),
            chunk_id: Some("chunk-a".to_string()),
            page: Some(1),
            doc_name: "Atlas".to_string(),
            preview: None,
            content: None,
            score: 0.9,
            layer: Some("dense".to_string()),
            chunk_type: Some("text".to_string()),
            asset_id: None,
            caption: None,
            image_url: None,
            parser_backend: None,
            source_locator: None,
            parse_run_id: None,
        },
        Citation {
            citation_id: 2,
            doc_id: "doc-1".to_string(),
            chunk_id: Some("chunk-img".to_string()),
            page: Some(2),
            doc_name: "Atlas".to_string(),
            preview: None,
            content: None,
            score: 0.8,
            layer: Some("multimodal_dense".to_string()),
            chunk_type: Some("image_with_context".to_string()),
            asset_id: Some("asset-1".to_string()),
            caption: Some("figure".to_string()),
            image_url: Some("https://example.com/figure.png".to_string()),
            parser_backend: None,
            source_locator: None,
            parse_run_id: None,
        },
    ];

    let rendered = response::materialize_answer_markup(
        "结论 [[cite:chunk-a]]\n[[image:chunk-img]]",
        &citations,
    );

    assert_eq!(rendered, "结论 [[1]]\n[[image:2]]");
}

#[test]
fn extract_referenced_chunk_ids_reads_citation_and_image_tokens() {
    let ids = response::extract_referenced_chunk_ids("A [[cite:chunk-a]]\n[[image:chunk-img]]");

    assert!(ids.contains("chunk-a"));
    assert!(ids.contains("chunk-img"));
    assert_eq!(ids.len(), 2);
}

#[test]
fn ensure_inline_image_placeholder_appends_first_image_when_missing() {
    let citations = vec![Citation {
        citation_id: 2,
        doc_id: "doc-1".to_string(),
        chunk_id: Some("chunk-img".to_string()),
        page: Some(2),
        doc_name: "Atlas".to_string(),
        preview: None,
        content: None,
        score: 0.8,
        layer: Some("multimodal_dense".to_string()),
        chunk_type: Some("image_with_context".to_string()),
        asset_id: Some("asset-1".to_string()),
        caption: Some("figure".to_string()),
        image_url: Some("https://example.com/figure.png".to_string()),
        parser_backend: None,
        source_locator: None,
        parse_run_id: None,
    }];

    let answer = response::ensure_inline_image_placeholder("正文回答 [[cite:chunk-a]]", &citations);

    assert!(answer.ends_with("[[image:chunk-img]]"));
}

#[tokio::test]
async fn build_rag_chat_response_from_bundle_reuses_bundle_citations() {
    let runtime = RagRuntime::with_data_plane(test_config(), Arc::new(StubRetrievalDataPlane));
    let request = make_request("Summarize the finding", "rag");
    let rag_plan = RagPlan {
        plan_version: "rag-item-v2".to_string(),
        plan_confidence: 1.0,
        clarify_needed: false,
        clarify_message: String::new(),
        items: vec![RagPlanItem {
            priority: 1.0,
            query: Some("Summarize the finding".to_string()),
            bm25_terms: None,
            summary: None,
        }],
    };
    let execute_response = ExecutePlanResponse {
        bundle: RetrievalBundle {
            chunks: vec![RetrievedChunk {
                chunk_id: "chunk-a".to_string(),
                doc_id: "doc-1".to_string(),
                chunk_type: "text".to_string(),
                page: Some(1),
                text: "Atlas rollback checklist".to_string(),
                score: 0.9,
                retrieval_channel: "dense".to_string(),
                asset_id: None,
                caption: None,
                image_url: None,
                parser_backend: None,
                source_locator: None,
                parse_run_id: None,
                score_breakdown: Vec::new(),
            }],
            graph_supported_chunks: Vec::new(),
            relation_paths: Vec::new(),
            citations: vec![Citation {
                citation_id: 1,
                doc_id: "doc-1".to_string(),
                chunk_id: Some("chunk-a".to_string()),
                page: Some(1),
                doc_name: "Atlas".to_string(),
                preview: Some("Atlas rollback checklist".to_string()),
                content: Some("Atlas rollback checklist".to_string()),
                score: 0.9,
                layer: Some("dense".to_string()),
                chunk_type: Some("text".to_string()),
                asset_id: None,
                caption: None,
                image_url: None,
                parser_backend: None,
                source_locator: None,
                parse_run_id: None,
            }],
            summary_chunks: Vec::new(),
        },
        coverage: Coverage {
            requested_doc_count: 1,
            matched_doc_count: 1,
            retrieved_chunk_count: 1,
            summary_chunk_count: 0,
            channel_coverage: Default::default(),
        },
        degrade_trace: Vec::new(),
        backend_trace: BackendTrace {
            trace: None,
            item_trace: Vec::new(),
            channel_trace: Vec::new(),
            retrieval_trace: common::RagTraceSummary {
                item_count: 0,
                total_candidate_budget: TOTAL_CANDIDATE_BUDGET,
                max_rerank_docs: FINAL_RERANK_BUDGET,
                max_final_chunks: FINAL_MIN_CHUNKS,
                top_k_returned: 1,
                summary_mode: "none".to_string(),
                items: Vec::new(),
            },
        },
    };

    let response = runtime
        .build_rag_chat_response_from_bundle(
            &request,
            Some("session-1"),
            &rag_plan,
            &execute_response,
            avrag_llm::SynthesisOutput {
                answer_text: "结论 [[cite:chunk-a]]".to_string(),
                answer_blocks: Vec::new(),
                cited_chunk_ids: vec!["chunk-a".to_string()],
                llm_usage: None,
            },
            Vec::new(),
        )
        .await
        .unwrap();

    assert_eq!(response.citations.len(), 1);
    assert_eq!(response.citations[0].doc_name, "Atlas");
    assert_eq!(response.answer, "结论 [[1]]");
}

#[tokio::test]
async fn build_rag_chat_response_from_bundle_graph_only_non_empty_citations_and_sources() {
    // Setup: graph-only retrieval bundle with no regular chunks
    let config = test_config();
    let runtime = RagRuntime::with_data_plane(config, Arc::new(StubRetrievalDataPlane));
    let request = make_request("query", "main");

    let rag_plan = RagPlan {
        plan_version: "rag-item-v2".to_string(),
        plan_confidence: 1.0,
        clarify_needed: false,
        clarify_message: String::new(),
        items: vec![RagPlanItem {
            priority: 1.0,
            query: Some("query".to_string()),
            bm25_terms: None,
            summary: None,
        }],
    };

    let execute_response = ExecutePlanResponse {
        bundle: RetrievalBundle {
            chunks: Vec::new(),
            graph_supported_chunks: vec![RetrievedChunk {
                chunk_id: "graph-chunk-1".to_string(),
                doc_id: "doc-1".to_string(),
                chunk_type: "graph_relation".to_string(),
                page: None,
                text: "Atlas uses checklist".to_string(),
                score: 0.8,
                retrieval_channel: "graph".to_string(),
                asset_id: None,
                caption: None,
                image_url: None,
                parser_backend: None,
                source_locator: None,
                parse_run_id: None,
                score_breakdown: Vec::new(),
            }],
            relation_paths: Vec::new(),
            citations: vec![Citation {
                citation_id: 1,
                doc_id: "doc-1".to_string(),
                chunk_id: Some("graph-chunk-1".to_string()),
                page: None,
                doc_name: "Doc 1".to_string(),
                preview: Some("Atlas uses checklist".to_string()),
                content: Some("Atlas uses checklist".to_string()),
                score: 0.8,
                layer: Some("graph".to_string()),
                chunk_type: Some("graph_relation".to_string()),
                asset_id: None,
                caption: None,
                image_url: None,
                parser_backend: None,
                source_locator: None,
                parse_run_id: None,
            }],
            summary_chunks: Vec::new(),
        },
        coverage: Coverage {
            requested_doc_count: 1,
            matched_doc_count: 1,
            retrieved_chunk_count: 0,
            summary_chunk_count: 0,
            channel_coverage: Default::default(),
        },
        degrade_trace: Vec::new(),
        backend_trace: BackendTrace {
            trace: None,
            item_trace: Vec::new(),
            channel_trace: Vec::new(),
            retrieval_trace: common::RagTraceSummary {
                item_count: 0,
                total_candidate_budget: TOTAL_CANDIDATE_BUDGET,
                max_rerank_docs: FINAL_RERANK_BUDGET,
                max_final_chunks: FINAL_MIN_CHUNKS,
                top_k_returned: 1,
                summary_mode: "none".to_string(),
                items: Vec::new(),
            },
        },
    };

    let response = runtime
        .build_rag_chat_response_from_bundle(
            &request,
            Some("session-1"),
            &rag_plan,
            &execute_response,
            avrag_llm::SynthesisOutput {
                answer_text: "结论 [[cite:graph-chunk-1]]".to_string(),
                answer_blocks: Vec::new(),
                cited_chunk_ids: vec!["graph-chunk-1".to_string()],
                llm_usage: None,
            },
            Vec::new(),
        )
        .await
        .unwrap();

    // Must return non-empty citations and sources
    assert!(
        !response.citations.is_empty(),
        "graph-only response must have non-empty citations"
    );
    assert!(
        !response.sources.is_empty(),
        "graph-only response must have non-empty sources"
    );
    assert_eq!(
        response.citations[0].chunk_id,
        Some("graph-chunk-1".to_string())
    );
    // Graph chunk citation markup must render to normal citation number
    assert_eq!(response.answer, "结论 [[1]]");
}
