use std::{collections::HashMap, collections::HashSet};

use anyhow::Result;
use contracts::auth_runtime::AuthContext;
use avrag_llm::{MultiModalEmbeddingInput, MultiModalRerankDocument};
use avrag_retrieval_data_plane::{
    Bm25SearchRequest, MultimodalSearchRequest, TextDenseSearchRequest, WeightedChunkList,
};
use contracts::chat::{ChatRequest, DegradeReason, DegradeTraceItem, RagPlan};

use crate::merge::{cut_top_k, dual_threshold_cut, global_rrf_merge};
use crate::retrieval::ScoredChunk;

use super::planner::{
    build_item_trace_with_total, effective_item_query, item_payload_kind, request_doc_ids,
};
use super::{
    FINAL_MIN_CHUNKS, FINAL_RERANK_BUDGET, FINAL_SCORE_THRESHOLD, GLOBAL_RRF_K, RagRuntime,
    TOTAL_CANDIDATE_BUDGET,
};

pub(super) fn build_final_candidate_pool(
    text_pool: Vec<ScoredChunk>,
    multimodal_pool: Vec<ScoredChunk>,
    max_candidates: usize,
) -> Vec<ScoredChunk> {
    if max_candidates == 0 {
        return Vec::new();
    }

    let mut merged = Vec::with_capacity(max_candidates);
    let mut seen = HashSet::new();
    let mut text_iter = text_pool.into_iter();
    let mut multimodal_iter = multimodal_pool.into_iter();

    loop {
        let mut progressed = false;

        if let Some(chunk) = text_iter.next() {
            progressed = true;
            if seen.insert(chunk.chunk_id) {
                merged.push(chunk);
                if merged.len() >= max_candidates {
                    break;
                }
            }
        }

        if let Some(chunk) = multimodal_iter.next() {
            progressed = true;
            if seen.insert(chunk.chunk_id) {
                merged.push(chunk);
                if merged.len() >= max_candidates {
                    break;
                }
            }
        }

        if !progressed {
            break;
        }
    }

    merged
}

pub(super) fn build_multimodal_rerank_documents(
    chunks: &[ScoredChunk],
) -> Vec<MultiModalRerankDocument> {
    chunks
        .iter()
        .map(|chunk| match chunk.image_path.clone() {
            Some(path) if !path.trim().is_empty() => MultiModalRerankDocument::Image(path),
            _ => MultiModalRerankDocument::Text(chunk.content.clone()),
        })
        .collect()
}

impl RagRuntime {
    pub async fn retrieve_text_dense_stage(
        &self,
        request: &ChatRequest,
        auth: &AuthContext,
        rag_plan: &RagPlan,
    ) -> Result<(Vec<super::WeightedChunkList>, Vec<DegradeTraceItem>)> {
        self.retrieve_text_dense_stage_with_budget(request, auth, rag_plan, TOTAL_CANDIDATE_BUDGET)
            .await
    }

    pub async fn retrieve_text_dense_stage_with_budget(
        &self,
        request: &ChatRequest,
        auth: &AuthContext,
        rag_plan: &RagPlan,
        total_candidate_budget: usize,
    ) -> Result<(Vec<super::WeightedChunkList>, Vec<DegradeTraceItem>)> {
        let doc_ids = request_doc_ids(request);
        let item_trace = build_item_trace_with_total(request, rag_plan, total_candidate_budget);
        let mut lists = Vec::new();
        let mut degrade_trace = Vec::new();

        for (item, trace_item) in rag_plan.items.iter().zip(item_trace.iter()) {
            if item_payload_kind(item) != "query" || trace_item.recall_budget == 0 {
                continue;
            }

            let effective_query = effective_item_query(item, &request.query);
            let query_vector = match self
                .config
                .embedding_client
                .embed(&[&effective_query])
                .await
            {
                Ok(vectors) => vectors.into_iter().next(),
                Err(error) => {
                    degrade_trace.push(DegradeTraceItem {
                        stage: "text_dense".to_string(),
                        reason: DegradeReason::EmbeddingUnavailable,
                        impact: format!(
                            "Skipping text dense retrieval for one query item: {error}"
                        ),
                    });
                    None
                }
            };

            let Some(vector) = query_vector else {
                continue;
            };

            match self
                .data_plane
                .search_text_dense(TextDenseSearchRequest {
                    auth: auth.clone(),
                    query_vector: vector,
                    doc_ids: doc_ids.clone(),
                    limit: trace_item.recall_budget,
                })
                .await
            {
                Ok(chunks) => lists.push(WeightedChunkList {
                    weight: item.priority,
                    chunks: cut_top_k(chunks, trace_item.recall_budget),
                }),
                Err(error) => degrade_trace.push(DegradeTraceItem {
                    stage: "text_dense".to_string(),
                    reason: DegradeReason::Other(format!("Text dense retrieval failed: {}", error)),
                    impact: "Skipping text dense retrieval for one query item".to_string(),
                }),
            }
        }

        Ok((lists, degrade_trace))
    }

