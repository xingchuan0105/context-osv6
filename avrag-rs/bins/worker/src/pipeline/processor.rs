use anyhow::Result;
use avrag_cache_redis::DocumentLock;
use avrag_retrieval_data_plane::RetrievalDataPlane;
use avrag_storage_pg::{ObjectStoreHandle, PgAppRepository};
use ingestion::parser::{
    OfficeParserServiceClient, ParsePlan, ParseRouteDecision, ParseRouter, PdfPageBackend,
    PdfRendererServiceClient,
};
use ingestion::{IngestionError, IngestionTask, TaskProcessor};
use std::path::Path;
use std::sync::Arc;
use tokio::time::Duration;
use tracing::{info, warn};
use uuid::Uuid;

use super::document_pipeline::{ParseRunState, RunDocumentPipelineParams, run_document_pipeline};
use super::helpers::{
    build_parse_backend_summary, build_parse_warning_payload, estimate_token_count,
};
use crate::indexing::env_flag_enabled;
use crate::ingestion_guard::{
    ensure_ingestion_side_effects_allowed, from_storage_error,
    spawn_ingestion_task_lock_heartbeat, stop_ingestion_task_lock_heartbeat,
    verify_uploaded_object_bytes, worker_task_kind,
};
use crate::pdf;
use crate::runtime_support::{fetch_url_content, task_context, url_to_filename};

/// Derive a stable, document-scoped i64 advisory-lock key from a document id.
///
/// UUIDs are 128-bit; `pg_try_advisory_lock` takes a single i64. We mask to the
/// positive i64 range to avoid the Postgres negative-key edge cases and to keep
/// the key document-scoped with low collision probability.
fn document_advisory_key(document_id: Uuid) -> i64 {
    (document_id.as_u128() & 0x7fffffffffffffff) as i64
}

/// RAII guard that releases a Postgres advisory lock on drop.
///
/// Mirrors the semantics of [`avrag_cache_redis::DocumentLockGuard`]: releasing
/// is best-effort and logged on failure. The lock is acquired via the worker's
/// repository pool as a fallback when no Redis document lock is configured.
struct PgAdvisoryLockGuard {
    pool: sqlx::PgPool,
    key: i64,
}

impl PgAdvisoryLockGuard {
    async fn try_acquire(pool: sqlx::PgPool, key: i64) -> Option<Self> {
        let acquired = sqlx::query_scalar::<_, bool>("select pg_try_advisory_lock($1)")
            .bind(key)
            .fetch_one(&pool)
            .await
            .ok()?;
        if acquired {
            Some(Self { pool, key })
        } else {
            None
        }
    }
}

impl Drop for PgAdvisoryLockGuard {
    fn drop(&mut self) {
        let pool = self.pool.clone();
        let key = self.key;
        tokio::spawn(async move {
            if let Err(error) = sqlx::query_scalar::<_, bool>("select pg_advisory_unlock($1)")
                .bind(key)
                .fetch_one(&pool)
                .await
            {
                info!(key, error = %error, "failed to release document advisory lock");
            }
        });
    }
}

pub(crate) struct PgTaskProcessor {
    pub(crate) repo: PgAppRepository,
    pub(crate) object_store: ObjectStoreHandle,
    pub(crate) retrieval_data_plane: Option<Arc<dyn RetrievalDataPlane>>,
    pub(crate) embedding_dim: usize,
    pub(crate) embedding_client: Option<avrag_llm::EmbeddingClient>,
    pub(crate) mm_embedding_client: Option<avrag_llm::EmbeddingClient>,
    pub(crate) asset_url_ttl_secs: u64,
    pub(crate) redis_lock: Option<DocumentLock>,
    pub(crate) summary_generator: Option<avrag_llm::SummaryGenerator>,
    pub(crate) section_index_generator: Option<avrag_llm::SectionIndexGenerator>,
    pub(crate) triplet_llm: Option<Arc<avrag_llm::LlmClient>>,
    pub(crate) analytics: Option<analytics::AnalyticsService>,
    pub(crate) usage_limit: Option<avrag_billing::usage_limit::UsageLimitService>,
    pub(crate) office_parser_client: Option<OfficeParserServiceClient>,
    pub(crate) pdf_renderer_client: Option<PdfRendererServiceClient>,
    pub(crate) ingestion_llm: Option<Arc<avrag_llm::LlmClient>>,
    pub(crate) task_timeout_secs: u64,
}

