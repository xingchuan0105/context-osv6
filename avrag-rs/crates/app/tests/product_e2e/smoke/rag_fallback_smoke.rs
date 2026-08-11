//! Legacy name kept for registry continuity.
//!
//! auto_fallback / host dense rewire after SaC-skip is **retired** (Lead+Workers).
//! This smoke checks the product RAG path still returns citations without the
//! old codegen-skip → server auto_fallback safety net.

use std::time::Duration;

use crate::product_e2e::{ChatResponse, DocumentStatus, HttpResponse, TestContext, assertions::*};

#[tokio::test]
async fn rag_lead_workers_returns_citations_without_auto_fallback() {
    super::require_smoke_suite();
    let mut ctx = TestContext::new_smoke_with_rag().await;

    let upload = ctx.upload_document("antifragile.txt").await.unwrap();
    assert_eq!(upload.status, 201);

    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(120))
        .await
        .unwrap();
    assert_eq!(status, DocumentStatus::Completed);

    // Do **not** set_mock_rag_skip_codegen — that path no longer triggers host dense
    // auto_fallback under Lead+Workers (design §13.3: no rewire).

    let http_resp: HttpResponse = ctx
        .chat(
            "What is antifragility?",
            &upload.workspace_id,
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
