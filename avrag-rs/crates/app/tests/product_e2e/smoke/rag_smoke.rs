//! P0-2: Document Q&A returns structured citation.
//!
//! Bridge coverage: this smoke uses [`TestContext::chat_without_mock_chunk_pin`] so
//! citations must come from real sandbox `dense_search` output, not mock-synthesis pin.
//! End-to-end bridge plumbing is also covered by `interpreter_hits_runtime_bridge_end_to_end`
//! in `rag-core`.

use std::time::Duration;

use crate::product_e2e::{ChatResponse, DocumentStatus, HttpResponse, TestContext, assertions::*};

#[tokio::test]
async fn rag_document_qa_returns_citation() {
    super::require_smoke_suite();
    let mut ctx = TestContext::new_smoke_with_rag().await;

    let upload = ctx.upload_document("antifragile.txt").await.unwrap();
    assert_eq!(
        upload.status, 201,
        "expected HTTP 201 from POST .../documents"
    );

    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    let chunk_count = ctx
        .query_document_chunk_count(&upload.document_id)
        .await
        .unwrap();
    assert!(
        chunk_count > 0,
        "expected chunk_count > 0 after successful ingestion, got {chunk_count}"
    );

    let document_chunk_ids = ctx
        .query_document_chunk_ids(&upload.document_id)
        .await
        .unwrap();
    assert!(
        !document_chunk_ids.is_empty(),
        "expected chunk ids in PG for bridge citation assertions"
    );

    let http_resp: HttpResponse = ctx
        .chat_without_mock_chunk_pin(
            "What is antifragility?",
            &upload.notebook_id,
            &[upload.document_id.clone()],
        )
        .await
        .unwrap();

    assert_http_ok(&http_resp);

    let resp: ChatResponse = http_resp.into_business().unwrap();

    assert!(
        resp.degrade_trace.is_empty(),
        "codegen happy path should not degrade: {:?}",
        resp.degrade_trace
    );
    assert_codegen_bridge_dense_retrieval(&resp);
    assert_has_citations(&resp);
    assert_citations_use_document_chunks(&resp, &document_chunk_ids);
    assert_citation_doc_id(&resp, &upload.document_id);
    assert_citation_referenced_in_answer(&resp);
    assert_answer_has_doc_citation(&resp);
    assert_answer_substantive(&resp, 50);
}
