//! Real-LLM RAG E2E regression tests.
//!
//! These tests validate that the V5 skill-based prompt assembly still
//! produces coherent RAG behavior against production LLM providers.
//!
//! Run:
//!   cargo test -p app --test product_e2e llm_real::rag_real -- --ignored --test-threads=1 --nocapture

use crate::product_e2e::{
    assertions::{
        assert_answer_has_doc_citation, assert_answer_substantive, assert_citation_doc_id,
        assert_citation_referenced_in_answer, assert_has_citations,
    },
    fixtures::shared_standard_doc_real_llm,
    llm_real::{
        REAL_LLM_MULTITOOL_MAX_ATTEMPTS, chat_with_citations_retry_attempts, chat_with_retry,
        merge_llm_real_extra, non_blocking_degrade,
    },
};

const RETRIEVAL_TOOLS: &[&str] = &[
    "dense_retrieval",
    "index_lookup",
    "doc_profile",
    "doc_summary",
];

/// P0: Basic RAG document Q&A returns a substantive answer with at least
/// one document citation when using a real LLM and real embedding provider.
#[tokio::test]
#[ignore = "requires real LLM API key; run with --ignored --test-threads=1"]
async fn real_llm_rag_document_qa_returns_citation() {
    super::require_nightly_suite();
    // Cold ingest once per binary via standard_doc; reuse for other thin agent queries.
    let (ctx, upload) = shared_standard_doc_real_llm().await;

    // Ask a question that requires reading the document (retry for transient LLM errors).
    let result = chat_with_retry(
        &ctx,
        "What is antifragility?",
        &upload.workspace_id,
        &[upload.document_id.clone()],
    )
    .await;
    let resp = &result.resp;

    // Product assertions — align with smoke/rag_smoke: citations + substance, not keywords.
    assert_has_citations(resp);
    assert_citation_doc_id(resp, &upload.document_id);
    assert_answer_has_doc_citation(resp);
    assert_answer_substantive(resp, 50);
    assert_citation_referenced_in_answer(resp);
    let blocking: Vec<_> = resp
        .degrade_trace
        .iter()
        .filter(|item| !non_blocking_degrade(item))
        .collect();
    assert!(
        blocking.is_empty(),
        "expected no blocking degradation on the happy path, got: {:?}",
        blocking
    );

    // 6. RAG retrieval is codegen/SDK-only (no native dense_retrieval tool_call).
    // Evidence quality is covered by citation assertions above.

    // 7. Persist artifact for audit even on pass.
    ctx.save_llm_artifact(
        "real_llm_rag_document_qa_returns_citation",
        resp,
        merge_llm_real_extra(
            &result,
            Some(serde_json::json!({"document_id": upload.document_id})),
        ),
        Some(result.reasoning),
    );
}

/// R1: Complex query should hit real retrieval (loose, non-deterministic).
///
/// Prefer ≥2 distinct tools when the model cooperates; otherwise require citations
/// plus at least one retrieval-class tool result (single consolidated codegen is OK).
#[tokio::test]
#[ignore = "requires real LLM API key; run with --ignored --test-threads=1"]
async fn real_llm_rag_complex_query_uses_multiple_tools() {
    super::require_nightly_suite();
    // Extended (not thin default): still reuses the same cold standard doc.
    let (ctx, upload) = shared_standard_doc_real_llm().await;

    let result = chat_with_citations_retry_attempts(
        &ctx,
        "First summarize this book's author and chapter structure, then explain a core idea from an early section.",
        &upload.workspace_id,
        &[upload.document_id.clone()],
        REAL_LLM_MULTITOOL_MAX_ATTEMPTS + 2,
    )
    .await;
    let resp = &result.resp;

    assert_has_citations(resp);
    assert_citation_doc_id(resp, &upload.document_id);
    assert_answer_substantive(resp, 80);
    assert!(
        resp.tool_results
            .iter()
            .any(|r| RETRIEVAL_TOOLS.contains(&r.tool.as_str())),
        "expected at least one retrieval-class tool, got: {:?}",
        resp.tool_results
            .iter()
            .map(|r| &r.tool)
            .collect::<Vec<_>>()
    );
    let blocking_degrades: Vec<_> = resp
        .degrade_trace
        .iter()
        .filter(|item| !non_blocking_degrade(item))
        .collect();
    assert!(
        blocking_degrades.is_empty(),
        "expected no blocking degradation on complex query path, got: {:?}",
        blocking_degrades
    );

    let distinct_tools: std::collections::HashSet<_> =
        resp.tool_results.iter().map(|r| r.tool.as_str()).collect();
    let multitool_goal_met = distinct_tools.len() >= 2;
    if !multitool_goal_met {
        eprintln!(
            "[llm_real] R1: multi-tool goal not met (got {distinct_tools:?}); \
             passing on citation-backed retrieval path"
        );
    }

    ctx.save_llm_artifact(
        "real_llm_rag_complex_query_uses_multiple_tools",
        resp,
        merge_llm_real_extra(
            &result,
            Some(serde_json::json!({
                "document_id": upload.document_id,
                "distinct_tools": resp.tool_results.iter().map(|r| &r.tool).collect::<Vec<_>>(),
                "multitool_goal_met": multitool_goal_met,
            })),
        ),
        Some(result.reasoning),
    );
}