    pub async fn retrieve_bm25_stage(
        &self,
        request: &ChatRequest,
        auth: &AuthContext,
        rag_plan: &RagPlan,
    ) -> Result<(Vec<super::WeightedChunkList>, Vec<DegradeTraceItem>)> {
        self.retrieve_bm25_stage_with_budget(request, auth, rag_plan, TOTAL_CANDIDATE_BUDGET)
            .await
    }

    pub async fn retrieve_bm25_stage_with_budget(
        &self,
        request: &ChatRequest,
        auth: &AuthContext,
        rag_plan: &RagPlan,
        total_candidate_budget: usize,
    ) -> Result<(Vec<super::WeightedChunkList>, Vec<DegradeTraceItem>)> {
        let doc_ids = request_doc_ids(request);
        let item_trace = build_item_trace_with_total(request, rag_plan, total_candidate_budget);
        let mut lists = Vec::new();
        let mut degrade_trace = Vec::new();

        for (item, trace_item) in rag_plan.items.iter().zip(item_trace.iter()) {
            if item_payload_kind(item) != "bm25_terms" || trace_item.recall_budget == 0 {
                continue;
            }

            let effective_query = effective_item_query(item, &request.query);
            match self
                .data_plane
                .search_bm25(Bm25SearchRequest {
                    auth: auth.clone(),
                    query: effective_query,
                    doc_ids: doc_ids.clone(),
                    limit: trace_item.recall_budget,
                })
                .await
            {
                Ok(output) => {
                    if let Some(reason) = output.trace.fallback_reason.as_ref() {
                        degrade_trace.push(DegradeTraceItem {
                            stage: "bm25".to_string(),
                            reason: DegradeReason::Other(format!(
                                "Sparse retrieval fallback: {}",
                                reason
                            )),
                            impact: "Used a fallback sparse retrieval path for one lexical item"
                                .to_string(),
                        });
                    }
                    tracing::info!(
                        lexical_backend = %output.trace.backend,
                        raw_hit_count = output.trace.raw_hit_count,
                        hydrated_hit_count = output.trace.hydrated_hit_count,
                        "bm25_terms retrieval item completed"
                    );
                    lists.push(WeightedChunkList {
                        weight: item.priority,
                        chunks: cut_top_k(output.chunks, trace_item.recall_budget),
                    });
                }
                Err(error) => degrade_trace.push(DegradeTraceItem {
                    stage: "bm25".to_string(),
                    reason: DegradeReason::Other(format!("BM25 retrieval failed: {}", error)),
                    impact: "Skipping sparse retrieval for one lexical item".to_string(),
                }),
            }
        }

        Ok((lists, degrade_trace))
    }

    pub async fn retrieve_multimodal_dense_stage(
        &self,
        request: &ChatRequest,
        auth: &AuthContext,
        rag_plan: &RagPlan,
    ) -> Result<(Vec<ScoredChunk>, Vec<DegradeTraceItem>)> {
        self.retrieve_multimodal_dense_stage_with_budget(
            request,
            auth,
            rag_plan,
            TOTAL_CANDIDATE_BUDGET,
        )
        .await
    }

