//! Shared RAG-ready fixture: ingested `antifragile.txt` + reusable infra.
//!
//! `#[tokio::test]` shuts down per-test runtimes, so API/worker tasks must respawn each
//! test. [`shared_rag_fixture`] keeps PG/Milvus/object-store, mock endpoints, one
//! [`app::AppState`], and upload metadata for the test binary.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize};

use tokio::sync::OnceCell;

use super::super::test_context::config::E2eBootstrapConfig;
use super::super::{DocumentStatus, TestContext, UploadResponse};
use super::super::setup;

/// Infra captured after one cold upload + ingestion (lives for the test binary).
pub(crate) struct RagSharedFixture {
    pub upload: UploadResponse,
    pub(crate) org_id: String,
    pub(crate) user_id: String,
    pub(crate) app_state: Arc<app::AppState>,
    pub(crate) pg_url: String,
    pub(crate) shared_pg: Arc<setup::SharedPostgres>,
    pub(crate) milvus_url: String,
    pub(crate) shared_milvus: Arc<setup::SharedMilvus>,
    pub(crate) milvus_collection_prefix: String,
    pub(crate) object_root: String,
    pub(crate) api_base_url: String,
    pub(crate) worker_bootstrap: E2eBootstrapConfig,
    pub(crate) search_should_429: Option<Arc<AtomicBool>>,
    pub(crate) embedding_should_503: Option<Arc<AtomicBool>>,
    pub(crate) embedding_call_count: Option<Arc<AtomicUsize>>,
    object_store_guard: Arc<tempfile::TempDir>,
}

impl RagSharedFixture {
    pub(crate) fn from_context(mut ctx: TestContext, upload: UploadResponse) -> Self {
        let org_id = ctx.org_id.clone();
        let user_id = ctx.user_id.clone();
        let app_state = ctx
            .app_state
            .take()
            .expect("rag fixture expects shared AppState");
        let api_base_url = ctx.base_url.clone();
        let worker_bootstrap = ctx
            .bootstrap
            .take()
            .expect("rag fixture expects bootstrap config");
        let pg_url = ctx.pg_url.clone();
        let shared_pg = ctx
            .shared_pg
            .take()
            .expect("rag fixture expects shared postgres");
        let shared_milvus = ctx
            .shared_milvus
            .take()
            .expect("rag fixture expects shared milvus");
        let milvus_url = ctx
            .milvus_url
            .take()
            .expect("rag fixture expects milvus url");
        let milvus_collection_prefix = ctx
            .milvus_collection_prefix
            .take()
            .expect("rag fixture expects milvus collection prefix");
        let search_should_429 = ctx.search_should_429.take();
        let embedding_should_503 = ctx.embedding_should_503.take();
        let embedding_call_count = ctx.embedding_call_count.take();
        let object_store_guard = Arc::new(std::mem::replace(
            &mut ctx.object_store_dir,
            tempfile::tempdir().expect("placeholder tempdir"),
        ));
        let object_root = object_store_guard.path().to_string_lossy().into_owned();

        ctx.worker = None;
        // Keep persistent HTTP tasks alive: dropping oneshot senders closes the channel
        // and unblocks the `abort_rx` branch in spawned servers.
        if let Some(tx) = ctx.server_abort.take() {
            std::mem::forget(tx);
        }
        if let Some(tx) = ctx.mock_llm_abort.take() {
            std::mem::forget(tx);
        }
        if let Some(tx) = ctx.mock_embedding_abort.take() {
            std::mem::forget(tx);
        }
        if let Some(tx) = ctx.mock_search_abort.take() {
            std::mem::forget(tx);
        }
        std::mem::forget(ctx);

        Self {
            upload,
            org_id,
            user_id,
            app_state,
            pg_url,
            shared_pg,
            milvus_url,
            shared_milvus,
            milvus_collection_prefix,
            object_root,
            api_base_url,
            worker_bootstrap,
            search_should_429,
            embedding_should_503,
            embedding_call_count,
            object_store_guard,
        }
    }
}

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

static SHARED_RAG_FIXTURE: OnceCell<RagSharedFixture> = OnceCell::const_new();

/// Module-scoped ingested document + infra (one cold path per test binary).
pub async fn shared_rag_fixture() -> &'static RagSharedFixture {
    SHARED_RAG_FIXTURE
        .get_or_init(|| async {
            let (ctx, upload) = ready_rag_context().await;
            RagSharedFixture::from_context(ctx, upload)
        })
        .await
}

/// Fresh API + worker on the current tokio runtime, reusing shared RAG infra.
pub async fn shared_ready_rag_context() -> TestContext {
    TestContext::spawn_from_rag_fixture(shared_rag_fixture().await).await
}
