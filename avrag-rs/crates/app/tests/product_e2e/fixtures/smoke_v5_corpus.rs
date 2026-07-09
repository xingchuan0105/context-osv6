//! Persistent 3-document corpus for `rag_system_prompt_smoke_v5`.
//!
//! Keeps PG rows, Milvus vectors, and object-store bytes across `cargo test` reruns
//! (external Postgres + fixed object-store path + module-scoped infra fixture).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::OnceCell;

use super::super::http_helpers::milvus_collection_prefix_for_identity;
use super::super::llm_real::load_env_from_repo_dotenv;
use super::super::preflight;
use super::super::setup;
use super::super::test_context::PersistentSmokeInfra;
use super::super::test_context::config::E2eBootstrapConfig;
use super::super::test_context::dump_ingestion_failure_diagnostics;
use super::super::{DEFAULT_TEST_ORG_ID, DEFAULT_TEST_USER_ID, DocumentStatus, TestContext};

const SMOKE_V5_NOTEBOOK_NAME: &str = "rag-system-prompt-smoke-v5";
const SMOKE_V5_CORPUS: &[(&str, u64)] = &[
    ("thesis_y_refrigeration.txt", 600),
    ("huawei_ipd_370_activities.txt", 120),
    ("baiyao_it_planning.txt", 300),
];