    pub async fn retrieve_multimodal_dense_stage_with_budget(
        &self,
        request: &ChatRequest,
        auth: &AuthContext,
        rag_plan: &RagPlan,
        total_candidate_budget: usize,
    ) -> Result<(Vec<ScoredChunk>, Vec<DegradeTraceItem>)> {
        let doc_ids = request_doc_ids(request);
        let item_trace = build_item_trace_with_total(request, rag_plan, total_candidate_budget);
        let mut chunks = Vec::new();
        let mut degrade_trace = Vec::new();
        let query_item_count = rag_plan
            .items
            .iter()
            .filter(|item| item_payload_kind(item) == "query")
            .count();

        let Some(mm_client) = self.config.mm_embedding_client.as_ref() else {
            if query_item_count > 0 {
                degrade_trace.push(DegradeTraceItem {
                    stage: "multimodal_dense".to_string(),
                    reason: DegradeReason::EmbeddingUnavailable,
                    impact: "Skipping multimodal dense retrieval".to_string(),
                });
            }
            return Ok((chunks, degrade_trace));
        };

        for (item, trace_item) in rag_plan.items.iter().zip(item_trace.iter()) {
            if item_payload_kind(item) != "query" || trace_item.recall_budget == 0 {
                continue;
            }

            let effective_query = effective_item_query(item, &request.query);
            let vector = match mm_client
                .embed_multimodal_fused(&MultiModalEmbeddingInput::text(effective_query), None)
                .await
            {
                Ok(vector) => vector,
                Err(error) => {
                    degrade_trace.push(DegradeTraceItem {
                        stage: "multimodal_dense".to_string(),
                        reason: DegradeReason::Other(format!(
                            "Multimodal embedding failed: {}",
                            error
                        )),
                        impact: "Skipping multimodal dense retrieval for one query item"
                            .to_string(),
                    });
                    continue;
                }
            };

            match self
                .data_plane
                .search_multimodal(MultimodalSearchRequest {
                    auth: auth.clone(),
                    query_vector: vector,
                    doc_ids: doc_ids.clone(),
                    limit: trace_item.recall_budget,
                })
                .await
            {
                Ok(results) => chunks.extend(results),
                Err(error) => degrade_trace.push(DegradeTraceItem {
                    stage: "multimodal_dense".to_string(),
                    reason: DegradeReason::Other(format!(
                        "Multimodal dense retrieval failed: {}",
                        error
                    )),
                    impact: "Skipping multimodal dense retrieval for one query item".to_string(),
                }),
            }
        }

        Ok((cut_top_k(chunks, total_candidate_budget), degrade_trace))
    }

    pub fn merge_text_stage(
        &self,
        text_dense_lists: Vec<super::WeightedChunkList>,
        sparse_lists: Vec<super::WeightedChunkList>,
    ) -> Vec<ScoredChunk> {
        self.merge_text_stage_with_budget(text_dense_lists, sparse_lists, TOTAL_CANDIDATE_BUDGET)
    }

    pub fn merge_text_stage_with_budget(
        &self,
        text_dense_lists: Vec<super::WeightedChunkList>,
        sparse_lists: Vec<super::WeightedChunkList>,
        total_candidate_budget: usize,
    ) -> Vec<ScoredChunk> {
        let mut rrf_inputs = Vec::new();
        for list in text_dense_lists {
            rrf_inputs.push((list.chunks, list.weight));
        }
        for list in sparse_lists {
            rrf_inputs.push((list.chunks, list.weight));
        }
        cut_top_k(
            global_rrf_merge(rrf_inputs, GLOBAL_RRF_K),
            total_candidate_budget,
        )
    }

    pub async fn multimodal_rerank_stage(
        &self,
        query: &str,
        text_pool: Vec<ScoredChunk>,
        multimodal_pool: Vec<ScoredChunk>,
    ) -> Result<(Vec<ScoredChunk>, Vec<DegradeTraceItem>)> {
        self.multimodal_rerank_stage_with_budget(
            query,
            text_pool,
            multimodal_pool,
            TOTAL_CANDIDATE_BUDGET,
            FINAL_RERANK_BUDGET,
        )
        .await
    }

    pub async fn multimodal_rerank_stage_with_budget(
        &self,
        query: &str,
        text_pool: Vec<ScoredChunk>,
        multimodal_pool: Vec<ScoredChunk>,
        total_candidate_budget: usize,
        rerank_budget: usize,
    ) -> Result<(Vec<ScoredChunk>, Vec<DegradeTraceItem>)> {
        let final_candidates =
            build_final_candidate_pool(text_pool, multimodal_pool, total_candidate_budget);
        if final_candidates.is_empty() {
            return Ok((
                Vec::new(),
                vec![DegradeTraceItem {
                    stage: "retrieval".to_string(),
                    reason: DegradeReason::NoValidRetrievalResults,
                    impact: "Passing zero-recall context to answer synthesis".to_string(),
                }],
            ));
        }

        let mut degrade_trace = Vec::new();
        let reranked = self
            .rerank_item_chunks(query, final_candidates, rerank_budget, &mut degrade_trace)
            .await;
        Ok((reranked, degrade_trace))
    }

