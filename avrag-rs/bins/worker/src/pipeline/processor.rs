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
use tokio::sync::Mutex;
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

/// Text + multimodal embedding clients and vector dimension.
pub(crate) struct EmbeddingDeps {
    pub(crate) embedding_dim: usize,
    pub(crate) embedding_client: Option<avrag_llm::EmbeddingClient>,
    pub(crate) mm_embedding_client: Option<avrag_llm::EmbeddingClient>,
}

/// LLM clients used during ingestion (summary, section index, triplets, VLM).
pub(crate) struct LlmDeps {
    pub(crate) summary_generator: Option<avrag_llm::SummaryGenerator>,
    pub(crate) section_index_generator: Option<avrag_llm::SectionIndexGenerator>,
    pub(crate) triplet_llm: Option<Arc<avrag_llm::LlmClient>>,
    pub(crate) ingestion_llm: Option<Arc<avrag_llm::LlmClient>>,
}

/// External parse services (office / PDF renderer).
pub(crate) struct ParseServiceDeps {
    pub(crate) office_parser_client: Option<OfficeParserServiceClient>,
    pub(crate) pdf_renderer_client: Option<PdfRendererServiceClient>,
}

/// Usage metering + product analytics for ingestion tasks.
pub(crate) struct MeteringDeps {
    pub(crate) analytics: Option<analytics::AnalyticsService>,
    pub(crate) usage_limit: Option<avrag_billing::usage_limit::UsageLimitService>,
    /// Rebound to task org/user at the start of each `process`.
    pub(crate) task_usage_observer: Option<Arc<app_billing::TaskTenantUsageObserver>>,
}

/// Storage / lock / retrieval infrastructure for a worker task.
pub(crate) struct StorageDeps {
    pub(crate) repo: PgAppRepository,
    pub(crate) object_store: ObjectStoreHandle,
    pub(crate) retrieval_data_plane: Option<Arc<dyn RetrievalDataPlane>>,
    pub(crate) asset_url_ttl_secs: u64,
    pub(crate) redis_lock: Option<DocumentLock>,
}

pub(crate) struct PgTaskProcessor {
    pub(crate) storage: StorageDeps,
    pub(crate) embedding: EmbeddingDeps,
    pub(crate) llm: LlmDeps,
    pub(crate) parse: ParseServiceDeps,
    pub(crate) metering: MeteringDeps,
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
                let seed = self.storage.repo
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
        self.storage.repo
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

    /// Best-effort terminal write when the timed-out future left a `running` parse_run.
    async fn finish_parse_run_after_timeout(
        &self,
        task: &IngestionTask,
        tracked: TimeoutTrackedParseRun,
    ) {
        let context = task_context(task);
        let failure_error = serde_json::json!({
            "message": format!("task timeout after {}s", self.task_timeout_secs),
            "code": "ingestion_task_timeout",
        });
        let empty_state = ParseRunState::default();
        // Minimal route decision for summary assembly when the inner future was cancelled.
        let route_decision = tracked.route_decision;
        match self
            .finish_parse_run(
                &context,
                tracked.parse_run_id,
                tracked.parse_run_started_at,
                &tracked.object_path,
                task,
                &route_decision,
                &empty_state,
                "failed",
                Some(&failure_error),
            )
            .await
        {
            Ok(()) => info!(
                task_id = %task.task_id,
                parse_run_id = %tracked.parse_run_id,
                "parse run marked failed after task timeout"
            ),
            Err(error) => warn!(
                task_id = %task.task_id,
                parse_run_id = %tracked.parse_run_id,
                error = %error,
                "failed to mark parse run failed after task timeout"
            ),
        }
    }
}

/// Shared across the timed future so a cancel can still finish the parse_run row.
struct TimeoutTrackedParseRun {
    parse_run_id: Uuid,
    parse_run_started_at: std::time::Instant,
    object_path: String,
    route_decision: ParseRouteDecision,
}

