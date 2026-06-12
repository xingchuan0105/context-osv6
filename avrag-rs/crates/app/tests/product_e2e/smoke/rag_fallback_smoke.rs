//! P0-RAG-fallback: when mock skips codegen, server auto_fallback still returns citations.

use std::time::Duration;

use crate::product_e2e::{ChatResponse, DocumentStatus, HttpResponse, TestContext, assertions::*};

#[tokio::test]
async fn rag_auto_fallback_when_codegen_skipped() {

    super::require_smoke_suite();
    let mut ctx = TestContext::new_smoke_with_rag().await;

    let upload = ctx.upload_document("antifragile.txt").await.unwrap();
    assert_eq!(upload.status, 201);

    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    ctx.set_mock_rag_skip_codegen(true);

    let http_resp: HttpResponse = ctx
        .chat(
            "What is antifragility?",
            &upload.notebook_id,
            &[upload.document_id.clone()],
        )
        .await
        .unwrap();

    assert_http_ok(&http_resp);
    let resp: ChatResponse = http_resp.into_business().unwrap();

    assert_has_citations(&resp);
    assert_citation_doc_id(&resp, &upload.document_id);
    assert_answer_substantive(&resp, 50);
}