struct PayloadSource {
    object_path: String,
    claimed_mime_type: String,
    is_url_task: bool,
}

impl PgTaskProcessor {
    /// Resolve the (object_path, mime, is_url) source for a task's payload.
    async fn resolve_payload_source(
        &self,
        task: &IngestionTask,
        context: &contracts::auth_runtime::AuthContext,
        document_id: Uuid,
    ) -> Result<PayloadSource, IngestionError> {
        match &task.payload {
            ingestion::IngestionTaskPayload::IngestDocument(payload) => Ok(PayloadSource {
                object_path: payload.object_path.clone(),
                claimed_mime_type: payload.mime_type.clone(),
                is_url_task: false,
            }),
            ingestion::IngestionTaskPayload::ReindexDocument(_) => {
                let seed = self
                    .repo
                    .bootstrap()
                    .get_document_task_seed(context, document_id)
                    .await
                    .map_err(from_storage_error)?
                    .ok_or_else(|| {
                        IngestionError::SeedNotFound
                    })?;
                Ok(PayloadSource {
                    object_path: seed.object_path,
                    claimed_mime_type: seed.mime_type,
                    is_url_task: false,
                })
            }
            ingestion::IngestionTaskPayload::IngestUrl(payload) => Ok(PayloadSource {
                object_path: payload.url.clone(),
                claimed_mime_type: "text/html".to_string(),
                is_url_task: true,
            }),
        }
    }

