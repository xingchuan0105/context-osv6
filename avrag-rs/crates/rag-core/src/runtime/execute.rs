use std::{
    collections::{BTreeSet, HashMap, HashSet},
    time::{Duration, Instant},
};

use anyhow::{Result, anyhow};
use avrag_auth::AuthContext;
use avrag_retrieval_data_plane::{
    GraphRelationHint, GraphSearchOutput, GraphSearchRequest, WeightedChunkList,
};
use contracts::chat::{DegradeReason, DegradeTraceItem};
use contracts::{
    BackendTrace, ChannelCoverage, ChannelTraceItem, Coverage, ExecutePlanRequest,
    ExecutePlanResponse, PlaceholderTriplet, RelationPath, RetrievalBundle, RetrievedChunk,
};
use sha2::{Digest, Sha256};

const RETRIEVAL_CACHE_TTL_SECS: u64 = 30 * 60; // 30 minutes

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

fn retrieval_cache_key(request: &ExecutePlanRequest) -> String {
    let json = serde_json::to_string(request).unwrap_or_default();
    format!("rag:execute:{}", sha256_hex(&json))
}

use crate::retrieval::ScoredChunk;

use super::planner::{build_item_trace_with_total, rag_summary_mode, request_doc_ids};
use super::{FINAL_MIN_CHUNKS, FINAL_RERANK_BUDGET, RagRuntime, TOTAL_CANDIDATE_BUDGET};

const CHANNEL_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ChannelCandidateBudgets {
    pub(super) text_dense: usize,
    pub(super) bm25: usize,
    pub(super) multimodal_dense: usize,
    pub(super) graph: usize,
}

struct TextDenseChannelOutput {
    lists: Vec<WeightedChunkList>,
    degrade_trace: Vec<DegradeTraceItem>,
    trace: ChannelTraceItem,
}

struct Bm25ChannelOutput {
    lists: Vec<WeightedChunkList>,
    degrade_trace: Vec<DegradeTraceItem>,
    trace: ChannelTraceItem,
}

struct MultimodalChannelOutput {
    chunks: Vec<ScoredChunk>,
    degrade_trace: Vec<DegradeTraceItem>,
    trace: ChannelTraceItem,
}

struct GraphChannelOutput {
    relation_paths: Vec<RelationPath>,
    supporting_chunks: Vec<ScoredChunk>,
    degrade_trace: Vec<DegradeTraceItem>,
    trace: ChannelTraceItem,
}

fn channel_candidate_budgets(
    request: &ExecutePlanRequest,
    total_candidate_budget: usize,
) -> ChannelCandidateBudgets {
    let defaults = default_channel_candidate_budgets(total_candidate_budget);
    let Some(explicit) = request.channel_budget.as_ref() else {
        return defaults;
    };
    ChannelCandidateBudgets {
        text_dense: explicit.text_dense.unwrap_or(defaults.text_dense),
        bm25: explicit.bm25.unwrap_or(defaults.bm25),
        multimodal_dense: explicit
            .multimodal_dense
            .unwrap_or(defaults.multimodal_dense),
        graph: explicit.graph.unwrap_or(defaults.graph),
    }
}

pub(super) fn default_channel_candidate_budgets(
    total_candidate_budget: usize,
) -> ChannelCandidateBudgets {
    let weights = [35usize, 25, 15, 25];
    let total_weight = weights.iter().sum::<usize>();
    let mut budgets = weights
        .iter()
        .map(|weight| (total_candidate_budget * *weight) / total_weight)
        .collect::<Vec<_>>();
    let assigned = budgets.iter().sum::<usize>();
    let mut remainders = weights
        .iter()
        .enumerate()
        .map(|(index, weight)| {
            (
                index,
                (total_candidate_budget * *weight) % total_weight,
                *weight,
            )
        })
        .collect::<Vec<_>>();
    remainders.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.2.cmp(&left.2))
            .then_with(|| left.0.cmp(&right.0))
    });
    for (index, _, _) in remainders
        .into_iter()
        .take(total_candidate_budget.saturating_sub(assigned))
    {
        budgets[index] += 1;
    }

    ChannelCandidateBudgets {
        text_dense: budgets[0],
        bm25: budgets[1],
        multimodal_dense: budgets[2],
        graph: budgets[3],
    }
}