    pub fn cut_final_candidates_stage(&self, reranked: Vec<ScoredChunk>) -> Vec<ScoredChunk> {
        dual_threshold_cut(reranked, FINAL_MIN_CHUNKS, FINAL_SCORE_THRESHOLD)
    }

    pub fn cut_final_candidates_stage_with_budget(
        &self,
        reranked: Vec<ScoredChunk>,
        final_chunk_budget: usize,
    ) -> Vec<ScoredChunk> {
        dual_threshold_cut(reranked, final_chunk_budget, FINAL_SCORE_THRESHOLD)
            .into_iter()
            .take(final_chunk_budget)
            .collect()
    }
}

impl RagRuntime {
    async fn rerank_item_chunks(
        &self,
        query: &str,
        chunks: Vec<ScoredChunk>,
        rerank_budget: usize,
        degrade_trace: &mut Vec<DegradeTraceItem>,
    ) -> Vec<ScoredChunk> {
        if chunks.is_empty() {
            return chunks;
        }

        if let Some(mm_reranker) = &self.config.mm_reranker {
            let documents = build_multimodal_rerank_documents(&chunks);
            match mm_reranker
                .rerank_multimodal_text_query(query, &documents, rerank_budget.min(documents.len()))
                .await
            {
                Ok(results) => {
                    let mut ranked = chunks;
                    let original_index_by_chunk = ranked
                        .iter()
                        .enumerate()
                        .map(|(index, chunk)| (chunk.chunk_id, index))
                        .collect::<HashMap<_, _>>();
                    let mut score_by_index = HashMap::new();
                    for result in results {
                        score_by_index.insert(result.index, result.score);
                    }
                    // Write rerank scores back into chunk.score so downstream
                    // cut_top_k / dual_threshold_cut order by rerank relevance
                    // instead of the stale dense score.
                    for chunk in &mut ranked {
                        if let Some(&index) = original_index_by_chunk.get(&chunk.chunk_id) {
                            if let Some(&score) = score_by_index.get(&index) {
                                chunk.score = score;
                            }
                        }
                    }
                    ranked.sort_by(|left, right| {
                        right
                            .score
                            .partial_cmp(&left.score)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                    return cut_top_k(ranked, rerank_budget);
                }
                Err(error) => {
                    degrade_trace.push(DegradeTraceItem {
                        stage: "mm_reranker".to_string(),
                        reason: DegradeReason::Other(format!(
                            "Multimodal reranker call failed: {}",
                            error
                        )),
                        impact: "Falling back to text rerank or pre-rerank ordering".to_string(),
                    });
                }
            }
        }

        if let Some(reranker) = &self.config.reranker {
            let doc_texts = chunks
                .iter()
                .map(|item| item.content.clone())
                .collect::<Vec<_>>();
            match reranker.rerank(query, &doc_texts).await {
                Ok(results) => {
                    let mut ranked = chunks;
                    let mut score_by_chunk = HashMap::new();
                    for result in results {
                        if result.index < ranked.len() {
                            score_by_chunk.insert(ranked[result.index].chunk_id, result.score);
                        }
                    }
                    ranked.sort_by(|left, right| {
                        let left_score = score_by_chunk.get(&left.chunk_id).copied().unwrap_or(0.0);
                        let right_score =
                            score_by_chunk.get(&right.chunk_id).copied().unwrap_or(0.0);
                        right_score
                            .partial_cmp(&left_score)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                    return cut_top_k(ranked, rerank_budget);
                }
                Err(error) => {
                    degrade_trace.push(DegradeTraceItem {
                        stage: "reranker".to_string(),
                        reason: DegradeReason::Other(format!("Reranker call failed: {}", error)),
                        impact: "Using pre-rerank ordering".to_string(),
                    });
                }
            }
        }
        cut_top_k(chunks, rerank_budget)
    }
}
