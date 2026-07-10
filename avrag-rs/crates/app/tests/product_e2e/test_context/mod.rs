//! TestContext bootstrap and HTTP helpers for Product E2E.
//!
//! Split by profile/builder — see [`profiles`] and [`builder`].

mod artifacts;
mod builder;
pub(crate) mod config;
mod diagnostics;
mod http;
mod profiles;

pub(crate) use diagnostics::dump_ingestion_failure_diagnostics;
pub(crate) use http::local_dev_email;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use tokio::sync::oneshot::Sender;

pub(crate) use builder::PersistentSmokeInfra;
pub use profiles::ChatStreamParams;

use super::setup;

/// Per-test execution context.
///
/// Created via profile constructors in [`profiles`]. Automatically cleans up on
/// drop (containers, temp dirs, worker process, HTTP server, mock servers).
pub struct TestContext {
    pub http_client: reqwest::Client,
    pub base_url: String,
    pub(crate) owner_user_id: String,
    pub(crate) user_id: String,
    pub(crate) app_state: Option<Arc<app::AppState>>,
    pub(crate) bootstrap: Option<config::E2eBootstrapConfig>,
    pub(crate) shared_pg: Option<Arc<setup::SharedPostgres>>,
    pub(crate) shared_milvus: Option<Arc<setup::SharedMilvus>>,
    pub(crate) milvus_url: Option<String>,
    pub(crate) milvus_collection_prefix: Option<String>,
    pub(crate) worker: Option<tokio::process::Child>,
    pub(crate) server_abort: Option<Sender<()>>,
    #[allow(dead_code)]
    pub(crate) object_store_dir: tempfile::TempDir,
    pub(crate) object_root: String,
    pub(crate) pg_url: String,
    pub(crate) mock_llm_abort: Option<Sender<()>>,
    pub(crate) mock_embedding_abort: Option<Sender<()>>,
    pub(crate) mock_search_abort: Option<Sender<()>>,
    pub(crate) mock_paddle_abort: Option<Sender<()>>,
    pub(crate) mock_paddle_jobs_submitted: Option<Arc<AtomicUsize>>,
    pub(crate) mock_office_abort: Option<Sender<()>>,
    pub(crate) search_controls: Option<crate::product_e2e::mock_servers::MockSearchControls>,
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
        if let Some(tx) = self.mock_paddle_abort.take() {
            let _ = tx.send(());
        }
        if let Some(tx) = self.mock_office_abort.take() {
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
    use super::builder::{pg_url_mark_migrated, pg_url_needs_migration};

    #[test]
    fn milvus_collection_prefix_uses_context_identity_suffix() {
        let prefix = milvus_collection_prefix_for_identity("12345678-aaaa-bbbb-cccc-dddddddddddd");

        assert_eq!(prefix, "avrag_e2e_12345678");
    }

    #[test]
    fn pg_url_migration_dedup_uses_cross_process_marker() {
        let temp = tempfile::tempdir().expect("temp pg migrated dir");
        unsafe {
            std::env::set_var(
                "AVRAG_E2E_PG_MIGRATED_DIR",
                temp.path().to_string_lossy().as_ref(),
            );
            std::env::set_var("AVRAG_E2E_PG_MIGRATION_WAIT_SECS", "0");
        }

        let database_url = "postgres://e2e:secret@127.0.0.1:5432/dedup_test";
        assert!(
            pg_url_needs_migration(database_url),
            "first caller should claim migration"
        );
        assert!(
            !pg_url_needs_migration(database_url),
            "second caller should wait and skip while lock is held"
        );

        pg_url_mark_migrated(database_url);
        assert!(
            !pg_url_needs_migration(database_url),
            "marker should suppress migration for later callers"
        );
    }
}