pub(super) fn graph_final_context_budget(
    final_chunk_budget: usize,
    graph_chunk_count: usize,
) -> usize {
    if final_chunk_budget == 0 || graph_chunk_count == 0 {
        return 0;
    }
    final_chunk_budget.div_ceil(5).max(1).min(graph_chunk_count)
}

fn filter_retrieved_chunks_for_context(
    retrieved_chunks: &[ScoredChunk],
    context_chunks: &[contracts::AnswerContextChunk],
) -> Vec<ScoredChunk> {
    let allowed_chunk_ids = context_chunks
        .iter()
        .filter(|chunk| chunk.chunk_type != "summary")
        .map(|chunk| chunk.chunk_id.clone())
        .collect::<HashSet<_>>();

    retrieved_chunks
        .iter()
        .filter(|chunk| allowed_chunk_ids.contains(&chunk.chunk_id.to_string()))
        .cloned()
        .collect()
}

fn retrieved_chunk_from_scored(chunk: &ScoredChunk) -> RetrievedChunk {
    RetrievedChunk {
        chunk_id: chunk.chunk_id.to_string(),
        doc_id: chunk.doc_id.to_string(),
        chunk_type: chunk.chunk_type.clone(),
        page: chunk.page,
        text: chunk.content.clone(),
        score: chunk.score,
        retrieval_channel: chunk.source.clone(),
        asset_id: chunk.asset_id.map(|asset_id| asset_id.to_string()),
        caption: chunk.caption.clone(),
        image_url: chunk.image_path.clone(),
        parser_backend: chunk.parser_backend.clone(),
        source_locator: chunk.source_locator.clone(),
        parse_run_id: chunk
            .parse_run_id
            .map(|parse_run_id| parse_run_id.to_string()),
        score_breakdown: Vec::new(),
    }
}

fn relation_path_from_candidate(
    index: usize,
    candidate: &avrag_retrieval_data_plane::RelationPathCandidate,
) -> RelationPath {
    RelationPath {
        path_id: format!("graph-path-{}", index + 1),
        entities: vec![candidate.subject.clone(), candidate.object.clone()],
        relations: vec![candidate.predicate.clone()],
        supporting_chunk_ids: candidate
            .supporting_chunk_ids
            .iter()
            .map(ToString::to_string)
            .collect(),
        score: candidate.score,
    }
}

fn citation_from_scored(
    index: usize,
    chunk: &ScoredChunk,
    doc_names: &HashMap<uuid::Uuid, String>,
) -> contracts::chat::Citation {
    contracts::chat::Citation {
        citation_id: (index + 1) as i64,
        doc_id: chunk.doc_id.to_string(),
        chunk_id: Some(chunk.chunk_id.to_string()),
        page: chunk.page.map(|page| page as usize),
        doc_name: doc_names
            .get(&chunk.doc_id)
            .cloned()
            .unwrap_or_else(|| format!("Document {}", chunk.doc_id)),
        preview: Some(chunk.content.chars().take(100).collect()),
        content: Some(chunk.content.clone()),
        score: chunk.score,
        layer: Some(chunk.source.clone()),
        chunk_type: Some(chunk.chunk_type.clone()),
        asset_id: chunk.asset_id.map(|asset_id| asset_id.to_string()),
        caption: chunk.caption.clone(),
        image_url: chunk.image_path.clone(),
        parser_backend: chunk.parser_backend.clone(),
        source_locator: chunk.source_locator.clone(),
        parse_run_id: chunk
            .parse_run_id
            .map(|parse_run_id| parse_run_id.to_string()),
    }
}

fn graph_relation_hints(request: &ExecutePlanRequest) -> Vec<GraphRelationHint> {
    let mut hints = request
        .graph_hints
        .iter()
        .filter_map(|hint| {
            let subject = hint
                .subject
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            let predicate = hint
                .predicate
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            let object = hint
                .object
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            (subject.is_some() || predicate.is_some() || object.is_some()).then_some(
                GraphRelationHint {
                    subject,
                    predicate,
                    object,
                },
            )
        })
        .collect::<Vec<_>>();
    hints.extend(
        request
            .placeholder_triplets
            .iter()
            .filter_map(placeholder_triplet_relation_hint),
    );
    hints
}

