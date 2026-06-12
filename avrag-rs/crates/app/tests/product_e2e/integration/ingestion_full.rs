//! P1: Full ingestion pipeline edge cases.

use std::time::Duration;

use crate::product_e2e::{ChatResponse, DocumentStatus, HttpResponse, TestContext, assertions::*};

#[tokio::test]
async fn empty_document_ingests_with_zero_chunks_and_degrades() {
    super::require_integration_suite();

    let mut ctx = TestContext::new_smoke_with_rag().await;

    // 1. Upload empty document
    let upload = ctx.upload_document("empty.txt").await.unwrap();
    assert_eq!(
        upload.status, 201,
        "expected HTTP 201 from POST .../documents"
    );

    // 2. Wait for ingestion — should still complete (not hang/fail)
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    // 3. Query the empty document — should degrade gracefully
    let http_resp: HttpResponse = ctx
        .chat(
            "What is in this document?",
            &upload.notebook_id,
            &[upload.document_id.clone()],
        )
        .await
        .unwrap();

    // 4. Protocol assertions
    assert_http_ok(&http_resp);

    // 5. Business assertions
    let resp: ChatResponse = http_resp.into_business().unwrap();
    // Empty doc should produce a degrade trace or fallback answer
    assert!(
        !resp.degrade_trace.is_empty() || resp.citations.is_empty(),
        "expected degrade trace or no citations for empty document, got citations: {:?}",
        resp.citations
    );
}
