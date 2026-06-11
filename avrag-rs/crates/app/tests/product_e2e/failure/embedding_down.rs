//! P2-11: Embedding service unavailable → RAG degrades to lexical or returns degrade trace.

use std::time::Duration;

use crate::product_e2e::{ChatResponse, DegradeReason, DocumentStatus, TestContext, assertions::*};

#[tokio::test]
async fn embedding_503_returns_degraded_answer_with_lexical_fallback() {
    let mut ctx = TestContext::new_smoke_with_rag().await;

    // 1. Upload and ingest a document while embedding is healthy.
    let upload = ctx.upload_document("antifragile.txt").await.unwrap();
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(60))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed, "document should ingest");

    // 2. Flip embedding mock to 503 — simulates dense-retrieval failure.
    ctx.set_embedding_503(true);

    // 3. Ask a RAG question.
    let http_resp = ctx
        .chat(
            "What is antifragility?",
            &upload.notebook_id,
            &[upload.document_id.clone()],
        )
        .await
        .unwrap();

    assert_http_ok(&http_resp);

    let resp: ChatResponse =
        serde_json::from_value(http_resp.body_json.clone()).expect("valid ChatResponse schema");

    // 4. Product-layer assertions:
    //    - HTTP 200 (system does not crash)
    //    - degrade_trace must be non-empty (embedding 503 triggered a degradation path)
    assert!(
        !resp.degrade_trace.is_empty(),
        "expected degrade_trace when embedding is unavailable, got: {:?}",
        resp.degrade_trace
    );
    assert_degrade_reason(&resp, DegradeReason::EmbeddingUnavailable);
    assert!(
        !resp.answer.trim().is_empty(),
        "expected non-empty answer even when embedding is down, got: {:?}",
        resp.answer
    );
}