fn smoke_v5_ingest_timeout_secs(base: u64) -> u64 {
    if let Ok(raw) = std::env::var("RAG_SMOKE_INGEST_TIMEOUT_SECS") {
        if let Ok(secs) = raw.trim().parse::<u64>() {
            if secs > 0 {
                return secs;
            }
        }
    }
    if std::env::var("INGESTION_TRIPLET_ENABLED")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    {
        base.saturating_mul(3)
    } else {
        base
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SmokeV5CorpusState {
    pub org_id: String,
    pub user_id: String,
    pub workspace_id: String,
    pub documents: Vec<SmokeV5Document>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SmokeV5Document {
    pub filename: String,
    pub document_id: String,
}

/// Infra + corpus metadata pinned for the test binary (survives per-test teardown).
pub(crate) struct SmokeV5CorpusFixture {
    pub corpus: SmokeV5CorpusState,
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
    #[allow(dead_code)]
    _object_store_guard: Arc<tempfile::TempDir>,
}

impl SmokeV5CorpusFixture {
    fn from_context(mut ctx: TestContext, corpus: SmokeV5CorpusState) -> Self {
        let org_id = ctx.org_id.clone();
        let user_id = ctx.user_id.clone();
        let app_state = ctx
            .app_state
            .take()
            .expect("smoke v5 fixture expects shared AppState");
        let api_base_url = ctx.base_url.clone();
        let worker_bootstrap = ctx
            .bootstrap
            .take()
            .expect("smoke v5 fixture expects bootstrap config");
        let pg_url = ctx.pg_url.clone();
        let shared_pg = ctx
            .shared_pg
            .take()
            .expect("smoke v5 fixture expects shared postgres");
        let shared_milvus = ctx
            .shared_milvus
            .take()
            .expect("smoke v5 fixture expects shared milvus");
        let milvus_url = ctx
            .milvus_url
            .take()
            .expect("smoke v5 fixture expects milvus url");
        let milvus_collection_prefix = ctx
            .milvus_collection_prefix
            .take()
            .expect("smoke v5 fixture expects milvus collection prefix");
        let _object_store_guard = Arc::new(std::mem::replace(
            &mut ctx.object_store_dir,
            tempfile::tempdir().expect("placeholder tempdir"),
        ));
        let object_root = worker_bootstrap.object_root.clone();

        ctx.worker = None;
        if let Some(tx) = ctx.server_abort.take() {
            std::mem::forget(tx);
        }
        std::mem::forget(ctx);

        Self {
            corpus,
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
            _object_store_guard,
        }
    }
}

fn smoke_v5_state_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/e2e_output/rag_quality_smoke_v5_corpus.json")
}

fn smoke_v5_object_store_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/e2e_output/rag_quality_smoke_v5_object_store")
}

fn smoke_v5_force_ingest() -> bool {
    std::env::var("RAG_QUALITY_SMOKE_FORCE_INGEST")
        .ok()
        .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
}

async fn persistent_infra() -> PersistentSmokeInfra {
    PersistentSmokeInfra {
        postgres_url: setup::resolve_persistent_smoke_postgres_url().await,
        object_store_path: smoke_v5_object_store_path(),
    }
}

fn load_smoke_v5_state() -> Option<SmokeV5CorpusState> {
    let path = smoke_v5_state_path();
    let raw = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn save_smoke_v5_state(state: &SmokeV5CorpusState) {
    let path = smoke_v5_state_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let raw = serde_json::to_string_pretty(state).expect("serialize smoke v5 corpus state");
    std::fs::write(&path, raw).expect("write smoke v5 corpus state");
    eprintln!("[smoke_v5] saved corpus cache to {}", path.display());
}

async fn validate_smoke_v5_corpus(ctx: &TestContext, state: &SmokeV5CorpusState) -> bool {
    let expected = smoke_v5_corpus_entries();
    if state.org_id != DEFAULT_TEST_ORG_ID || state.user_id != DEFAULT_TEST_USER_ID {
        eprintln!("[smoke_v5] corpus cache identity mismatch (expected default test org/user)");
        return false;
    }
    if state.documents.len() != expected.len() {
        eprintln!(
            "[smoke_v5] corpus cache has {} docs, expected {}",
            state.documents.len(),
            expected.len()
        );
        return false;
    }
    for (filename, _) in &expected {
        if !state.documents.iter().any(|doc| doc.filename == *filename) {
            eprintln!("[smoke_v5] corpus cache missing document {filename}");
            return false;
        }
    }
    for doc in &state.documents {
        let status = match ctx.fetch_document_status(&doc.document_id).await {
            Ok(body) => body["status"].as_str().unwrap_or("unknown").to_string(),
            Err(err) => {
                eprintln!(
                    "[smoke_v5] corpus cache doc {} status fetch failed: {err}",
                    doc.filename
                );
                return false;
            }
        };
        if status != "completed" {
            eprintln!(
                "[smoke_v5] corpus cache doc {} status={status} (need completed)",
                doc.filename
            );
            return false;
        }
    }
    true
}

async fn ingest_smoke_v5_corpus(ctx: &mut TestContext) -> SmokeV5CorpusState {
    let corpus_entries = smoke_v5_corpus_entries();
    assert!(
        !corpus_entries.is_empty(),
        "RAG_SMOKE_SINGLE_DOC must match a smoke_v5 corpus filename"
    );

    let notebook = ctx
        .create_notebook(SMOKE_V5_NOTEBOOK_NAME)
        .await
        .expect("create notebook");
    let mut documents = Vec::new();

    for (filename, timeout_secs) in &corpus_entries {
        let timeout_secs = smoke_v5_ingest_timeout_secs(*timeout_secs);
        eprintln!("[smoke_v5] uploading {filename} (timeout={timeout_secs}s) ...");
        let upload = ctx
            .upload_document_to_notebook(filename, &notebook.id)
            .await
            .unwrap_or_else(|e| panic!("upload {filename}: {e}"));
        let status = match ctx
            .wait_for_ingestion(&upload.document_id, Duration::from_secs(timeout_secs))
            .await
        {
            Ok(status) => status,
            Err(error) => {
                dump_ingestion_failure_diagnostics(ctx, &upload.document_id).await;
                panic!("wait_for_ingestion {filename}: {error}");
            }
        };
        if status != DocumentStatus::Completed {
            dump_ingestion_failure_diagnostics(ctx, &upload.document_id).await;
            panic!("ingestion failed for {filename}: status={status:?}");
        }
        eprintln!(
            "[smoke_v5] {filename} ingested (doc_id={})",
            upload.document_id
        );
        documents.push(SmokeV5Document {
            filename: (*filename).to_string(),
            document_id: upload.document_id,
        });
    }

    SmokeV5CorpusState {
        org_id: DEFAULT_TEST_ORG_ID.to_string(),
        user_id: DEFAULT_TEST_USER_ID.to_string(),
        workspace_id: notebook.id,
        documents,
    }
}

fn smoke_v5_corpus_entries() -> Vec<(&'static str, u64)> {
    if let Ok(spec) = std::env::var("RAG_SMOKE_SINGLE_DOC") {
        let name = spec.trim();
        if !name.is_empty() {
            let filtered: Vec<_> = SMOKE_V5_CORPUS
                .iter()
                .copied()
                .filter(|(filename, _)| *filename == name)
                .collect();
            if !filtered.is_empty() {
                eprintln!("[smoke_v5] RAG_SMOKE_SINGLE_DOC={name} -> {} file(s)", filtered.len());
                return filtered;
            }
            eprintln!("[smoke_v5] RAG_SMOKE_SINGLE_DOC={name} not in corpus — using full set");
        }
    }
    SMOKE_V5_CORPUS.to_vec()
}

async fn ensure_smoke_v5_corpus(ctx: &mut TestContext) -> SmokeV5CorpusState {
    if smoke_v5_force_ingest() {
        eprintln!("[smoke_v5] RAG_QUALITY_SMOKE_FORCE_INGEST=1 — re-ingesting corpus");
        let state = ingest_smoke_v5_corpus(ctx).await;
        save_smoke_v5_state(&state);
        return state;
    }

    if let Some(state) = load_smoke_v5_state() {
        if validate_smoke_v5_corpus(ctx, &state).await {
            eprintln!(
                "[smoke_v5] reusing cached corpus (workspace_id={}, {} docs)",
                state.workspace_id,
                state.documents.len()
            );
            return state;
        }
        eprintln!("[smoke_v5] cached corpus invalid — re-ingesting");
    } else {
        eprintln!("[smoke_v5] no corpus cache — ingesting");
    }

    let state = ingest_smoke_v5_corpus(ctx).await;
    save_smoke_v5_state(&state);
    state
}

async fn cold_smoke_v5_fixture() -> SmokeV5CorpusFixture {
    load_env_from_repo_dotenv();
    unsafe {
        std::env::set_var("AVRAG_INGESTION_QUEUE_GROUP", "e2e-smoke");
    }
    preflight::assert_no_external_workers();
    if std::env::var("RAG_QUALITY_SMOKE_TRIPLET_ENABLED")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    {
        unsafe {
            std::env::set_var("INGESTION_TRIPLET_ENABLED", "1");
        }
        eprintln!("[smoke_v5] RAG_QUALITY_SMOKE_TRIPLET_ENABLED=1 — graph triplet extraction enabled for ingest");
    }
    let infra = persistent_infra().await;
    preflight::assert_smoke_database_isolated(&infra.postgres_url);
    eprintln!(
        "[smoke_v5] persistent infra: pg={} object_store={}",
        infra.postgres_url,
        infra.object_store_path.display()
    );

    if smoke_v5_force_ingest() {
        let prefix = milvus_collection_prefix_for_identity(DEFAULT_TEST_ORG_ID);
        eprintln!(
            "[smoke_v5] pre-worker drop Milvus collections prefix={prefix} (BM25 / graph re-index)"
        );
        setup::drop_milvus_collections(&prefix).await;
    }

    let identity = Some((
        DEFAULT_TEST_ORG_ID.to_string(),
        DEFAULT_TEST_USER_ID.to_string(),
    ));
    let mut ctx = TestContext::new_with_real_llm_pdf_persistent_corpus(identity, &infra).await;
    let corpus = ensure_smoke_v5_corpus(&mut ctx).await;
    SmokeV5CorpusFixture::from_context(ctx, corpus)
}

static SHARED_SMOKE_V5_FIXTURE: OnceCell<SmokeV5CorpusFixture> = OnceCell::const_new();

/// Module-scoped ingested corpus + infra (one cold path per test binary).
pub(crate) async fn shared_smoke_v5_fixture() -> &'static SmokeV5CorpusFixture {
    SHARED_SMOKE_V5_FIXTURE
        .get_or_init(cold_smoke_v5_fixture)
        .await
}

/// Fresh API + worker on the current tokio runtime, reusing persistent smoke-v5 infra.
pub async fn shared_smoke_v5_context() -> (&'static SmokeV5CorpusFixture, TestContext) {
    let fixture = shared_smoke_v5_fixture().await;
    let ctx = TestContext::spawn_from_smoke_v5_fixture(fixture).await;
    (fixture, ctx)
}