#[async_trait::async_trait]
impl TaskProcessor for PgTaskProcessor {
    async fn process(&mut self, task: &IngestionTask) -> Result<(), IngestionError> {
        let task_kind = worker_task_kind(task);
        telemetry::prometheus::observe_worker_task_started(task_kind);
        let started_at = std::time::Instant::now();
        // Attribute exit-metered LLM/embedding spend to the task org/actor.
        if let Some(obs) = self.metering.task_usage_observer.as_ref() {
            let auth = task_context(task);
            let tenant = avrag_llm::TenantContext {
                org_id: auth.org_id().into_uuid(),
                user_id: auth
                    .actor_id()
                    .map(|a| a.into_uuid())
                    .unwrap_or_else(Uuid::nil),
            };
            obs.rebind(tenant).await;
        }
        let lock_heartbeat = task.lock_token.as_ref().map(|lock_token| {
            spawn_ingestion_task_lock_heartbeat(
                self.storage.repo.clone(),
                task.task_id.clone(),
                lock_token.clone(),
            )
        });
        // Survives future cancel so timeout can finish a `running` parse_run row.
        let timeout_parse_run: Arc<Mutex<Option<TimeoutTrackedParseRun>>> =
            Arc::new(Mutex::new(None));
        let timeout_parse_run_for_task = timeout_parse_run.clone();
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
            let _redis_lock_guard = if let Some(ref lock) = self.storage.redis_lock {
                match lock.try_acquire(document_id).await {
                    Ok(Some(guard)) => Some(guard),
                    Ok(None) => {
                        // Must NOT return Ok: WorkerRuntime treats Ok as Completed.
                        return Err(IngestionError::document_locked(format!(
                            "redis document lock held for {document_id}; requeue for retry"
                        )));
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
            // advisory lock is already held by another worker, fail (requeue) —
            // never Ok-skip, which would mark the document completed empty.
            let _pg_lock_guard = if _redis_lock_guard.is_none() {
                let key = document_advisory_key(document_id);
                match PgAdvisoryLockGuard::try_acquire(self.storage.repo.raw().clone(), key).await {
                    Some(guard) => Some(guard),
                    None => {
                        return Err(IngestionError::document_locked(format!(
                            "postgres advisory lock held for {document_id} (key={key}); requeue for retry"
                        )));
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
                    .map_err(|error| IngestionError::storage_object(error))?
            } else {
                self.storage.object_store
                    .get(&object_path)
                    .await
                    .map_err(|error| IngestionError::storage_object(error))?
            };
            if !is_url_task {
                verify_uploaded_object_bytes(&self.storage.repo, &context, document_id, &bytes).await?;
            }
            let filename = if is_url_task {
                url_to_filename(&object_path)
            } else {
                Path::new(&object_path)
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| object_path.clone())
            };
            let workspace_id = Uuid::parse_str(&task.workspace_id).unwrap_or_else(|_| Uuid::nil());

            // Security scan: malware (ClamAV) + ZIP-bomb detection.
            if !is_url_task {
                match ingestion::security_scanner::scan_upload(&bytes, &filename).await {
                    Ok(ingestion::security_scanner::ScanResult::Clean) => {}
                    Ok(ingestion::security_scanner::ScanResult::ThreatDetected { threat_name }) => {
                        return Err(IngestionError::malware(threat_name));
                    }
                    Ok(ingestion::security_scanner::ScanResult::ZipBomb { ratio }) => {
                        return Err(IngestionError::zip_bomb(ratio));
                    }
                    Err(error) => {
                        if env_flag_enabled("SECURITY_SCAN_FAIL_OPEN", false) {
                            warn!(error = %error, "security scan encountered an error; SECURITY_SCAN_FAIL_OPEN=true, allowing processing to continue");
                        } else {
                            return Err(IngestionError::scanner_unavailable(error));
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
                &self.storage.repo,
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
            self.storage.repo
                .documents()
                .create_document_parse_run(
                    &context,
                    avrag_storage_pg::CreateDocumentParseRunParams {
                        run_id: parse_run_id,
                        workspace_id,
                        document_id,
                        backend_summary: &initial_backend_summary,
                        artifact_path: Some(&object_path),
                        task_id: &task.task_id,
                        lock_token: task.lock_token.as_deref(),
                    },
                )
                .await
                .map_err(from_storage_error)?;
            info!(
                filename = %filename,
                document_id = %document_id,
                parse_run_id = %parse_run_id,
                "document parse run created"
            );
            {
                let mut track_route = route_decision.clone();
                // Drop heavy LiteParse snapshot from the timeout tracker.
                track_route.liteparse_snapshot = None;
                *timeout_parse_run_for_task.lock().await = Some(TimeoutTrackedParseRun {
                    parse_run_id,
                    parse_run_started_at,
                    object_path: object_path.clone(),
                    route_decision: track_route,
                });
            }

            let pipeline_metrics = match run_document_pipeline(
                self,
                RunDocumentPipelineParams {
                    task,
                    context: &context,
                    workspace_id,
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
                        &self.storage.repo,
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
                    // Completed successfully — clear tracker so timeout handler is a no-op.
                    *timeout_parse_run_for_task.lock().await = None;

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
                    let may_record_failure = self.storage.repo
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
                        *timeout_parse_run_for_task.lock().await = None;
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
                    *timeout_parse_run_for_task.lock().await = None;
                    return Err(error);
                }
            };

            let content = pipeline_metrics.content;
            let processed_chunk_count = pipeline_metrics.processed_chunk_count;

            let embedding_tokens = estimate_token_count(&content);
            let _ = self.storage.repo
                .sessions()
                .record_usage_event(
                    &context,
                    "pages_processed",
                    i64::try_from(processed_chunk_count).unwrap_or(1),
                    "worker_ingestion",
                )
                .await;
            let _ = self.storage.repo
                .sessions()
                .record_usage_event(
                    &context,
                    "embedding_tokens",
                    embedding_tokens,
                    "worker_ingestion",
                )
                .await;
            if let Some(ref analytics) = self.metering.analytics
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
                        workspace_id: Uuid::parse_str(&task.workspace_id).ok(),
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
            Err(_) => {
                // While the lease/heartbeat is still valid, mark any open parse_run failed.
                if let Some(tracked) = timeout_parse_run.lock().await.take() {
                    self.finish_parse_run_after_timeout(task, tracked).await;
                }
                Err(IngestionError::Timeout(self.task_timeout_secs))
            }
        };
        // Dropping the timed future releases advisory/redis guards via Drop;
        // stop heartbeat after timeout finish so finish_document_parse_run still sees processing lease.
        stop_ingestion_task_lock_heartbeat(lock_heartbeat).await;
        let outcome = match &result {
            Ok(()) => "success",
            Err(error) => {
                info!(
                    task_id = %task.task_id,
                    error_class = error.class(),
                    error = %error,
                    "worker task failed"
                );
                "failure"
            }
        };
        telemetry::prometheus::observe_worker_task_completed(
            task_kind,
            outcome,
            started_at.elapsed().as_secs_f64() * 1000.0,
        );
        result
    }
}
