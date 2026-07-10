//! Persistent 7-document corpus for `realistic_corpus_full_eval`.
//!
//! Keeps PG rows, Milvus vectors, and object-store bytes across `cargo test` reruns
//! (external Postgres + fixed object-store path + module-scoped infra fixture).
//! **Never auto-ingests** — resolves corpus from JSON cache or PG discovery by filename.
//! Based on smoke_v5_corpus.rs.

use std::sync::Arc;

use sqlx::Connection;
use tokio::sync::OnceCell;
use uuid::Uuid;

use super::super::llm_real::load_env_from_repo_dotenv;
use super::super::preflight;
use super::super::setup;
use super::super::test_context::PersistentSmokeInfra;
use super::super::test_context::config::E2eBootstrapConfig;
use super::super::{DEFAULT_TEST_ORG_ID, DEFAULT_TEST_USER_ID, TestContext};

const REALISTIC_NOTEBOOK_NAME: &str = "rag-quality-realistic-corpus";
const REALISTIC_CORPUS: &[(&str, u64)] = &[
    ("thesis_y_refrigeration.txt", 600),
    ("adr-0004-rag-agent-loop.md", 120),
    ("adr-0009-codegen-sandbox-bridge.md", 120),
    ("consulting_platform_network_effects.txt", 300),
    ("consulting_compensation_design.txt", 120),
    ("huawei_ipd_370_activities.txt", 120),
    ("baiyao_it_planning.txt", 300),
];

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RealisticCorpusState {
    pub owner_user_id: String,
    pub user_id: String,
    pub workspace_id: String,
    pub documents: Vec<RealisticDocument>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RealisticDocument {
    pub filename: String,
    pub document_id: String,
}

/// Infra + corpus metadata pinned for the test binary (survives per-test teardown).
pub(crate) struct RealisticCorpusFixture {
    pub corpus: RealisticCorpusState,
    pub(crate) owner_user_id: String,
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

impl RealisticCorpusFixture {
    fn from_context(mut ctx: TestContext, corpus: RealisticCorpusState) -> Self {
        let owner_user_id = ctx.owner_user_id.clone();
        let user_id = ctx.user_id.clone();
        let app_state = ctx
            .app_state
            .take()
            .expect("realistic fixture expects shared AppState");
        let api_base_url = ctx.base_url.clone();
        let worker_bootstrap = ctx
            .bootstrap
            .take()
            .expect("realistic fixture expects bootstrap config");
        let pg_url = ctx.pg_url.clone();
        let shared_pg = ctx
            .shared_pg
            .take()
            .expect("realistic fixture expects shared postgres");
        let shared_milvus = ctx
            .shared_milvus
            .take()
            .expect("realistic fixture expects shared milvus");
        let milvus_url = ctx
            .milvus_url
            .take()
            .expect("realistic fixture expects milvus url");
        let milvus_collection_prefix = ctx
            .milvus_collection_prefix
            .take()
            .expect("realistic fixture expects milvus collection prefix");
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
            owner_user_id,
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

fn realistic_state_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/e2e_output/rag_quality_realistic_corpus.json")
}

fn realistic_object_store_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/e2e_output/rag_quality_realistic_object_store")
}

async fn persistent_infra() -> PersistentSmokeInfra {
    PersistentSmokeInfra {
        postgres_url: setup::resolve_persistent_smoke_postgres_url().await,
        object_store_path: realistic_object_store_path(),
    }
}

fn load_realistic_state() -> Option<RealisticCorpusState> {
    let path = realistic_state_path();
    let raw = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn save_realistic_state(state: &RealisticCorpusState) {
    let path = realistic_state_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let raw = serde_json::to_string_pretty(state).expect("serialize realistic corpus state");
    std::fs::write(&path, raw).expect("write realistic corpus state");
    eprintln!("[realistic] saved corpus cache to {}", path.display());
}

/// `documents`/`chunks`/`notebooks` have FORCE ROW LEVEL SECURITY keyed on
/// `app.current_user`; a raw connection sees zero rows and every doc would look
/// "missing from PG" (the historical phantom-empty-DB / reingest loop).
async fn connect_with_org_context(pg_url: &str, owner_user_id: &str) -> anyhow::Result<sqlx::PgConnection> {
    let mut conn = sqlx::PgConnection::connect(pg_url).await?;
    sqlx::query("select set_config('app.current_user', $1, false)")
        .bind(owner_user_id)
        .execute(&mut conn)
        .await?;
    Ok(conn)
}

fn fail_missing_realistic_corpus(detail: &str) -> ! {
    panic!(
        "realistic corpus unavailable — tests never auto-ingest.\n\
         {detail}\n\
         Fix: ingest the 7 realistic documents manually into notebook \
         '{REALISTIC_NOTEBOOK_NAME}' on the persistent e2e PG, then rerun.\n\
         Cache: {}",
        realistic_state_path().display()
    );
}

async fn validate_realistic_corpus_pg(pg_url: &str, state: &RealisticCorpusState) -> bool {
    let expected = REALISTIC_CORPUS;
    if state.owner_user_id != DEFAULT_TEST_ORG_ID || state.user_id != DEFAULT_TEST_USER_ID {
        eprintln!("[realistic] corpus cache identity mismatch (expected default test org/user)");
        return false;
    }
    if state.documents.len() != expected.len() {
        eprintln!(
            "[realistic] corpus cache has {} docs, expected {}",
            state.documents.len(),
            expected.len()
        );
        return false;
    }
    for (filename, _) in expected {
        if !state.documents.iter().any(|doc| doc.filename == *filename) {
            eprintln!("[realistic] corpus cache missing document {filename}");
            return false;
        }
    }

    let mut conn = match connect_with_org_context(pg_url, &state.owner_user_id).await {
        Ok(conn) => conn,
        Err(err) => {
            eprintln!("[realistic] corpus cache PG connect failed: {err}");
            return false;
        }
    };

    for doc in &state.documents {
        let doc_id = match Uuid::parse_str(&doc.document_id) {
            Ok(id) => id,
            Err(err) => {
                eprintln!(
                    "[realistic] corpus cache doc {} has invalid document_id: {err}",
                    doc.filename
                );
                return false;
            }
        };
        let row: Option<(String, i32)> = sqlx::query_as(
            "SELECT status, chunk_count FROM documents WHERE id = $1",
        )
        .bind(doc_id)
        .fetch_optional(&mut conn)
        .await
        .unwrap_or(None);
        match row {
            Some((status, chunk_count)) if status == "completed" && chunk_count > 0 => {}
            Some((status, chunk_count)) => {
                eprintln!(
                    "[realistic] corpus cache doc {} status={status} chunk_count={chunk_count} (need completed/>0)",
                    doc.filename
                );
                return false;
            }
            None => {
                eprintln!(
                    "[realistic] corpus cache doc {} missing from PG (id={})",
                    doc.filename, doc.document_id
                );
                return false;
            }
        }

        let body_units: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM chunks WHERE document_id = $1 AND chunk_type = 'body'",
        )
        .bind(doc_id)
        .fetch_one(&mut conn)
        .await
        .unwrap_or((0,));
        if body_units.0 == 0 {
            eprintln!(
                "[realistic] corpus cache doc {} has 0 PG body chunks (need >0)",
                doc.filename
            );
            return false;
        }

        let cursor_rows: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM chunks WHERE document_id = $1 AND chunk_type = 'body' AND (metadata->>'cursor') IS NOT NULL",
        )
        .bind(doc_id)
        .fetch_one(&mut conn)
        .await
        .unwrap_or((0,));
        if cursor_rows.0 == 0 {
            eprintln!(
                "[realistic] corpus cache doc {} has 0 body chunks with cursor metadata (need >0 for seq/window enrichment)",
                doc.filename
            );
            return false;
        }
    }
    true
}

async fn discover_realistic_corpus_pg(pg_url: &str) -> Option<RealisticCorpusState> {
    let owner_user_id = Uuid::parse_str(DEFAULT_TEST_ORG_ID).ok()?;
    let mut conn = match connect_with_org_context(pg_url, DEFAULT_TEST_ORG_ID).await {
        Ok(conn) => conn,
        Err(err) => {
            eprintln!("[realistic] PG discovery connect failed: {err}");
            return None;
        }
    };
    let mut documents = Vec::new();
    let mut workspace_id: Option<Uuid> = None;

    for (filename, _) in REALISTIC_CORPUS {
        let row: Option<(Uuid, Uuid)> = sqlx::query_as(
            r#"
            SELECT d.id, d.workspace_id
            FROM documents d
            LEFT JOIN workspaces n ON n.id = d.workspace_id
            WHERE d.owner_user_id = $1
              AND d.file_name = $2
              AND d.status = 'completed'
              AND d.chunk_count > 0
            ORDER BY
              CASE WHEN n.title = $3 THEN 0 ELSE 1 END,
              d.updated_at DESC
            LIMIT 1
            "#,
        )
        .bind(owner_user_id)
        .bind(*filename)
        .bind(REALISTIC_NOTEBOOK_NAME)
        .fetch_optional(&mut conn)
        .await
        .ok()?;

        let (doc_id, nb_id) = match row {
            Some(pair) => pair,
            None => {
                eprintln!("[realistic] PG discovery: no completed doc for {filename}");
                return None;
            }
        };
        if let Some(expected_nb) = workspace_id {
            if expected_nb != nb_id {
                eprintln!(
                    "[realistic] PG discovery: {filename} workspace_id={nb_id} differs from prior {expected_nb} (using first notebook for chat scope)"
                );
            }
        } else {
            workspace_id = Some(nb_id);
        }
        documents.push(RealisticDocument {
            filename: (*filename).to_string(),
            document_id: doc_id.to_string(),
        });
    }

    let workspace_id = workspace_id?;
    Some(RealisticCorpusState {
        owner_user_id: DEFAULT_TEST_ORG_ID.to_string(),
        user_id: DEFAULT_TEST_USER_ID.to_string(),
        workspace_id: workspace_id.to_string(),
        documents,
    })
}

async fn resolve_realistic_corpus(pg_url: &str) -> RealisticCorpusState {
    if let Some(state) = load_realistic_state() {
        if validate_realistic_corpus_pg(pg_url, &state).await {
            eprintln!(
                "[realistic] reusing cached corpus (workspace_id={}, {} docs)",
                state.workspace_id,
                state.documents.len()
            );
            return state;
        }
        eprintln!("[realistic] cached corpus stale — trying PG discovery by filename");
    } else {
        eprintln!("[realistic] no corpus cache — trying PG discovery by filename");
    }

    if let Some(state) = discover_realistic_corpus_pg(pg_url).await {
        if validate_realistic_corpus_pg(pg_url, &state).await {
            eprintln!(
                "[realistic] discovered corpus in PG (workspace_id={}, {} docs) — refreshed cache",
                state.workspace_id,
                state.documents.len()
            );
            save_realistic_state(&state);
            return state;
        }
        fail_missing_realistic_corpus(
            "PG has documents matching filenames but they fail validation (status/chunks/cursor).",
        );
    }

    fail_missing_realistic_corpus(
        "JSON cache missing or stale, and PG has no completed 7-document realistic corpus.",
    );
}

async fn cold_realistic_fixture() -> RealisticCorpusFixture {
    load_env_from_repo_dotenv();
    unsafe {
        std::env::set_var("AVRAG_INGESTION_QUEUE_GROUP", "e2e-realistic");
        std::env::set_var("E2E_PRESERVE_MILVUS_ON_DROP", "1");
    }
    preflight::assert_no_external_workers();
    if std::env::var("RAG_QUALITY_REALISTIC_TRIPLET_ENABLED")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    {
        unsafe {
            std::env::set_var("INGESTION_TRIPLET_ENABLED", "1");
        }
        eprintln!("[realistic] RAG_QUALITY_REALISTIC_TRIPLET_ENABLED=1 — graph triplet extraction enabled for ingest");
    }
    let infra = persistent_infra().await;
    preflight::assert_smoke_database_isolated(&infra.postgres_url);
    eprintln!(
        "[realistic] persistent infra: pg={} object_store={}",
        infra.postgres_url,
        infra.object_store_path.display()
    );

    let corpus = resolve_realistic_corpus(&infra.postgres_url).await;

    let identity = Some((
        DEFAULT_TEST_ORG_ID.to_string(),
        DEFAULT_TEST_USER_ID.to_string(),
    ));
    let ctx = TestContext::new_with_real_llm_pdf_persistent_corpus(identity, &infra).await;
    RealisticCorpusFixture::from_context(ctx, corpus)
}

static SHARED_REALISTIC_FIXTURE: OnceCell<RealisticCorpusFixture> = OnceCell::const_new();

/// Module-scoped ingested corpus + infra (one cold path per test binary).
pub(crate) async fn shared_realistic_fixture() -> &'static RealisticCorpusFixture {
    SHARED_REALISTIC_FIXTURE
        .get_or_init(cold_realistic_fixture)
        .await
}

/// Fresh API + worker on the current tokio runtime, reusing persistent realistic infra.
pub async fn shared_realistic_context() -> (&'static RealisticCorpusFixture, TestContext) {
    let fixture = shared_realistic_fixture().await;
    let ctx = TestContext::spawn_from_realistic_fixture(fixture).await;
    (fixture, ctx)
}
