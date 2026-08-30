//! LLM ingest benchmark (test-only, `#[cfg(test)]`).
//!
//! Boundary: one file → the PRODUCTION windowed PS+triplet stage
//! (`pipeline::windowed_llm::run_windowed_ps_and_triplets`, markitdown parse
//! included) → stage timings + the actual artifacts (doc summary, triplets,
//! toc, profile metadata) written to `BENCH_OUT_DIR`. Nothing else: no HTTP
//! server, no worker subprocess, no queue, no embeddings, no Milvus, no
//! retrieval probe. The pipeline's own PG side effects (toc/profile writes)
//! degrade to skips when the guard rejects the synthetic task — the bench
//! needs only the LLM endpoint and `markitdown` on PATH.
//!
//! Switching LLM = `INGESTION_LLM_*` env only (zero rebuild).
//!
//! Run:
//! ```bash
//! cd avrag-rs && set -a && source .env && set +a && \
//!   INGESTION_LLM_MODEL=qwen3.8-flash BENCH_OUT_DIR=/tmp/bench/qwen38 \
//!   cargo test -p avrag-worker --lib llm_bench -- --ignored --nocapture
//! ```
//! Or via `scripts/run_llm_ingest_bench.sh` (multi-model + summary table).
#![cfg(test)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use avrag_llm::ModelProviderConfig;
use avrag_storage_pg::{ObjectStoreHandle, PgAppRepository};
use contracts::auth_runtime::{AuthContext, SubjectKind, UserId};
use ingestion::{IngestDocumentPayload, IngestionTask, IngestionTaskKind, IngestionTaskPayload};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use super::document_pipeline::ParseRunState;
use super::processor::{EmbeddingDeps, LlmDeps, MeteringDeps, PgTaskProcessor, StorageDeps};
use super::window_split::split_document_windows;
use super::windowed_llm::run_windowed_ps_and_triplets;

const DEFAULT_DB_URL: &str = "postgres://avrag:avrag@127.0.0.1:5432/avrag_rs_e2e_smoke";
// Synthetic tenant identity matching the e2e layout (object store owner root).
const OWNER_UUID: &str = "00000000-0000-0000-0000-000000000001";
const USER_UUID: &str = "00000000-0000-0000-0000-000000000002";