fn placeholder_triplet_relation_hint(triplet: &PlaceholderTriplet) -> Option<GraphRelationHint> {
    let subject = normalize_triplet_slot(&triplet.subject);
    let predicate = normalize_triplet_slot(&triplet.predicate);
    let object = normalize_triplet_slot(&triplet.object);
    let placeholder_count = usize::from(subject.is_none())
        + usize::from(predicate.is_none())
        + usize::from(object.is_none());

    (placeholder_count <= 2 && (subject.is_some() || predicate.is_some() || object.is_some()))
        .then_some(GraphRelationHint {
            subject,
            predicate,
            object,
        })
}

fn normalize_triplet_slot(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.contains('?') {
        None
    } else {
        Some(value.to_string())
    }
}

fn dedupe_entity_names(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .filter_map(|value| {
            let key = value.to_lowercase();
            seen.insert(key).then_some(value)
        })
        .collect()
}

fn channel_coverage(
    chunks: &[ScoredChunk],
    graph_supported_chunks: &[ScoredChunk],
) -> ChannelCoverage {
    let mut coverage = ChannelCoverage {
        graph: graph_supported_chunks.len(),
        ..Default::default()
    };
    for chunk in chunks {
        if chunk.source.contains("bm25") || chunk.source.contains("sparse") {
            coverage.bm25 += 1;
        } else if chunk.source.contains("multimodal") {
            coverage.multimodal_dense += 1;
        } else {
            coverage.text_dense += 1;
        }
    }
    coverage
}

fn weighted_chunk_count(lists: &[WeightedChunkList]) -> usize {
    lists.iter().map(|list| list.chunks.len()).sum()
}

fn channel_degrade_reason(degrade_trace: &[DegradeTraceItem]) -> Option<String> {
    (!degrade_trace.is_empty()).then(|| {
        degrade_trace
            .iter()
            .map(|item| item.reason.as_str())
            .collect::<Vec<_>>()
            .join("; ")
    })
}

fn channel_trace(
    channel: &str,
    count: usize,
    latency: Duration,
    degrade_trace: &[DegradeTraceItem],
) -> ChannelTraceItem {
    ChannelTraceItem {
        channel: channel.to_string(),
        raw_count: count,
        hydrated_count: count,
        selected_count: count,
        latency_ms: Some(latency.as_millis() as u64),
        degrade_reason: channel_degrade_reason(degrade_trace),
    }
}

fn timeout_degrade(stage: &str) -> DegradeTraceItem {
    DegradeTraceItem {
        stage: stage.to_string(),
        reason: DegradeReason::ChannelTimeout,
        impact: format!("Skipping {stage} retrieval channel"),
    }
}

impl RagRuntime {
    async fn run_text_dense_channel(
        &self,
        request: &contracts::chat::ChatRequest,
        auth: &AuthContext,
        rag_plan: &contracts::chat::RagPlan,
        budget: usize,
    ) -> TextDenseChannelOutput {
        let started = Instant::now();
        let result = tokio::time::timeout(
            CHANNEL_TIMEOUT,
            self.retrieve_text_dense_stage_with_budget(request, auth, rag_plan, budget),
        )
        .await;

        match result {
            Ok(Ok((lists, degrade_trace))) => {
                let count = weighted_chunk_count(&lists);
                TextDenseChannelOutput {
                    lists,
                    trace: channel_trace("text_dense", count, started.elapsed(), &degrade_trace),
                    degrade_trace,
                }
            }
            Ok(Err(error)) => {
                let degrade_trace = vec![DegradeTraceItem {
                    stage: "text_dense".to_string(),
                    reason: DegradeReason::Other(format!("Text dense channel failed: {error}")),
                    impact: "Skipping text dense retrieval channel".to_string(),
                }];
                TextDenseChannelOutput {
                    lists: Vec::new(),
                    trace: channel_trace("text_dense", 0, started.elapsed(), &degrade_trace),
                    degrade_trace,
                }
            }
            Err(_) => {
                let degrade_trace = vec![timeout_degrade("text_dense")];
                TextDenseChannelOutput {
                    lists: Vec::new(),
                    trace: channel_trace("text_dense", 0, started.elapsed(), &degrade_trace),
                    degrade_trace,
                }
            }
        }
    }

