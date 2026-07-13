//! Canonical **standard document** for product E2E paths that need retrieval after ingest.
//!
//! ## Policy (L3 refactor 2026-07-10)
//!
//! - **One fixture file**: [`STANDARD_DOC_FIXTURE`] (`antifragile.txt`).
//! - Bytes are identical under:
//!   - `avrag-rs/crates/app/tests/product_e2e/fixtures/antifragile.txt`
//!   - `frontend_next/e2e/fixtures/antifragile.txt`
//! - **Real-LLM thin paths** (rag / multi_turn / format) share **one cold upload+ingest**
//!   per test binary via [`shared_standard_doc_real_llm`], then respawn worker only.
//! - Playwright journey/skills should prefer this same file when testing upload→RAG,
//!   so queries and golden sets stay aligned (see `TEST_PYRAMID_DEDUP_MAP`).
//!
//! Chat-only and search-open-web paths do **not** need this corpus.

use std::time::Duration;

use tokio::sync::OnceCell;

use super::ready_rag::RagSharedFixture;
use super::super::{DocumentStatus, TestContext, UploadResponse};

/// Sole standard product document name (relative to product_e2e fixtures dir).
pub const STANDARD_DOC_FIXTURE: &str = "antifragile.txt";

static SHARED_REAL_LLM_STANDARD_DOC: OnceCell<RagSharedFixture> = OnceCell::const_new();

async fn cold_real_llm_standard_doc() -> RagSharedFixture {
    let mut ctx = TestContext::new_with_real_llm().await;
    let upload = ctx
        .upload_document(STANDARD_DOC_FIXTURE)
        .await
        .expect("upload standard doc antifragile.txt");
    assert_eq!(upload.status, 201, "standard doc upload must return 201");
    let status = ctx
        .wait_for_ingestion(&upload.document_id, Duration::from_secs(180))
        .await
        .expect("ingest standard doc");
    assert_eq!(
        status,
        DocumentStatus::Completed,
        "standard doc ingestion must complete"
    );
    RagSharedFixture::from_context(ctx, upload)
}

/// Module-scoped real-LLM infra + one ingested standard document.
pub(crate) async fn shared_standard_doc_fixture() -> &'static RagSharedFixture {
    SHARED_REAL_LLM_STANDARD_DOC
        .get_or_init(|| async { cold_real_llm_standard_doc().await })
        .await
}

/// Fresh worker on the current runtime; reuses cold-ingested standard doc + API.
///
/// Prefer this for L3-thin real paths that query the document (RAG / multi-turn / format).
pub async fn shared_standard_doc_real_llm() -> (TestContext, UploadResponse) {
    let fixture = shared_standard_doc_fixture().await;
    let ctx = TestContext::spawn_from_rag_fixture(fixture).await;
    (ctx, fixture.upload.clone())
}
