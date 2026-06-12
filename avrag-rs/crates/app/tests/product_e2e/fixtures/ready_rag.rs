//! Shared RAG-ready fixture: smoke context + ingested antifragile document.

use std::sync::Mutex;

use tokio::sync::OnceCell;

use super::super::{DocumentStatus, TestContext, UploadResponse};

/// Smoke RAG context with `antifragile.txt` uploaded and ingestion completed.
pub async fn ready_rag_context() -> (TestContext, UploadResponse) {
    let mut ctx = TestContext::new_smoke_with_rag().await;
    let upload = ctx
        .upload_document("antifragile.txt")
        .await
        .expect("upload antifragile fixture");
    let status = ctx
        .wait_for_ingestion(&upload.document_id, std::time::Duration::from_secs(120))
        .await
        .expect("wait for ingestion");
    assert_eq!(status, DocumentStatus::Completed);
    (ctx, upload)
}

static SHARED_READY_RAG: OnceCell<(Mutex<TestContext>, UploadResponse)> = OnceCell::const_new();

/// Module-scoped RAG fixture: one cold bootstrap per test binary (requires `--test-threads=1`).
pub async fn shared_ready_rag() -> &'static (Mutex<TestContext>, UploadResponse) {
    SHARED_READY_RAG
        .get_or_init(|| async {
            let (ctx, upload) = ready_rag_context().await;
            (Mutex::new(ctx), upload)
        })
        .await
}
