use anyhow::Result;
use avrag_cache_redis::DocumentLock;
use avrag_retrieval_data_plane::RetrievalDataPlane;
use avrag_storage_pg::{ObjectStoreHandle, PgAppRepository};
use ingestion::parser::{
    OfficeParserServiceClient, ParsePlan, ParseRouter, PdfPageBackend, PdfRendererServiceClient,
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
use crate::ingestion_guard::{
    ensure_ingestion_side_effects_allowed, spawn_ingestion_task_lock_heartbeat,
    stop_ingestion_task_lock_heartbeat, verify_uploaded_object_bytes, worker_task_kind,
};
use crate::pdf;
use crate::runtime_support::{fetch_url_content, task_context, url_to_filename};

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
                    .map_err(|error| IngestionError::StateSink(error.to_string()))?;

            let _lock_guard = if let Some(ref lock) = self.redis_lock {
                match lock.try_acquire(document_id).await {
                    Ok(Some(guard)) => Some(guard),
                    Ok(None) => {
                        info!(%document_id, "skipping document — lock held by another worker");
                        return Ok(());
                    }
                    Err(_) => None,
                }
            } else {
                None
            };

            let (object_path, claimed_mime_type, is_url_task) = match &task.payload {
                ingestion::IngestionTaskPayload::IngestDocument(payload) => {
                    (payload.object_path.clone(), payload.mime_type.clone(), false)
                }
                ingestion::IngestionTaskPayload::ReindexDocument(_) => {
                    let seed = self
                        .repo
                        .get_document_task_seed(&context, document_id)
                        .await
                        .map_err(|error| IngestionError::StateSink(error.to_string()))?
                        .ok_or_else(|| {
                            IngestionError::StateSink("document seed not found".to_string())
                        })?;
                    (seed.object_path, seed.mime_type, false)
                }
                ingestion::IngestionTaskPayload::IngestUrl(payload) => {
                    (payload.url.clone(), "text/html".to_string(), true)
                }
            };

            let bytes = if is_url_task {
                fetch_url_content(&object_path)
                    .await
                    .map_err(|error| IngestionError::StateSink(error.to_string()))?
            } else {
                self.object_store
                    .get(&object_path)
                    .await
                    .map_err(|error| IngestionError::StateSink(error.to_string()))?
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
                        return Err(IngestionError::StateSink(format!(
                            "security scan failed: malware detected ({threat_name})"
                        )));
                    }
                    Ok(ingestion::security_scanner::ScanResult::ZipBomb { ratio }) => {
                        return Err(IngestionError::StateSink(format!(
                            "security scan failed: ZIP bomb detected (compression ratio {ratio:.1})"
                        )));
                    }
                    Err(error) => {
                        warn!(error = %error, "security scan encountered an error, allowing processing to continue");
                    }
                }
            }

            let mut route_decision = ParseRouter::route(&bytes, &filename, &claimed_mime_type)
                .map_err(|error| IngestionError::StateSink(error.to_string()))?;
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
                .map_err(|error| IngestionError::StateSink(error.to_string()))?;

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
                    let backend_summary = build_parse_backend_summary(
                        &route_decision,
                        parse_run_state.document_ir.as_ref(),
                        &parse_run_state.outputs,
                    );
                    let warnings_json = build_parse_warning_payload(
                        parse_run_state.document_ir.as_ref(),
                        &parse_run_state.validation_warnings,
                    );
                    self.repo
                        .finish_document_parse_run(
                            &context,
                            avrag_storage_pg::FinishDocumentParseRunParams {
                                run_id: parse_run_id,
                                status: "completed",
                                backend_summary: &backend_summary,
                                duration_ms: i64::try_from(
                                    parse_run_started_at.elapsed().as_millis(),
                                )
                                .unwrap_or(i64::MAX),
                                warnings_json: &warnings_json,
                                error_json: None,
                                artifact_path: Some(&object_path),
                                task_id: &task.task_id,
                                lock_token: task.lock_token.as_deref(),
                            },
                        )
                        .await
                        .map_err(|error| IngestionError::StateSink(error.to_string()))?;

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
                    let failure_summary = build_parse_backend_summary(
                        &route_decision,
                        parse_run_state.document_ir.as_ref(),
                        &parse_run_state.outputs,
                    );
                    let failure_warnings = build_parse_warning_payload(
                        parse_run_state.document_ir.as_ref(),
                        &parse_run_state.validation_warnings,
                    );
                    let failure_error = serde_json::json!({ "message": error.to_string() });
                    let _ = self
                        .repo
                        .finish_document_parse_run(
                            &context,
                            avrag_storage_pg::FinishDocumentParseRunParams {
                                run_id: parse_run_id,
                                status: "failed",
                                backend_summary: &failure_summary,
                                duration_ms: i64::try_from(
                                    parse_run_started_at.elapsed().as_millis(),
                                )
                                .unwrap_or(i64::MAX),
                                warnings_json: &failure_warnings,
                                error_json: Some(&failure_error),
                                artifact_path: Some(&object_path),
                                task_id: &task.task_id,
                                lock_token: task.lock_token.as_deref(),
                            },
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
                .record_usage_event(
                    &context,
                    "pages_processed",
                    i64::try_from(processed_chunk_count).unwrap_or(1),
                    "worker_ingestion",
                )
                .await;
            let _ = self
                .repo
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
            Err(_) => Err(IngestionError::StateSink(format!(
                "document ingestion timed out after {} seconds",
                self.task_timeout_secs
            ))),
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