    async fn run_bm25_channel(
        &self,
        request: &contracts::chat::ChatRequest,
        auth: &AuthContext,
        rag_plan: &contracts::chat::RagPlan,
        budget: usize,
    ) -> Bm25ChannelOutput {
        let started = Instant::now();
        let result = tokio::time::timeout(
            CHANNEL_TIMEOUT,
            self.retrieve_bm25_stage_with_budget(request, auth, rag_plan, budget),
        )
        .await;

        match result {
            Ok(Ok((lists, degrade_trace))) => {
                let count = weighted_chunk_count(&lists);
                Bm25ChannelOutput {
                    lists,
                    trace: channel_trace("bm25", count, started.elapsed(), &degrade_trace),
                    degrade_trace,
                }
            }
            Ok(Err(error)) => {
                let degrade_trace = vec![DegradeTraceItem {
                    stage: "bm25".to_string(),
                    reason: DegradeReason::Other(format!("BM25 channel failed: {error}")),
                    impact: "Skipping BM25 retrieval channel".to_string(),
                }];
                Bm25ChannelOutput {
                    lists: Vec::new(),
                    trace: channel_trace("bm25", 0, started.elapsed(), &degrade_trace),
                    degrade_trace,
                }
            }
            Err(_) => {
                let degrade_trace = vec![timeout_degrade("bm25")];
                Bm25ChannelOutput {
                    lists: Vec::new(),
                    trace: channel_trace("bm25", 0, started.elapsed(), &degrade_trace),
                    degrade_trace,
                }
            }
        }
    }

    async fn run_multimodal_channel(
        &self,
        request: &contracts::chat::ChatRequest,
        auth: &AuthContext,
        rag_plan: &contracts::chat::RagPlan,
        budget: usize,
    ) -> MultimodalChannelOutput {
        let started = Instant::now();
        let result = tokio::time::timeout(
            CHANNEL_TIMEOUT,
            self.retrieve_multimodal_dense_stage_with_budget(request, auth, rag_plan, budget),
        )
        .await;

        match result {
            Ok(Ok((chunks, degrade_trace))) => {
                let count = chunks.len();
                MultimodalChannelOutput {
                    chunks,
                    trace: channel_trace(
                        "multimodal_dense",
                        count,
                        started.elapsed(),
                        &degrade_trace,
                    ),
                    degrade_trace,
                }
            }
            Ok(Err(error)) => {
                let degrade_trace = vec![DegradeTraceItem {
                    stage: "multimodal_dense".to_string(),
                    reason: DegradeReason::Other(format!(
                        "Multimodal dense channel failed: {error}"
                    )),
                    impact: "Skipping multimodal dense retrieval channel".to_string(),
                }];
                MultimodalChannelOutput {
                    chunks: Vec::new(),
                    trace: channel_trace("multimodal_dense", 0, started.elapsed(), &degrade_trace),
                    degrade_trace,
                }
            }
            Err(_) => {
                let degrade_trace = vec![timeout_degrade("multimodal_dense")];
                MultimodalChannelOutput {
                    chunks: Vec::new(),
                    trace: channel_trace("multimodal_dense", 0, started.elapsed(), &degrade_trace),
                    degrade_trace,
                }
            }
        }
    }

    async fn run_graph_channel(
        &self,
        request: &ExecutePlanRequest,
        auth: &AuthContext,
        relation_limit: usize,
        supporting_chunk_limit: usize,
    ) -> GraphChannelOutput {
        let started = Instant::now();
        let result = tokio::time::timeout(
            CHANNEL_TIMEOUT,
            self.retrieve_graph_stage(request, auth, relation_limit, supporting_chunk_limit),
        )
        .await;

        match result {
            Ok(output) => output,
            Err(_) => {
                let degrade_trace = vec![timeout_degrade("graph")];
                GraphChannelOutput {
                    relation_paths: Vec::new(),
                    supporting_chunks: Vec::new(),
                    trace: channel_trace("graph", 0, started.elapsed(), &degrade_trace),
                    degrade_trace,
                }
            }
        }
    }

