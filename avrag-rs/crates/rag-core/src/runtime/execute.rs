use std::collections::{BTreeSet, HashMap, HashSet};

use anyhow::{Result, anyhow};
use avrag_auth::AuthContext;
use common::{
    BackendTrace, Coverage, ExecutePlanRequest, ExecutePlanResponse, RetrievalBundle,
    RetrievedChunk,
};

use crate::retrieval::ScoredChunk;

use super::planner::{build_item_trace_with_total, rag_summary_mode};
use super::{FINAL_MIN_CHUNKS, FINAL_RERANK_BUDGET, RagRuntime, TOTAL_CANDIDATE_BUDGET};

fn filter_retrieved_chunks_for_context(
    retrieved_chunks: &[ScoredChunk],
    context_chunks: &[common::AnswerContextChunk],
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
    }
}

impl RagRuntime {
    pub async fn execute_plan(
        &self,
        request: &ExecutePlanRequest,
        auth: &AuthContext,
    ) -> Result<ExecutePlanResponse> {
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
        let rerank_budget = total_candidate_budget.min(FINAL_RERANK_BUDGET);

        let (text_dense_lists, mut degrade_trace) = self
            .retrieve_text_dense_stage_with_budget(
                &compat_request,
                auth,
                &compat_plan,
                total_candidate_budget,
            )
            .await?;
        let (bm25_lists, mut stage_degrade) = self
            .retrieve_bm25_stage_with_budget(
                &compat_request,
                auth,
                &compat_plan,
                total_candidate_budget,
            )
            .await?;
        degrade_trace.append(&mut stage_degrade);

        let (multimodal_pool, mut stage_degrade) = self
            .retrieve_multimodal_dense_stage_with_budget(
                &compat_request,
                auth,
                &compat_plan,
                total_candidate_budget,
            )
            .await?;
        degrade_trace.append(&mut stage_degrade);

        let text_pool =
            self.merge_text_stage_with_budget(text_dense_lists, bm25_lists, total_candidate_budget);
        let (reranked_chunks, mut stage_degrade) = self
            .multimodal_rerank_stage_with_budget(
                &compat_request.query,
                text_pool,
                multimodal_pool,
                total_candidate_budget,
                rerank_budget,
            )
            .await?;
        degrade_trace.append(&mut stage_degrade);

        let retrieved_chunks =
            self.cut_final_candidates_stage_with_budget(reranked_chunks, final_chunk_budget);
        let raw_summary_chunks = self
            .apply_summary_policy(&compat_request, auth, &compat_plan, &retrieved_chunks)
            .await?;
        let answer_context =
            self.build_answer_context_chunks(&raw_summary_chunks, &retrieved_chunks);
        let filtered_retrieved_chunks =
            filter_retrieved_chunks_for_context(&retrieved_chunks, &answer_context)
                .into_iter()
                .take(final_chunk_budget)
                .collect::<Vec<_>>();
        let remaining_summary_budget =
            final_chunk_budget.saturating_sub(filtered_retrieved_chunks.len());
        let summary_chunks = answer_context
            .iter()
            .filter(|chunk| chunk.chunk_type == "summary")
            .take(remaining_summary_budget)
            .cloned()
            .collect::<Vec<_>>();

        let unique_doc_ids = filtered_retrieved_chunks
            .iter()
            .map(|chunk| chunk.doc_id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let doc_names = if let Some(pg_repo) = self.config.pg_repo.as_ref() {
            pg_repo
                .get_document_names(auth, &unique_doc_ids)
                .await
                .unwrap_or_default()
        } else {
            HashMap::new()
        };

        let citations = filtered_retrieved_chunks
            .iter()
            .enumerate()
            .map(|(index, chunk)| common::Citation {
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
            })
            .collect::<Vec<_>>();

        let matched_doc_count = filtered_retrieved_chunks
            .iter()
            .map(|chunk| chunk.doc_id.to_string())
            .chain(
                summary_chunks
                    .iter()
                    .filter_map(|chunk| chunk.doc_id.clone()),
            )
            .collect::<HashSet<_>>()
            .len();
        let summary_chunk_count = summary_chunks.len();
        let summary_mode = rag_summary_mode(&compat_plan);

        Ok(ExecutePlanResponse {
            bundle: RetrievalBundle {
                chunks: filtered_retrieved_chunks
                    .iter()
                    .map(retrieved_chunk_from_scored)
                    .collect(),
                citations,
                summary_chunks,
            },
            coverage: Coverage {
                requested_doc_count: request.doc_scope.len(),
                matched_doc_count,
                retrieved_chunk_count: filtered_retrieved_chunks.len(),
                summary_chunk_count,
            },
            degrade_trace,
            backend_trace: BackendTrace {
                trace: request.trace.clone(),
                item_trace: item_trace.clone(),
                retrieval_trace: common::RagTraceSummary {
                    item_count: item_trace.len(),
                    total_candidate_budget,
                    max_rerank_docs: rerank_budget,
                    max_final_chunks: final_chunk_budget,
                    top_k_returned: filtered_retrieved_chunks.len(),
                    summary_mode,
                    items: item_trace,
                },
            },
        })
    }
}