fn env_opt(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

fn env_flag(key: &str) -> bool {
    env_opt(key).is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

fn provider_config_from_env() -> ModelProviderConfig {
    ModelProviderConfig {
        base_url: env_opt("INGESTION_LLM_BASE_URL").expect("INGESTION_LLM_BASE_URL"),
        api_key: env_opt("INGESTION_LLM_API_KEY").unwrap_or_default(),
        model: env_opt("INGESTION_LLM_MODEL").unwrap_or_else(|| "qwen3.7-flash".into()),
        timeout_ms: env_opt("INGESTION_LLM_TIMEOUT_MS")
            .and_then(|v| v.parse().ok())
            .unwrap_or(180_000),
        api_style: env_opt("INGESTION_LLM_API_STYLE")
            .and_then(|s| avrag_llm::ApiStyle::from_config_str(&s)),
        dimensions: None,
        enable_thinking: Some(env_flag("INGESTION_LLM_ENABLE_THINKING")),
        enable_cache: None,
        rpm_limit: None,
        tpm_limit: None,
    }
}

#[tokio::test]
#[ignore = "real LLM bench; run via scripts/run_llm_ingest_bench.sh or with INGESTION_LLM_* env"]
async fn bench_windowed_summary_triplets() {
    let model = env_opt("INGESTION_LLM_MODEL").unwrap_or_else(|| "qwen3.7-flash".into());
    let input = env_opt("BENCH_FILE").unwrap_or_else(|| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/llm_bench/huawei_ipd_370_activities.txt"
        )
        .to_string()
    });
    let out_dir = PathBuf::from(
        env_opt("BENCH_OUT_DIR")
            .unwrap_or_else(|| format!("{}/avrag_llm_bench/{model}", std::env::temp_dir().display())),
    );
    std::fs::create_dir_all(&out_dir).expect("create BENCH_OUT_DIR");
    let filename = input
        .rsplit('/')
        .next()
        .unwrap_or("input.txt")
        .to_string();

    // --- parse stage: the production markitdown route (pub lib fn) ---
    let bytes = std::fs::read(&input).unwrap_or_else(|e| panic!("read {input}: {e}"));
    let document_id = Uuid::new_v4();
    let t_parse = Instant::now();
    let (_ir, markdown) =
        ingestion::parser::parse_markitdown_document_ir(document_id, &filename, &bytes)
            .await
            .expect("markitdown parse");
    let parse_secs = t_parse.elapsed().as_secs_f64();
    let raw_text = markdown;
    let window_count = split_document_windows(&raw_text).len();

    // --- minimal processor: only the LLM stage is exercised; PG side effects
    // (toc/profile) are attempted through a lazy pool and degrade to skips
    // when the synthetic task fails the guard. ---
    let db_url = env_opt("BENCH_DATABASE_URL").unwrap_or_else(|| DEFAULT_DB_URL.to_string());
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect_lazy(&db_url)
        .expect("pg pool");
    let llm = Arc::new(avrag_llm::LlmClient::new(provider_config_from_env()));
    let processor = PgTaskProcessor {
        storage: StorageDeps {
            repo: PgAppRepository::from_pool(pool),
            object_store: ObjectStoreHandle::local(out_dir.join("objects")),
            retrieval_data_plane: None,
            asset_url_ttl_secs: 3600,
            redis_lock: None,
        },
        embedding: EmbeddingDeps {
            embedding_dim: 1024,
            embedding_client: None,
            mm_embedding_client: None,
        },
        llm: LlmDeps {
            ingestion_llm: Some(llm),
            completion_cache: None,
        },
        metering: MeteringDeps {
            analytics: None,
            usage_limit: None,
            task_usage_observer: None,
            wallet: None,
            provider_secrets: None,
        },
        task_timeout_secs: 900,
    };

    let owner: Uuid = OWNER_UUID.parse().expect("owner uuid");
    let user: Uuid = USER_UUID.parse().expect("user uuid");
    let workspace = Uuid::new_v4();
    let context = AuthContext::new(UserId::new(user), SubjectKind::User)
        .with_workspace_scope(workspace);
    let task = IngestionTask {
        task_id: Uuid::new_v4().to_string(),
        kind: IngestionTaskKind::IngestDocument,
        owner_user_id: owner.to_string(),
        workspace_id: workspace.to_string(),
        document_id: document_id.to_string(),
        requested_by: None,
        idempotency_key: format!("llm-bench-{document_id}"),
        enqueued_at: String::new(),
        payload: IngestionTaskPayload::IngestDocument(IngestDocumentPayload {
            source_uri: input.clone(),
            object_path: format!("bench/{document_id}/{filename}"),
            mime_type: "text/plain".to_string(),
            filename: filename.clone(),
            file_size: bytes.len() as u64,
        }),
        lock_token: None,
        attempt_count: 0,
        max_attempts: 3,
    };

    // --- the production windowed PS + triplet stage ---
    let mut parse_run_state = ParseRunState::default();
    let t_llm = Instant::now();
    let result = run_windowed_ps_and_triplets(
        &processor,
        &context,
        &task,
        document_id,
        workspace,
        &filename,
        &filename,
        &raw_text,
        &mut parse_run_state,
    )
    .await;
    let llm_secs = t_llm.elapsed().as_secs_f64();

    // --- artifacts ---
    std::fs::write(out_dir.join("summary.md"), &result.summary_text).expect("write summary");
    if let Some(meta) = &result.profile_metadata {
        if let Ok(json) = serde_json::to_string_pretty(meta) {
            let _ = std::fs::write(out_dir.join("profile_metadata.json"), json);
        }
    }
    let triplets_json = serde_json::json!({
        "model": model,
        "total_tokens": result.triplets.total_tokens,
        "count": result.triplets.triplets.len(),
        "triplets": result
            .triplets
            .triplets
            .iter()
            .map(|t| serde_json::json!({
                "subject": t.subject,
                "predicate": t.predicate,
                "object": t.object,
                "confidence": t.confidence,
                "source": t.source,
            }))
            .collect::<Vec<_>>(),
    });
    std::fs::write(
        out_dir.join("triplets.json"),
        serde_json::to_string_pretty(&triplets_json).expect("serialize triplets"),
    )
    .expect("write triplets");

    let bench = serde_json::json!({
        "model": model,
        "input_file": input,
        "file_bytes": bytes.len(),
        "windows": window_count,
        "parse_secs": parse_secs,
        "llm_secs": llm_secs,
        "prompt_tokens": result.prompt_tokens,
        "completion_tokens": result.completion_tokens,
        "triplet_count": result.triplets.triplets.len(),
        "summary_chars": result.summary_text.chars().count(),
        "toc_entries": result.toc_entries.len(),
        "entity_count": parse_run_state.outputs.entity_count,
        "relation_count": parse_run_state.outputs.relation_count,
        "graph_degrade_count": parse_run_state.outputs.graph_degrade_count,
        "graph_degrade_reasons": parse_run_state.outputs.graph_degrade_reasons,
    });
    std::fs::write(
        out_dir.join("timings.json"),
        serde_json::to_string_pretty(&bench).expect("serialize timings"),
    )
    .expect("write timings");
    eprintln!("BENCH_RESULT={}", bench);

    // When the triplet turn failed inside the pipeline, the stored degrade
    // reason only keeps the outermost anyhow context. Reproduce the exact
    // seed→produce flow here (same temperatures as the pipeline) and print the
    // FULL error chain; on success, write the artifacts the pipeline missed.
    if result.triplets.triplets.is_empty() {
        use super::ingestion_session::{DocumentIngestionSession, compose_window_system};
        use super::helpers::{PROFILE_SEED_TEMPERATURE, TRIPLET_TEMPERATURE};
        const PS_JOINT_PROMPT: &str =
            include_str!("../../../../prompts/pipeline/profile-summary.joint.md");
        const PS_USER: &str = include_str!("../../../../prompts/templates/profile-summary-user.tmpl");
        const TRIPLET_PROMPT: &str =
            include_str!("../../../../prompts/pipeline/triplet-extraction.system.md");
        const TRIPLET_USER: &str =
            include_str!("../../../../prompts/templates/triplet-extraction-user.tmpl");
        let window = split_document_windows(&raw_text)
            .into_iter()
            .next()
            .unwrap_or_else(|| raw_text.clone());
        let system = compose_window_system(&window);
        let mut session = DocumentIngestionSession::new(processor.llm.ingestion_llm.clone().expect("llm"));
        let diag_seed = match session
            .seed(&system, &format!("{PS_JOINT_PROMPT}\n\n{PS_USER}"), Some(PROFILE_SEED_TEMPERATURE))
            .await
        {
            Ok(turn) => {
                eprintln!(
                    "BENCH_DIAG seed ok: prompt={} compl={}",
                    turn.usage.prompt_tokens, turn.usage.completion_tokens
                );
                turn
            }
            Err(error) => {
                eprintln!("BENCH_DIAG seed FAILED: {error:#}");
                return;
            }
        };
        let diag_started = Instant::now();
        match session
            .produce(&format!("{TRIPLET_PROMPT}\n\n{TRIPLET_USER}"), Some(TRIPLET_TEMPERATURE))
            .await
        {
            Ok(turn) => {
                eprintln!(
                    "BENCH_DIAG produce ok: compl={} secs={:.1}",
                    turn.usage.completion_tokens,
                    diag_started.elapsed().as_secs_f64()
                );
                let parsed = super::triplet_extraction::parse_triplet_response_no_chunk(&turn.content);
                match parsed {
                    Ok(triplets) => {
                        let triplets_json = serde_json::json!({
                            "model": model,
                            "total_tokens": turn.usage.total_tokens,
                            "count": triplets.len(),
                            "triplets": triplets.iter().map(|t| serde_json::json!({
                                "subject": t.subject,
                                "predicate": t.predicate,
                                "object": t.object,
                                "confidence": t.confidence,
                                "source": t.source,
                            })).collect::<Vec<_>>(),
                        });
                        let _ = std::fs::write(
                            out_dir.join("triplets.json"),
                            serde_json::to_string_pretty(&triplets_json).expect("serialize"),
                        );
                        eprintln!("BENCH_DIAG triplets_saved={}", triplets.len());
                    }
                    Err(error) => eprintln!("BENCH_DIAG produce parse FAILED: {error}"),
                }
                let _ = diag_seed;
            }
            Err(error) => eprintln!("BENCH_DIAG produce FAILED: {error:#}"),
        }
    }
}