    /// Persist the terminal state of a parse run (completed or failed). Shared
    /// by the success and failure branches of `process` so the summary/warnings
    /// assembly and `finish_document_parse_run` call stays in one place.
    async fn finish_parse_run(
        &self,
        context: &contracts::auth_runtime::AuthContext,
        parse_run_id: Uuid,
        parse_run_started_at: std::time::Instant,
        object_path: &str,
        task: &IngestionTask,
        route_decision: &ParseRouteDecision,
        parse_run_state: &ParseRunState,
        status: &str,
        error_json: Option<&serde_json::Value>,
    ) -> Result<(), IngestionError> {
        let backend_summary = build_parse_backend_summary(
            route_decision,
            parse_run_state.document_ir.as_ref(),
            &parse_run_state.outputs,
        );
        let warnings_json = build_parse_warning_payload(
            parse_run_state.document_ir.as_ref(),
            &parse_run_state.validation_warnings,
            &parse_run_state.outputs,
        );
        self.repo
            .documents()
            .finish_document_parse_run(
                context,
                avrag_storage_pg::FinishDocumentParseRunParams {
                    run_id: parse_run_id,
                    status,
                    backend_summary: &backend_summary,
                    duration_ms: i64::try_from(parse_run_started_at.elapsed().as_millis())
                        .unwrap_or(i64::MAX),
                    warnings_json: &warnings_json,
                    error_json,
                    artifact_path: Some(object_path),
                    task_id: &task.task_id,
                    lock_token: task.lock_token.as_deref(),
                },
            )
            .await
            .map_err(from_storage_error)?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl TaskProcessor for PgTaskProcessor {
    async fn process(&mut self, task: &IngestionTask) -> Result<(), IngestionError> {
        let task_kind = worker_task_kind(task);
        telemetry::prometheus::observe_worker_task_started(task_kind);
        let started_at = std::time::Instant::now();
        let lock_heartbeat = task.lock_token.as_ref().map(|lock_token| {
            spawn_ingestion_task_lock_heartbeat(
                self.repo.clone(),
                task.task_id.clone(),
                lock_token.clone(),
            )
        });
        let timeout_result = tokio::time::timeout(
            Duration::from_secs(self.task_timeout_secs),
            async {
                let context = task_context(task);
                let document_id = Uuid::parse_str(&task.document_id)
                    .map_err(IngestionError::from)?;

            // Acquire a per-document lock so two workers cannot process the same
            // document concurrently. Prefer the Redis-backed distributed lock when
            // configured; otherwise fall back to a Postgres advisory lock derived
            // from the document id so the guarantee still holds without Redis.
            let _redis_lock_guard = if let Some(ref lock) = self.redis_lock {
                match lock.try_acquire(document_id).await {
                    Ok(Some(guard)) => Some(guard),
                    Ok(None) => {
                        info!(%document_id, "skipping document — lock held by another worker");
                        return Ok(());
                    }
                    Err(error) => {
                        warn!(%document_id, error = %error, "redis document lock acquire failed; falling back to advisory lock");
                        None
                    }
                }
            } else {
                None
            };
            // When no Redis lock is held, acquire the advisory fallback. If the
            // advisory lock is already held by another worker, skip this document.
            let _pg_lock_guard = if _redis_lock_guard.is_none() {
                let key = document_advisory_key(document_id);
                match PgAdvisoryLockGuard::try_acquire(self.repo.raw().clone(), key).await {
                    Some(guard) => Some(guard),
                    None => {
                        info!(%document_id, advisory_key = key, "skipping document — advisory lock held by another worker");
                        return Ok(());
                    }
                }
            } else {
                None
            };

            let source = self.resolve_payload_source(task, &context, document_id).await?;
            let object_path = source.object_path;
            let claimed_mime_type = source.claimed_mime_type;
            let is_url_task = source.is_url_task;

            let bytes = if is_url_task {
                fetch_url_content(&object_path)
                    .await
                    .map_err(|error| IngestionError::storage(error))?
            } else {
                self.object_store
                    .get(&object_path)
                    .await
                    .map_err(|error| IngestionError::storage(error))?
            };
            if !is_url_task {
                verify_uploaded_object_bytes(&self.repo, &context, document_id, &bytes).await?;
            }
            let filename = if is_url_task {
                url_to_filename(&object_path)
            } else {
                Path::new(&object_path)
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| object_path.clone())
            };
            let notebook_id = Uuid::parse_str(&task.notebook_id).unwrap_or_else(|_| Uuid::nil());

            // Security scan: malware (ClamAV) + ZIP-bomb detection.
            if !is_url_task {
                match ingestion::security_scanner::scan_upload(&bytes, &filename).await {
                    Ok(ingestion::security_scanner::ScanResult::Clean) => {}
                    Ok(ingestion::security_scanner::ScanResult::ThreatDetected { threat_name }) => {
                        return Err(IngestionError::security(format!("malware detected ({threat_name})")));
                    }
                    Ok(ingestion::security_scanner::ScanResult::ZipBomb { ratio }) => {
                        return Err(IngestionError::security(format!("ZIP bomb detected (compression ratio {ratio:.1})")));
                    }
                    Err(error) => {
                        if env_flag_enabled("SECURITY_SCAN_FAIL_OPEN", false) {
                            warn!(error = %error, "security scan encountered an error; SECURITY_SCAN_FAIL_OPEN=true, allowing processing to continue");
                        } else {
                            return Err(IngestionError::security(format!("scanner unavailable: {error}")));
                        }
                    }
                }
            }

            let mut route_decision = ParseRouter::route(&bytes, &filename, &claimed_mime_type)
                .map_err(|error| IngestionError::storage(error))?;
            if let ParsePlan::Pdf(ref mut pdf_plan) = route_decision.plan {
                pdf::maybe_truncate_pdf_plan(pdf_plan);
            }

            info!(
                filename = %filename,
                route = ?route_decision.route,
                reason = %route_decision.reason,
                "Document routing decision"
            );
            if let ParsePlan::Pdf(pdf_plan) = &route_decision.plan {
                let liteparse_text_pages = pdf_plan
                    .pages
                    .iter()
                    .filter(|page| page.backend == PdfPageBackend::LITEPARSE_TEXT)
                    .count();
                let visual_raster_pages = pdf_plan
                    .pages
                    .iter()
                    .filter(|page| page.backend == PdfPageBackend::VisualRaster)
                    .count();
                info!(
                    filename = %filename,
                    total_pages = pdf_plan.pages.len(),
                    liteparse_text_pages,
                    visual_raster_pages,
                    "PDF page routing plan prepared"
                );
            }

            ensure_ingestion_side_effects_allowed(
                &self.repo,
                &context,
                task,
                document_id,
                "parse run creation",
            )
            .await?;
            let parse_run_id = Uuid::new_v4();
            let parse_run_started_at = std::time::Instant::now();
            let mut parse_run_state = ParseRunState::default();
            let initial_backend_summary = build_parse_backend_summary(
                &route_decision,
                None,
                &parse_run_state.outputs,
            );
            self.repo
                .documents()
                .create_document_parse_run(
                    &context,
                    avrag_storage_pg::CreateDocumentParseRunParams {
                        run_id: parse_run_id,
                        notebook_id,
                        document_id,
                        backend_summary: &initial_backend_summary,
                        artifact_path: Some(&object_path),
                        task_id: &task.task_id,
                        lock_token: task.lock_token.as_deref(),
                    },
                )
                .await
                .map_err(from_storage_error)?;

            let pipeline_metrics = match run_document_pipeline(
                self,
                RunDocumentPipelineParams {
                    task,
                    context: &context,
                    notebook_id,
                    document_id,
                    parse_run_id,
                    bytes: &bytes,
                    filename: &filename,
                    object_path: &object_path,
                    route_decision: &route_decision,
                },
                &mut parse_run_state,
            )
            .await
            {
                Ok(metrics) => {
                    ensure_ingestion_side_effects_allowed(
                        &self.repo,
                        &context,
                        task,
                        document_id,
                        "parse run completion",
                    )
                    .await?;
                    self.finish_parse_run(
                        &context,
                        parse_run_id,
                        parse_run_started_at,
                        &object_path,
                        task,
                        &route_decision,
                        &parse_run_state,
                        "completed",
                        None,
                    )
                    .await?;

                    if let Some(document_ir) = parse_run_state.document_ir.as_ref() {
                        info!(
                            filename = %filename,
                            parse_run_duration_ms = parse_run_started_at.elapsed().as_millis(),
                            blocks = document_ir.blocks.len(),
                            assets = document_ir.assets.len(),
                            warnings = document_ir.warnings.len() + parse_run_state.validation_warnings.len(),
                            "Document parse run completed"
                        );
                    }

                    metrics
                }
                Err(error) => {
                    let may_record_failure = self
                        .repo
                        .documents()
                        .document_allows_ingestion_side_effects(
                            &context,
                            document_id,
                            &task.task_id,
                            task.lock_token.as_deref(),
                        )
                        .await
                        .unwrap_or(false);
                    if !may_record_failure {
                        return Err(error);
                    }
                    let failure_error = serde_json::json!({ "message": error.to_string() });
                    let _ = self
                        .finish_parse_run(
                            &context,
                            parse_run_id,
                            parse_run_started_at,
                            &object_path,
                            task,
                            &route_decision,
                            &parse_run_state,
                            "failed",
                            Some(&failure_error),
                        )
                        .await;
                    return Err(error);
                }
            };

            let content = pipeline_metrics.content;
            let processed_chunk_count = pipeline_metrics.processed_chunk_count;

            let embedding_tokens = estimate_token_count(&content);
            let _ = self
                .repo
                .sessions()
                .record_usage_event(
                    &context,
                    "pages_processed",
                    i64::try_from(processed_chunk_count).unwrap_or(1),
                    "worker_ingestion",
                )
                .await;
            let _ = self
                .repo
                .sessions()
                .record_usage_event(
                    &context,
                    "embedding_tokens",
                    embedding_tokens,
                    "worker_ingestion",
                )
                .await;
            if let Some(ref analytics) = self.analytics
                && let Some(user_id) = task
                    .requested_by
                    .as_deref()
                    .and_then(|value| Uuid::parse_str(value).ok())
                {
                    let event = analytics::CostEvent {
                        event_id: Uuid::new_v4(),
                        event_time: chrono::Utc::now(),
                        user_id,
                        session_id: None,
                        notebook_id: Uuid::parse_str(&task.notebook_id).ok(),
                        event_name: analytics::CostEventName::EmbeddingUsageMetered,
                        feature: "embedding".to_string(),
                        provider: "worker".to_string(),
                        model: "document_embedding".to_string(),
                        prompt_tokens: 0,
                        completion_tokens: 0,
                        embedding_tokens,
                        usage_units: embedding_tokens,
                        storage_bytes_delta: 0,
                        external_call_count: 0,
                        source: "worker_ingestion".to_string(),
                        metadata: serde_json::json!({
                            "task_id": task.task_id.clone(),
                            "document_id": task.document_id.clone(),
                            "task_kind": task_kind,
                        }),
                    };
                    if let Err(error) = analytics.record_cost_event(&event).await {
                        info!(document_id = %document_id, error = %error, "failed to record embedding analytics event");
                    }
                }
                Ok(())
            }
        )
        .await;
        let result = match timeout_result {
            Ok(inner) => inner,
            Err(_) => Err(IngestionError::Timeout(self.task_timeout_secs)),
        };
        stop_ingestion_task_lock_heartbeat(lock_heartbeat).await;
        telemetry::prometheus::observe_worker_task_completed(
            task_kind,
            if result.is_ok() { "success" } else { "failure" },
            started_at.elapsed().as_secs_f64() * 1000.0,
        );
        result
    }
}
