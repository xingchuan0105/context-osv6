//! Shared RAG-ready fixture: smoke context + ingested antifragile document.

use std::time::Duration;

use super::super::{DocumentStatus, TestContext, UploadResponse};

/// Smoke RAG context with `antifragile.txt` uploaded and ingestion completed.
pub async fn ready_rag_context() -> (TestContext, UploadResponse) {
    let mut ctx = TestContext::new_smoke_with_rag().await;
    let upload = ctx
        .upload_document("antifragile.txt")
        .await
        .expect("upload antifragile fixture");
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(120))
        .await
        .expect("wait for ingestion");
    assert_eq!(status, DocumentStatus::Completed);
    (ctx, upload)
}