    async fn retrieve_graph_stage(
        &self,
        request: &ExecutePlanRequest,
        auth: &AuthContext,
        relation_limit: usize,
        supporting_chunk_limit: usize,
    ) -> GraphChannelOutput {
        let started = Instant::now();
        let mut degrade_trace = Vec::new();
        let relation_hints = graph_relation_hints(request);
        let entity_names = dedupe_entity_names(relation_hints.iter().flat_map(|hint| {
            [hint.subject.clone(), hint.object.clone()]
                .into_iter()
                .flatten()
        }));

        if entity_names.is_empty() && relation_hints.is_empty() {
            if relation_limit > 0 && supporting_chunk_limit > 0 {
                degrade_trace.push(DegradeTraceItem {
                    stage: "graph".to_string(),
                    reason: DegradeReason::ChannelFailed,
                    impact: "Skipping graph retrieval without structured triplets".to_string(),
                });
            }
            let trace = ChannelTraceItem {
                channel: "graph".to_string(),
                raw_count: 0,
                hydrated_count: 0,
                selected_count: 0,
                latency_ms: Some(started.elapsed().as_millis() as u64),
                degrade_reason: channel_degrade_reason(&degrade_trace),
            };
            return GraphChannelOutput {
                relation_paths: Vec::new(),
                supporting_chunks: Vec::new(),
                degrade_trace,
                trace,
            };
        }

        if relation_limit == 0 || supporting_chunk_limit == 0 {
            let trace = ChannelTraceItem {
                channel: "graph".to_string(),
                raw_count: 0,
                hydrated_count: 0,
                selected_count: 0,
                latency_ms: Some(started.elapsed().as_millis() as u64),
                degrade_reason: None,
            };
            return GraphChannelOutput {
                relation_paths: Vec::new(),
                supporting_chunks: Vec::new(),
                degrade_trace,
                trace,
            };
        }

        let doc_ids = request_doc_ids(&request.to_chat_request_compat());
        match self
            .data_plane
            .search_graph(GraphSearchRequest {
                auth: auth.clone(),
                doc_ids,
                entity_names,
                relation_hints,
                relation_limit,
                supporting_chunk_limit,
                query_entities: Vec::new(),
                query_entity_vectors: Vec::new(),
                hop_limit: 1,
                fan_out_limit: 10,
                tenant_org_id: auth.org_id().to_string(),
            })
            .await
        {
            Ok(output) => graph_stage_output(output, started.elapsed(), degrade_trace),
            Err(error) => {
                let reason = format!("Graph retrieval failed: {error}");
                degrade_trace.push(DegradeTraceItem {
                    stage: "graph".to_string(),
                    reason: DegradeReason::Other(reason.clone()),
                    impact: "Skipping graph relation retrieval".to_string(),
                });
                GraphChannelOutput {
                    relation_paths: Vec::new(),
                    supporting_chunks: Vec::new(),
                    trace: channel_trace("graph", 0, started.elapsed(), &degrade_trace),
                    degrade_trace,
                }
            }
        }
    }

