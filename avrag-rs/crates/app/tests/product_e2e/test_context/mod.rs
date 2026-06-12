//! TestContext bootstrap and HTTP helpers for Product E2E.
//!
//! Split by profile/builder — see [`profiles`] and [`builder`].

mod artifacts;
mod builder;
mod http;
mod profiles;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use tokio::sync::oneshot::Sender;

pub use profiles::ChatStreamParams;

use super::setup;

/// Per-test execution context.
///
/// Created via profile constructors in [`profiles`]. Automatically cleans up on
/// drop (containers, temp dirs, worker process, HTTP server, mock servers).
pub struct TestContext {
    pub http_client: reqwest::Client,
    pub base_url: String,
    pub(crate) shared_pg: Option<Arc<setup::SharedPostgres>>,
    pub(crate) shared_milvus: Option<Arc<setup::SharedMilvus>>,
    pub(crate) milvus_collection_prefix: Option<String>,
    pub(crate) worker: Option<tokio::process::Child>,
    pub(crate) server_abort: Option<Sender<()>>,
    #[allow(dead_code)]
    pub(crate) object_store_dir: tempfile::TempDir,
    pub(crate) pg_url: String,
    pub(crate) mock_llm_abort: Option<Sender<()>>,
    pub(crate) mock_embedding_abort: Option<Sender<()>>,
    pub(crate) mock_search_abort: Option<Sender<()>>,
    pub(crate) search_should_429: Option<Arc<AtomicBool>>,
    pub(crate) embedding_should_503: Option<Arc<AtomicBool>>,
    pub(crate) embedding_call_count: Option<Arc<AtomicUsize>>,
    pub(crate) redis_container_name: Option<String>,
    pub(crate) worker_log_path: Option<std::path::PathBuf>,
    pub(crate) artifact_run_id: String,
}

impl Drop for TestContext {
    fn drop(&mut self) {
        if let Some(mut worker) = self.worker.take() {
            let _ = worker.start_kill();
        }
        if let Some(tx) = self.server_abort.take() {
            let _ = tx.send(());
        }
        if let Some(tx) = self.mock_llm_abort.take() {
            let _ = tx.send(());
        }
        if let Some(tx) = self.mock_embedding_abort.take() {
            let _ = tx.send(());
        }
        if let Some(tx) = self.mock_search_abort.take() {
            let _ = tx.send(());
        }
        if let Some(pg) = self.shared_pg.take() {
            setup::release_shared_postgres(&pg);
        }
        if let Some(ref prefix) = self.milvus_collection_prefix {
            setup::sync_drop_milvus_collections(prefix);
        }
        if let Some(milvus) = self.shared_milvus.take() {
            setup::release_shared_milvus(&milvus);
        }
        if let Some(ref container) = self.redis_container_name {
            setup::sync_stop_redis(container);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::http_helpers::milvus_collection_prefix_for_identity;

    #[test]
    fn milvus_collection_prefix_uses_context_identity_suffix() {
        let prefix =
            milvus_collection_prefix_for_identity("12345678-aaaa-bbbb-cccc-dddddddddddd");

        assert_eq!(prefix, "avrag_e2e_12345678");
    }
}
