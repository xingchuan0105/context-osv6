//! Regression test: `build_rag_chat_response_from_bundle` must preserve the
//! stable `citation_id` each citation was assigned when the retrieval bundle
//! was built. It must NOT renumber them to a contiguous 1..N display order,
//! because the rendered answer markup (`[[<citation_id>]]`) and the citations
//! persisted on the stored message are both derived from the same slice, and
//! `app-chat/src/citations.rs::lookup_citation` matches by `citation_id`.

#![allow(deprecated)]

use std::sync::Arc;

use avrag_rag_core::RagRuntime;
use avrag_retrieval_data_plane::{
    Bm25SearchOutput, Bm25SearchRequest, Bm25SearchTrace, GraphSearchOutput, GraphSearchRequest,
    MultimodalSearchRequest, RetrievalReadPort, ScoredChunk, TextDenseSearchRequest,
};
use contracts::chat::{ChatRequest, Citation, RagPlan, RagPlanItem};
use contracts::{
    BackendTrace, Coverage, RagTraceSummary, RetrievalBundle, RetrievedChunk,
};

/// No-op data plane: `build_rag_chat_response_from_bundle` performs no
/// retrieval, so these never need to return chunks.
struct NoopDataPlane;

#[async_trait::async_trait]
impl RetrievalReadPort for NoopDataPlane {
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
    async fn search_graph(&self, _request: GraphSearchRequest) -> anyhow::Result<GraphSearchOutput> {
        Ok(GraphSearchOutput::default())
    }
}

fn make_request(query: &str, agent_type: &str) -> ChatRequest {
    ChatRequest {
        query: query.to_string(),
        workspace_id: None,
        session_id: None,
        agent_type: agent_type.to_string(),
        source_type: None,
        source_token: None,
        doc_scope: Vec::new(),
        messages: Vec::new(),
        stream: false,
        debug: false,
        language: None,
        format_hint: None,
    }
}

fn runtime() -> RagRuntime {
    let config = avrag_rag_core::test_doubles::test_rag_config();
    RagRuntime::with_data_plane(config, Arc::new(NoopDataPlane))
}

/// Bundle whose citations carry deliberately non-contiguous, non-1-based
/// `citation_id`s (42 and 7). If the response builder renumbered them it would
/// emit 1 and 2 instead.
fn bundle_with_stable_citation_ids() -> (RetrievalBundle, BackendTrace, Coverage) {
    let chunk_a = RetrievedChunk {
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
    };
    let chunk_b = RetrievedChunk {
        chunk_id: "chunk-b".to_string(),
        doc_id: "doc-2".to_string(),
        chunk_type: "text".to_string(),
        page: Some(3),
        text: "Pager rotation runbook".to_string(),
        score: 0.8,
        retrieval_channel: "bm25".to_string(),
        asset_id: None,
        caption: None,
        image_url: None,
        parser_backend: None,
        source_locator: None,
        parse_run_id: None,
        score_breakdown: Vec::new(),
    };
    let citation = |id: i64, chunk: &RetrievedChunk, name: &str| Citation {
        citation_id: id,
        doc_id: chunk.doc_id.clone(),
        chunk_id: Some(chunk.chunk_id.clone()),
        page: chunk.page.map(|page| page as usize),
        doc_name: name.to_string(),
        preview: Some(chunk.text.chars().take(100).collect()),
        content: Some(chunk.text.clone()),
        score: chunk.score,
        layer: Some(chunk.retrieval_channel.clone()),
        chunk_type: Some(chunk.chunk_type.clone()),
        asset_id: None,
        caption: None,
        image_url: None,
        parser_backend: None,
        source_locator: None,
        parse_run_id: None,
    };

    let bundle = RetrievalBundle {
        chunks: vec![chunk_a.clone(), chunk_b.clone()],
        graph_supported_chunks: Vec::new(),
        relation_paths: Vec::new(),
        citations: vec![
            citation(42, &chunk_a, "Atlas"),
            citation(7, &chunk_b, "Runbook"),
        ],
        summary_chunks: Vec::new(),
    };
    let coverage = Coverage {
        requested_doc_count: 2,
        matched_doc_count: 2,
        retrieved_chunk_count: 2,
        summary_chunk_count: 0,
        channel_coverage: Default::default(),
    };
    let backend_trace = BackendTrace {
        item_trace: Vec::new(),
        channel_trace: Vec::new(),
        retrieval_trace: RagTraceSummary {
            item_count: 0,
            total_candidate_budget: 64,
            max_rerank_docs: 16,
            max_final_chunks: 8,
            top_k_returned: 2,
            summary_mode: "none".to_string(),
            items: Vec::new(),
        },
    };
    (bundle, backend_trace, coverage)
}

#[tokio::test]
async fn build_rag_chat_response_from_bundle_preserves_stable_citation_ids() {
    let runtime = runtime();
    let request = make_request("Summarize the findings", "rag");
    let rag_plan = RagPlan {
        plan_version: "rag-item-v2".to_string(),
        plan_confidence: 1.0,
        clarify_needed: false,
        clarify_message: String::new(),
        items: vec![RagPlanItem {
            priority: 1.0,
            query: Some("Summarize the findings".to_string()),
            bm25_terms: None,
            summary: None,
        }],
    };
    let (bundle, backend_trace, coverage) = bundle_with_stable_citation_ids();

    let response = runtime
        .build_rag_chat_response_from_bundle(
            &request,
            Some("session-1"),
            &rag_plan,
            &bundle,
            &backend_trace,
            &coverage,
            avrag_rag_core_ports::SynthesisOutput {
                answer_text: "Atlas [[cite:chunk-a]] and runbook [[cite:chunk-b]]"
                    .to_string(),
                answer_blocks: Vec::new(),
                cited_chunk_ids: vec!["chunk-a".to_string(), "chunk-b".to_string()],
                llm_usage: None,
            },
            Vec::new(),
        )
        .await
        .unwrap();

    // Both citations survive (none dropped).
    assert_eq!(
        response.citations.len(),
        2,
        "both cited chunks must produce a citation"
    );

    // Citation ids are the ORIGINAL bundle ids (42, 7), NOT renumbered (1, 2).
    let by_chunk: std::collections::HashMap<&str, i64> = response
        .citations
        .iter()
        .filter_map(|c| c.chunk_id.as_deref().map(|id| (id, c.citation_id)))
        .collect();
    assert_eq!(
        by_chunk.get("chunk-a"),
        Some(&42),
        "chunk-a must retain its stable citation_id 42, got {:?}",
        by_chunk.get("chunk-a")
    );
    assert_eq!(
        by_chunk.get("chunk-b"),
        Some(&7),
        "chunk-b must retain its stable citation_id 7, got {:?}",
        by_chunk.get("chunk-b")
    );

    // The rendered answer markup must reference those SAME stable ids,
    // confirming internal consistency between answer and citations (which is
    // what lookup_citation ultimately relies on after persistence).
    assert_eq!(
        response.answer, "Atlas [[42]] and runbook [[7]]",
        "answer markup must embed the preserved (non-renumbered) citation ids"
    );
}