    pub async fn execute_plan(
        &self,
        request: &ExecutePlanRequest,
        auth: &AuthContext,
    ) -> Result<ExecutePlanResponse> {
        let cache_key = format!(
            "rag:execute:{}:{}",
            auth.org_id(),
            retrieval_cache_key(request)
        );
        if let Some(cache) = self.cache() {
            match cache.get_json::<ExecutePlanResponse>(&cache_key).await {
                Ok(Some(cached)) => {
                    tracing::debug!("L2 cache hit for execute_plan");
                    return Ok(cached);
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!("L2 cache read error: {}", e);
                }
            }
        }

        request
            .validate()
            .map_err(|error| anyhow!(error.to_string()))?;
        let total_candidate_budget = request
            .budget
            .as_ref()
            .and_then(|budget| budget.total_candidate_budget)
            .unwrap_or(TOTAL_CANDIDATE_BUDGET);
        let final_chunk_budget = request
            .budget
            .as_ref()
            .and_then(|budget| budget.final_chunk_budget)
            .unwrap_or(FINAL_MIN_CHUNKS);

        let compat_request = request.to_chat_request_compat();
        let compat_plan = request.to_rag_plan_compat();
        let item_trace =
            build_item_trace_with_total(&compat_request, &compat_plan, total_candidate_budget);
        let channel_budgets = channel_candidate_budgets(request, total_candidate_budget);
        let regular_candidate_budget = channel_budgets
            .text_dense
            .saturating_add(channel_budgets.bm25)
            .saturating_add(channel_budgets.multimodal_dense)
            .max(1);
        let rerank_budget = regular_candidate_budget.min(FINAL_RERANK_BUDGET);

        let (text_dense_output, bm25_output, multimodal_output, graph_output) = tokio::join!(
            self.run_text_dense_channel(
                &compat_request,
                auth,
                &compat_plan,
                channel_budgets.text_dense,
            ),
            self.run_bm25_channel(&compat_request, auth, &compat_plan, channel_budgets.bm25),
            self.run_multimodal_channel(
                &compat_request,
                auth,
                &compat_plan,
                channel_budgets.multimodal_dense,
            ),
            self.run_graph_channel(request, auth, channel_budgets.graph, channel_budgets.graph,),
        );

        let mut degrade_trace = Vec::new();
        degrade_trace.extend(text_dense_output.degrade_trace);
        degrade_trace.extend(bm25_output.degrade_trace);
        degrade_trace.extend(multimodal_output.degrade_trace);
        degrade_trace.extend(graph_output.degrade_trace);

        let text_pool = self.merge_text_stage_with_budget(
            text_dense_output.lists,
            bm25_output.lists,
            channel_budgets
                .text_dense
                .saturating_add(channel_budgets.bm25)
                .max(1),
        );
        let (reranked_chunks, mut stage_degrade) = self
            .multimodal_rerank_stage_with_budget(
                &compat_request.query,
                text_pool,
                multimodal_output.chunks,
                regular_candidate_budget,
                rerank_budget,
            )
            .await?;
        degrade_trace.append(&mut stage_degrade);

        let graph_final_budget =
            graph_final_context_budget(final_chunk_budget, graph_output.supporting_chunks.len());
        let graph_supported_chunks = graph_output
            .supporting_chunks
            .into_iter()
            .take(graph_final_budget)
            .collect::<Vec<_>>();
        let regular_final_budget = final_chunk_budget.saturating_sub(graph_supported_chunks.len());
        let retrieved_chunks =
            self.cut_final_candidates_stage_with_budget(reranked_chunks, regular_final_budget);
        let raw_summary_chunks = self
            .apply_summary_policy(&compat_request, auth, &compat_plan, &retrieved_chunks)
            .await?;
        let answer_context =
            self.build_answer_context_chunks(&raw_summary_chunks, &retrieved_chunks);
        let filtered_retrieved_chunks =
            filter_retrieved_chunks_for_context(&retrieved_chunks, &answer_context)
                .into_iter()
                .take(regular_final_budget)
                .collect::<Vec<_>>();
        let remaining_summary_budget = final_chunk_budget
            .saturating_sub(filtered_retrieved_chunks.len())
            .saturating_sub(graph_supported_chunks.len());
        let summary_chunks = answer_context
            .iter()
            .filter(|chunk| chunk.chunk_type == "summary")
            .take(remaining_summary_budget)
            .cloned()
            .collect::<Vec<_>>();

        let unique_doc_ids = filtered_retrieved_chunks
            .iter()
            .map(|chunk| chunk.doc_id)
            .chain(graph_supported_chunks.iter().map(|chunk| chunk.doc_id))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let doc_names = if let Some(content_store) = self.config.content_store.as_ref() {
            let names = content_store
                .get_document_names(auth, &unique_doc_ids)
                .await
                .inspect_err(|e| {
                    tracing::warn!(
                        error = %e,
                        doc_ids = ?unique_doc_ids,
                        "content_store.get_document_names failed, degrading"
                    )
                })
                .unwrap_or_default();
            if names.len() < unique_doc_ids.len() {
                tracing::info!(
                    resolved = names.len(),
                    requested = unique_doc_ids.len(),
                    "some document names could not be resolved"
                );
            }
            names
        } else {
            HashMap::new()
        };

        let mut citations = filtered_retrieved_chunks
            .iter()
            .enumerate()
            .map(|(index, chunk)| citation_from_scored(index, chunk, &doc_names))
            .collect::<Vec<_>>();
        let base_citation_count = citations.len();
        citations.extend(
            graph_supported_chunks
                .iter()
                .enumerate()
                .map(|(index, chunk)| {
                    citation_from_scored(base_citation_count + index, chunk, &doc_names)
                }),
        );

        let matched_doc_count = filtered_retrieved_chunks
            .iter()
            .map(|chunk| chunk.doc_id.to_string())
            .chain(
                graph_supported_chunks
                    .iter()
                    .map(|chunk| chunk.doc_id.to_string()),
            )
            .chain(
                summary_chunks
                    .iter()
                    .filter_map(|chunk| chunk.doc_id.clone()),
            )
            .collect::<HashSet<_>>()
            .len();
        let summary_chunk_count = summary_chunks.len();
        let summary_mode = rag_summary_mode(&compat_plan);

        let response = ExecutePlanResponse {
            bundle: RetrievalBundle {
                chunks: filtered_retrieved_chunks
                    .iter()
                    .map(retrieved_chunk_from_scored)
                    .collect(),
                graph_supported_chunks: graph_supported_chunks
                    .iter()
                    .map(retrieved_chunk_from_scored)
                    .collect(),
                relation_paths: graph_output.relation_paths,
                citations,
                summary_chunks,
            },
            coverage: Coverage {
                requested_doc_count: request.doc_scope.len(),
                matched_doc_count,
                retrieved_chunk_count: filtered_retrieved_chunks.len(),
                summary_chunk_count,
                channel_coverage: channel_coverage(
                    &filtered_retrieved_chunks,
                    &graph_supported_chunks,
                ),
            },
            degrade_trace,
            backend_trace: BackendTrace {
                trace: request.trace.clone(),
                item_trace: item_trace.clone(),
                channel_trace: vec![
                    text_dense_output.trace,
                    bm25_output.trace,
                    multimodal_output.trace,
                    graph_output.trace,
                ],
                retrieval_trace: contracts::chat::RagTraceSummary {
                    item_count: item_trace.len(),
                    total_candidate_budget,
                    max_rerank_docs: rerank_budget,
                    max_final_chunks: final_chunk_budget,
                    top_k_returned: filtered_retrieved_chunks.len(),
                    summary_mode,
                    items: item_trace,
                },
            },
        };

        if let Some(cache) = self.cache() {
            if let Err(e) = cache
                .set_json(&cache_key, &response, RETRIEVAL_CACHE_TTL_SECS)
                .await
            {
                tracing::warn!("L2 cache write error: {}", e);
            }
        }

        Ok(response)
    }
}

fn graph_stage_output(
    output: GraphSearchOutput,
    elapsed: Duration,
    degrade_trace: Vec<DegradeTraceItem>,
) -> GraphChannelOutput {
    let relation_paths = output
        .relation_paths
        .iter()
        .enumerate()
        .map(|(index, candidate)| relation_path_from_candidate(index, candidate))
        .collect::<Vec<_>>();
    let raw_count = relation_paths.len();
    let selected_count = output.supporting_chunks.len();
    GraphChannelOutput {
        relation_paths,
        supporting_chunks: output.supporting_chunks,
        trace: ChannelTraceItem {
            channel: "graph".to_string(),
            raw_count,
            hydrated_count: raw_count,
            selected_count,
            latency_ms: Some(elapsed.as_millis() as u64),
            degrade_reason: channel_degrade_reason(&degrade_trace),
        },
        degrade_trace,
    }
}
