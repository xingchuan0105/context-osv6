mod agent_memory_jobs;
mod analytics_jobs;

use anyhow::Result;
use app::{AppConfig, AppState};
use avrag_auth::{ActorId, AuthContext, OrgId, SubjectKind};
use avrag_cache_redis::DocumentLock;
use avrag_llm::{ChatMessage, MultiModalEmbeddingInput, SummaryGenerator};
use avrag_retrieval_data_plane::{
    DocumentIndexBatch, EntityIndexRecord, GraphPassageIndexRecord, MultimodalChunkIndexRecord,
    RelationIndexRecord, RetrievalDataPlane, TextChunkIndexRecord,
};
use avrag_search::{LexicalChunkDocument, TantivyLexicalIndex};
use avrag_storage_milvus::{MilvusConfig as StorageMilvusConfig, MilvusDataPlane};
use avrag_storage_pg::{
    NotificationCreateParams, ObjectStoreHandle, PgAppRepository, S3ObjectStore,
};
use avrag_storage_qdrant::{
    HttpQdrantBackend, QdrantCollectionConfig, QdrantDistance, QdrantPointUpsert,
    SecureQdrantFilterBuilder, VectorSearchBackend,
};
use ingestion::chunker::ChunkPolicy;
use ingestion::parser::{
    CodeParser, DocumentParser, ExternalParseKind, HtmlParser, LocalParseKind, MineruClient,
    MineruConfig, OfficeDocType, OfficeParserServiceClient, OfficeParserServiceConfig, ParsePlan,
    ParseRouter, PdfPageBackend, PdfParser, TextParser, normalize_parsed_document,
};
use ingestion::{
    AuditRecord, AuditSink, DocumentIr, DocumentIrValidationOptions, DocumentType, IngestionError,
    IngestionTask, NoopAuditSink, NoopStateSink, NoopTaskProcessor, NoopTaskSource, PageIr,
    ParseBackend, StateSink, TaskProcessor, TaskSource, Transition, WorkerRuntime, WorkerTick,
    sanitize_and_validate_document_ir,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::time::{Duration, interval};
use tracing::info;
use uuid::Uuid;

struct PgTaskSource {
    repo: PgAppRepository,
    worker_id: String,
}

#[async_trait::async_trait]
impl TaskSource for PgTaskSource {
    async fn fetch_next(&mut self) -> Result<Option<IngestionTask>, IngestionError> {
        self.repo
            .claim_next_ingestion_task(&self.worker_id)
            .await
            .map_err(|error| IngestionError::TaskSource(error.to_string()))
    }

    async fn complete(&mut self, task: &IngestionTask) -> Result<(), IngestionError> {
        self.repo
            .complete_ingestion_task(&task.task_id)
            .await
            .map_err(|error| IngestionError::TaskSource(error.to_string()))
    }

    async fn fail(&mut self, task: &IngestionTask, error: &str) -> Result<(), IngestionError> {
        self.repo
            .fail_ingestion_task(&task.task_id, error)
            .await
            .map_err(|err| IngestionError::TaskSource(err.to_string()))
    }
}

struct PgAuditSink {
    repo: PgAppRepository,
}

#[async_trait::async_trait]
impl AuditSink for PgAuditSink {
    async fn record(&mut self, record: AuditRecord) -> Result<(), IngestionError> {
        self.repo
            .append_audit_record(&record)
            .await
            .map_err(|error| IngestionError::AuditSink(error.to_string()))
    }
}

struct PgStateSink {
    repo: PgAppRepository,
}

#[async_trait::async_trait]
impl StateSink for PgStateSink {
    async fn transition(
        &mut self,
        task: &IngestionTask,
        transition: Transition,
    ) -> Result<(), IngestionError> {
        ingestion::DocumentStateMachine::validate(&transition)?;
        let context = task_context(task);
        let document_id = Uuid::parse_str(&task.document_id)
            .map_err(|error| IngestionError::StateSink(error.to_string()))?;

        self.repo
            .set_document_status(&context, document_id, transition.to.clone())
            .await
            .map_err(|error| IngestionError::StateSink(error.to_string()))?;

        self.repo
            .append_audit_record(&AuditRecord {
                audit_id: common::new_id(),
                org_id: task.org_id.clone(),
                actor_id: task.requested_by.clone(),
                action: ingestion::AuditAction::StateTransition,
                resource_type: "document".to_string(),
                resource_id: task.document_id.clone(),
                payload: serde_json::json!({
                    "from": transition.from,
                    "to": transition.to,
                    "task_id": task.task_id,
                }),
                created_at: chrono::Utc::now().to_rfc3339(),
            })
            .await
            .map_err(|error| IngestionError::StateSink(error.to_string()))?;

        if matches!(
            transition.to,
            common::DocumentStatus::Completed | common::DocumentStatus::Failed
        ) {
            if matches!(transition.to, common::DocumentStatus::Failed) {
                if let Some(user_id) = task
                    .requested_by
                    .as_deref()
                    .and_then(|value| Uuid::parse_str(value).ok())
                {
                    let analytics = analytics::AnalyticsService::new(self.repo.raw().clone());
                    let event = analytics::ProductEvent {
                        event_id: Uuid::new_v4(),
                        event_time: chrono::Utc::now(),
                        user_id,
                        session_id: None,
                        notebook_id: Uuid::parse_str(&task.notebook_id).ok(),
                        surface: analytics::Surface::Workspace,
                        event_name: analytics::ProductEventName::DocumentUploadFailed,
                        result: analytics::ResultTag::Failure,
                        request_id: None,
                        trace_id: None,
                        client_platform: "worker".to_string(),
                        metadata: serde_json::json!({
                            "document_id": task.document_id.clone(),
                            "task_id": task.task_id.clone(),
                        }),
                    };
                    if let Err(error) = analytics.record_product_event(&event).await {
                        info!(error = %error, document_id = task.document_id, "failed to record document upload failure event");
                    }
                }
            }
            if let Some(user_id) = task
                .requested_by
                .as_deref()
                .and_then(|value| Uuid::parse_str(value).ok())
            {
                let title = if matches!(transition.to, common::DocumentStatus::Completed) {
                    "Document ingestion completed"
                } else {
                    "Document ingestion failed"
                };
                let body = if matches!(transition.to, common::DocumentStatus::Completed) {
                    "A document finished ingestion and is ready for retrieval."
                } else {
                    "A document failed ingestion and needs attention."
                };
                let _ = self
                    .repo
                    .create_notification(
                        &context,
                        NotificationCreateParams {
                            user_id,
                            event_type: if matches!(
                                transition.to,
                                common::DocumentStatus::Completed
                            ) {
                                "ingestion.success".to_string()
                            } else {
                                "ingestion.failed".to_string()
                            },
                            title: title.to_string(),
                            body: body.to_string(),
                            data: serde_json::json!({
                                "document_id": task.document_id.clone(),
                                "task_id": task.task_id.clone(),
                                "status": format!("{:?}", transition.to),
                            }),
                            channels: vec!["in_app".to_string()],
                        },
                    )
                    .await;
            }
        }

        Ok(())
    }
}

struct PgTaskProcessor {
    repo: PgAppRepository,
    object_store: ObjectStoreHandle,
    qdrant: Option<HttpQdrantBackend>,
    retrieval_data_plane: Option<Arc<dyn RetrievalDataPlane>>,
    lexical_index: Option<Arc<TantivyLexicalIndex>>,
    qdrant_collection: String,
    qdrant_multimodal_collection: String,
    embedding_dim: usize,
    mm_embedding_dim: Option<usize>,
    embedding_client: Option<avrag_llm::EmbeddingClient>,
    mm_embedding_client: Option<avrag_llm::EmbeddingClient>,
    asset_url_ttl_secs: u64,
    redis_lock: Option<DocumentLock>,
    summary_generator: Option<avrag_llm::SummaryGenerator>,
    triplet_llm: Option<Arc<avrag_llm::LlmClient>>,
    analytics: Option<analytics::AnalyticsService>,
    usage_limit: Option<avrag_usage_limit::UsageLimitService>,
    mineru_client: Option<MineruClient>,
    office_parser_client: Option<OfficeParserServiceClient>,
}

#[async_trait::async_trait]
impl TaskProcessor for PgTaskProcessor {
    async fn process(&mut self, task: &IngestionTask) -> Result<(), IngestionError> {
        let task_kind = worker_task_kind(task);
        telemetry::prometheus::observe_worker_task_started(task_kind);
        let started_at = std::time::Instant::now();
        let result = async {
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

            let (object_path, claimed_mime_type) = match &task.payload {
                ingestion::IngestionTaskPayload::IngestDocument(payload) => {
                    (payload.object_path.clone(), payload.mime_type.clone())
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
                    (seed.object_path, seed.mime_type)
                }
            };

            let bytes = self
                .object_store
                .get(&object_path)
                .await
                .map_err(|error| IngestionError::StateSink(error.to_string()))?;
            let filename = Path::new(&object_path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| object_path.clone());
            let notebook_id = Uuid::parse_str(&task.notebook_id).unwrap_or_else(|_| Uuid::nil());

            let route_decision = ParseRouter::route(&bytes, &filename, &claimed_mime_type)
                .map_err(|error| IngestionError::StateSink(error.to_string()))?;

            info!(
                filename = %filename,
                route = ?route_decision.route,
                reason = %route_decision.reason,
                "Document routing decision"
            );
            if let ParsePlan::Pdf(pdf_plan) = &route_decision.plan {
                let edgeparse_pages = pdf_plan
                    .pages
                    .iter()
                    .filter(|page| page.backend == PdfPageBackend::EdgeParse)
                    .count();
                let mineru_ocr_pages = pdf_plan
                    .pages
                    .iter()
                    .filter(|page| page.backend == PdfPageBackend::MineruOcr)
                    .count();
                info!(
                    filename = %filename,
                    total_pages = pdf_plan.pages.len(),
                    edgeparse_pages,
                    mineru_ocr_pages,
                    "PDF page routing plan prepared"
                );
            }

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
                    parse_run_id,
                    notebook_id,
                    document_id,
                    &initial_backend_summary,
                    Some(&object_path),
                )
                .await
                .map_err(|error| IngestionError::StateSink(error.to_string()))?;

            let pipeline_metrics = match run_document_pipeline(
                self,
                task,
                &context,
                notebook_id,
                document_id,
                parse_run_id,
                &bytes,
                &filename,
                &object_path,
                &route_decision,
                &mut parse_run_state,
            )
            .await
            {
                Ok(metrics) => {
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
                            parse_run_id,
                            "completed",
                            &backend_summary,
                            i64::try_from(parse_run_started_at.elapsed().as_millis())
                                .unwrap_or(i64::MAX),
                            &warnings_json,
                            None,
                            Some(&object_path),
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
                            parse_run_id,
                            "failed",
                            &failure_summary,
                            i64::try_from(parse_run_started_at.elapsed().as_millis())
                                .unwrap_or(i64::MAX),
                            &failure_warnings,
                            Some(&failure_error),
                            Some(&object_path),
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
            if let Some(ref analytics) = self.analytics {
                if let Some(user_id) = task
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
            }
            Ok(())
        }
        .await;
        telemetry::prometheus::observe_worker_task_completed(
            task_kind,
            if result.is_ok() { "success" } else { "failure" },
            started_at.elapsed().as_secs_f64() * 1000.0,
        );
        result
    }
}

fn worker_task_kind(task: &IngestionTask) -> &'static str {
    match task.kind {
        ingestion::IngestionTaskKind::IngestDocument => "ingest_document",
        ingestion::IngestionTaskKind::ReindexDocument => "reindex_document",
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    telemetry::init("avrag-worker")?;
    let config = AppConfig::from_env();
    let database_url = config.database_url.clone();
    let state = AppState::bootstrap(config).await?;
    let embedding_dim = state.config().embedding.dimensions.unwrap_or(64);
    let heartbeat_secs = std::env::var("AVRAG_WORKER_HEARTBEAT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(30);
    let poll_secs = std::env::var("AVRAG_WORKER_POLL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(5);
    let worker_id =
        std::env::var("AVRAG_WORKER_ID").unwrap_or_else(|_| format!("worker-{}", common::new_id()));
    let mut poll_interval = interval(Duration::from_secs(poll_secs));
    let mut heartbeat_interval = interval(Duration::from_secs(heartbeat_secs));

    info!(
        runtime_mode = state.runtime_mode(),
        heartbeat_secs, poll_secs, "avrag worker skeleton started"
    );

    if let Some(database_url) = database_url {
        let repo = PgAppRepository::connect(&database_url).await?;
        if state.config().auto_migrate {
            repo.migrate().await?;
        }
        let lexical_index = if state.config().retrieval_backend == "legacy" {
            state
                .config()
                .tantivy_index_dir
                .as_deref()
                .map(TantivyLexicalIndex::open_writer)
                .transpose()?
                .map(Arc::new)
        } else {
            None
        };
        let analytics_pool = repo.raw().clone();
        let mut analytics_job_runner =
            analytics_jobs::AnalyticsJobRunner::from_env(analytics_pool.clone());
        let mut agent_memory_job_runner =
            agent_memory_jobs::AgentPreferenceConsolidationJobRunner::from_env(
                analytics_pool.clone(),
            );
        let usage_limit_pool = repo.raw().clone();
        let mut worker = WorkerRuntime::new(
            PgTaskSource {
                repo: repo.clone(),
                worker_id: worker_id.clone(),
            },
            PgAuditSink { repo: repo.clone() },
            PgStateSink { repo: repo.clone() },
            PgTaskProcessor {
                repo,
                object_store: build_worker_object_store(state.config()).await?,
                qdrant: build_worker_qdrant(state.config()),
                retrieval_data_plane: build_worker_retrieval_data_plane(state.config()).await?,
                lexical_index,
                qdrant_collection: state.config().qdrant.collection.clone(),
                qdrant_multimodal_collection: state.config().qdrant.multimodal_collection.clone(),
                embedding_dim,
                mm_embedding_dim: state.config().mm_embedding.dimensions,
                embedding_client: {
                    let ec = &state.config().embedding;
                    ec.to_llm_config().map(avrag_llm::EmbeddingClient::new)
                },
                asset_url_ttl_secs: state.config().object_storage.download_url_expire_sec,
                mm_embedding_client: {
                    let ec = &state.config().mm_embedding;
                    ec.to_llm_config().map(avrag_llm::EmbeddingClient::new)
                },
                redis_lock: {
                    let url = &state.config().redis.url;
                    if !url.trim().is_empty() {
                        DocumentLock::new(url).ok()
                    } else {
                        None
                    }
                },
                summary_generator: {
                    let sc = &state.config().summary_llm;
                    if let Some(config) = sc.to_llm_config() {
                        let mut generator = SummaryGenerator::new(config);
                        if let Some(template) =
                            load_prompt_template(state.config(), "summary_generation")
                        {
                            generator = generator.with_prompt_template(template);
                        }
                        if let Some(template) =
                            load_prompt_template(state.config(), "summary_generation_finalize")
                        {
                            generator = generator.with_finalize_prompt_template(template);
                        }
                        Some(generator)
                    } else {
                        None
                    }
                },
                triplet_llm: build_worker_triplet_llm(state.config()),
                analytics: Some(analytics::AnalyticsService::new(analytics_pool)),
                usage_limit: Some(avrag_usage_limit::UsageLimitService::new(usage_limit_pool)),
                mineru_client: MineruConfig::from_env().map(MineruClient::new),
                office_parser_client: OfficeParserServiceConfig::from_env()
                    .map(OfficeParserServiceClient::new),
            },
        );

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    info!("worker shutdown signal received");
                    break;
                }
                _ = poll_interval.tick() => {
                    match worker.run_once().await {
                        Ok(WorkerTick::Idle) => info!("worker poll completed with no tasks"),
                        Ok(WorkerTick::Processed(task)) => {
                            info!(task_id = task.task_id, kind = ?task.kind, "worker processed task");
                        }
                        Err(error) => {
                            info!(error = %error, "worker poll failed");
                        }
                    }
                }
                _ = heartbeat_interval.tick() => {
                    if let Some(runner) = analytics_job_runner.as_mut()
                        && let Err(error) = runner.maybe_run().await
                    {
                        telemetry::prometheus::record_dependency_failure("analytics_rollup");
                        info!(error = %error, worker_id, "analytics rollup job failed");
                    }
                    if let Some(runner) = agent_memory_job_runner.as_mut()
                        && let Err(error) = runner.maybe_run().await
                    {
                        telemetry::prometheus::record_dependency_failure("agent_memory");
                        info!(error = %error, worker_id, "agent preference consolidation job failed");
                    }
                    info!(runtime_mode = state.runtime_mode(), worker_id, "worker heartbeat");
                }
            }
        }
        return Ok(());
    }

    let mut worker = WorkerRuntime::new(
        NoopTaskSource,
        NoopAuditSink,
        NoopStateSink,
        NoopTaskProcessor,
    );
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("worker shutdown signal received");
                break;
            }
            _ = poll_interval.tick() => {
                match worker.run_once().await? {
                    WorkerTick::Idle => {
                        info!("worker poll completed with no tasks");
                    }
                    WorkerTick::Processed(task) => {
                        info!(task_id = task.task_id, kind = ?task.kind, "worker processed task");
                    }
                }
            }
            _ = heartbeat_interval.tick() => {
                info!(runtime_mode = state.runtime_mode(), "worker heartbeat");
            }
        }
    }

    Ok(())
}

fn task_context(task: &IngestionTask) -> AuthContext {
    let org_id = Uuid::parse_str(&task.org_id).unwrap_or_else(|_| Uuid::nil());
    let auth = AuthContext::new(OrgId::from(org_id), SubjectKind::System);
    if let Some(requested_by) = task
        .requested_by
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok())
    {
        return auth.with_actor_id(ActorId::new(requested_by));
    }
    auth
}

async fn build_worker_object_store(config: &AppConfig) -> Result<ObjectStoreHandle> {
    if !config.object_storage.endpoint.trim().is_empty()
        && !config.object_storage.bucket.trim().is_empty()
        && !config.object_storage.access_key.trim().is_empty()
        && !config.object_storage.secret_key.trim().is_empty()
    {
        let store = S3ObjectStore::new(
            config.object_storage.endpoint.clone(),
            config.object_storage.bucket.clone(),
            config.object_storage.region.clone(),
            config.object_storage.access_key.clone(),
            config.object_storage.secret_key.clone(),
            config.object_storage.use_path_style,
        )
        .await?;
        return Ok(ObjectStoreHandle::S3(store));
    }
    Ok(ObjectStoreHandle::local(PathBuf::from(
        config.object_root.clone(),
    )))
}

async fn build_worker_retrieval_data_plane(
    config: &AppConfig,
) -> Result<Option<Arc<dyn RetrievalDataPlane>>> {
    match config.retrieval_backend.as_str() {
        "legacy" => Ok(None),
        "milvus" => {
            let milvus_config = StorageMilvusConfig {
                url: config.milvus.url.clone(),
                token: Some(config.milvus.token.clone()).filter(|token| !token.trim().is_empty()),
                database: Some(config.milvus.database.clone())
                    .filter(|database| !database.trim().is_empty()),
                collection_prefix: config.milvus.collection_prefix.clone(),
                text_vector_dim: config.milvus.text_vector_dim,
                multimodal_vector_dim: config.milvus.multimodal_vector_dim,
                metric_type: config.milvus.metric_type.clone(),
            };
            let data_plane: Arc<dyn RetrievalDataPlane> =
                Arc::new(MilvusDataPlane::new(milvus_config));
            data_plane.ensure_schema().await?;
            Ok(Some(data_plane))
        }
        other => anyhow::bail!("unsupported RETRIEVAL_BACKEND: {other}"),
    }
}

fn build_worker_qdrant(config: &AppConfig) -> Option<HttpQdrantBackend> {
    if config.retrieval_backend == "legacy" && !config.qdrant.url.trim().is_empty() {
        return Some(HttpQdrantBackend::new(config.qdrant.url.clone()));
    }
    None
}

fn build_worker_triplet_llm(config: &AppConfig) -> Option<Arc<avrag_llm::LlmClient>> {
    if config.retrieval_backend != "milvus" {
        return None;
    }
    config
        .summary_llm
        .to_llm_config()
        .or_else(|| config.answer_llm.to_llm_config())
        .map(avrag_llm::LlmClient::new)
        .map(Arc::new)
}

fn estimate_token_count(text: &str) -> i64 {
    common::estimate_token_count(text)
}

fn is_remote_media_reference(path: &str) -> bool {
    common::is_remote_url(path)
}

async fn resolve_mineru_source_url(
    processor: &PgTaskProcessor,
    object_path: &str,
) -> Result<Option<String>, IngestionError> {
    if object_path.trim().is_empty() {
        return Ok(None);
    }
    if common::is_remote_url(object_path) {
        return Ok(Some(object_path.to_string()));
    }

    let presigned = processor
        .object_store
        .presigned_get_url(object_path, processor.asset_url_ttl_secs.max(300))
        .await
        .map_err(|error| IngestionError::StateSink(error.to_string()))?;
    Ok(Some(presigned))
}

#[derive(Clone)]
struct StoredMultimodalChunk {
    chunk_id: Uuid,
    asset_id: Uuid,
    image_path: String,
    caption: Option<String>,
    context_text: String,
    page: Option<i64>,
    chunk_type: String,
    parser_backend: String,
    source_locator: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
struct ExtractedTriplet {
    subject: String,
    predicate: String,
    object: String,
    supporting_chunk_ids: Vec<Uuid>,
}

#[derive(Debug, Clone)]
struct TripletExtractionBatch {
    chunk_ids: Vec<Uuid>,
    payload: serde_json::Value,
}

#[derive(Debug, Default)]
struct TripletExtractionOutput {
    triplets: Vec<ExtractedTriplet>,
    total_tokens: u32,
}

#[derive(Debug, Default)]
struct GraphIndexRecords {
    entities: Vec<EntityIndexRecord>,
    relations: Vec<RelationIndexRecord>,
    passages: Vec<GraphPassageIndexRecord>,
}

#[derive(Debug, Default, Clone)]
struct ParseRunOutputs {
    block_count: usize,
    asset_count: usize,
    text_chunk_count: usize,
    multimodal_chunk_count: usize,
    mirrored_asset_count: usize,
    text_vector_count: usize,
    multimodal_vector_count: usize,
    entity_count: usize,
    relation_count: usize,
    graph_passage_count: usize,
    graph_degrade_count: usize,
    graph_degrade_reasons: Vec<String>,
}

#[derive(Debug, Default, Clone)]
struct ParseRunState {
    document_ir: Option<DocumentIr>,
    validation_warnings: Vec<ingestion::DocumentIrValidationIssue>,
    outputs: ParseRunOutputs,
}

struct IngestionPipelineMetrics {
    content: String,
    processed_chunk_count: usize,
}

async fn execute_parse_plan(
    processor: &PgTaskProcessor,
    bytes: &[u8],
    filename: &str,
    object_path: &str,
    document_id: Uuid,
    route_decision: &ingestion::parser::ParseRouteDecision,
) -> Result<DocumentIr, IngestionError> {
    match &route_decision.plan {
        ParsePlan::Local(plan) => {
            execute_local_parse(bytes, filename, document_id, &plan.kind).await
        }
        ParsePlan::Office(plan) => {
            execute_office_parse(processor, bytes, filename, document_id, &plan.doc_type).await
        }
        ParsePlan::External(plan) => {
            execute_external_parse(
                processor,
                bytes,
                filename,
                object_path,
                document_id,
                &plan.kind,
            )
            .await
        }
        ParsePlan::Pdf(plan) => {
            execute_pdf_parse(processor, bytes, filename, object_path, document_id, plan).await
        }
    }
}

async fn run_document_pipeline(
    processor: &PgTaskProcessor,
    task: &IngestionTask,
    context: &AuthContext,
    notebook_id: Uuid,
    document_id: Uuid,
    parse_run_id: Uuid,
    bytes: &[u8],
    filename: &str,
    object_path: &str,
    route_decision: &ingestion::parser::ParseRouteDecision,
    parse_run_state: &mut ParseRunState,
) -> Result<IngestionPipelineMetrics, IngestionError> {
    let validation_report = sanitize_and_validate_document_ir(
        execute_parse_plan(
            processor,
            bytes,
            filename,
            object_path,
            document_id,
            route_decision,
        )
        .await?,
        &DocumentIrValidationOptions::default(),
    )
    .map_err(|error| IngestionError::StateSink(error.to_string()))?;

    let document_ir = validation_report.document;
    parse_run_state.validation_warnings = validation_report.warnings;
    parse_run_state.outputs.block_count = document_ir.blocks.len();
    parse_run_state.outputs.asset_count = document_ir.assets.len();
    parse_run_state.document_ir = Some(document_ir.clone());

    processor
        .repo
        .clear_document_ir_projection(context, document_id)
        .await
        .map_err(|error| IngestionError::StateSink(error.to_string()))?;
    processor
        .repo
        .replace_document_blocks(
            context,
            notebook_id,
            document_id,
            &build_document_block_rows(&document_ir, parse_run_id),
        )
        .await
        .map_err(|error| IngestionError::StateSink(error.to_string()))?;

    let chunk_plan =
        ingestion::chunker::build_ir_chunk_plan(&document_ir, filename, &ChunkPolicy::default());
    parse_run_state.outputs.text_chunk_count = chunk_plan.text_chunks.len();
    parse_run_state.outputs.multimodal_chunk_count = chunk_plan.multimodal_chunks.len();

    let content = collect_document_text(&chunk_plan);
    let body_chunks = build_document_chunk_rows(&chunk_plan, parse_run_id);

    info!(
        filename = %filename,
        text_chunks = chunk_plan.text_chunks.len(),
        multimodal_chunks = chunk_plan.multimodal_chunks.len(),
        "IR chunk plan generated"
    );

    let chunks = processor
        .repo
        .store_document_body_chunks(
            context,
            document_id,
            Some(parse_run_id),
            &content,
            &body_chunks,
        )
        .await
        .map_err(|error| IngestionError::StateSink(error.to_string()))?;
    let processed_chunk_count = chunks.len().max(1);

    if let Some(index) = &processor.lexical_index {
        let lexical_chunks = chunks
            .iter()
            .map(|chunk| {
                let chunk_id = Uuid::parse_str(&chunk.chunk_id).map_err(|error| {
                    IngestionError::StateSink(format!("invalid chunk id for Tantivy: {error}"))
                })?;
                let doc_id = Uuid::parse_str(&chunk.doc_id).map_err(|error| {
                    IngestionError::StateSink(format!("invalid doc id for Tantivy: {error}"))
                })?;
                Ok(LexicalChunkDocument {
                    org_id: context.org_id().into_uuid(),
                    doc_id,
                    chunk_id,
                    page: chunk.page,
                    content: chunk.content.clone(),
                })
            })
            .collect::<std::result::Result<Vec<_>, IngestionError>>()?;
        index
            .replace_document_chunks(context.org_id().into_uuid(), document_id, &lexical_chunks)
            .map_err(|error| {
                IngestionError::StateSink(format!("Tantivy indexing failed: {error}"))
            })?;
        info!(
            document_id = %document_id,
            chunks = lexical_chunks.len(),
            "Tantivy lexical index updated"
        );
    }

    let mut asset_uuid_by_ref = std::collections::HashMap::new();
    let mut stored_asset_path_by_ref = std::collections::HashMap::new();

    for asset in &document_ir.assets {
        let stored_asset_id = Uuid::new_v4();
        asset_uuid_by_ref.insert(asset.asset_id.clone(), stored_asset_id);

        let stored_image_path = mirror_document_asset(
            &processor.object_store,
            context,
            &task.notebook_id,
            &task.document_id,
            stored_asset_id,
            &asset.storage_path,
            processor.asset_url_ttl_secs,
        )
        .await
        .map_err(|error| IngestionError::StateSink(error.to_string()))?;
        if stored_image_path.is_some() {
            parse_run_state.outputs.mirrored_asset_count += 1;
        }
        stored_asset_path_by_ref.insert(asset.asset_id.clone(), stored_image_path.clone());

        processor
            .repo
            .store_document_asset(
                context,
                avrag_storage_pg::StoreDocumentAssetParams {
                    asset_id: stored_asset_id,
                    notebook_id,
                    document_id,
                    parse_run_id: Some(parse_run_id),
                    page: asset.page.map(|page| page as i32),
                    asset_kind: asset.asset_kind.as_str().to_string(),
                    storage_path: stored_image_path.clone(),
                    mime_type: asset.mime_type.clone(),
                    width: asset.width.map(|value| value as i32),
                    height: asset.height.map(|value| value as i32),
                    caption: None,
                    parser_backend: asset.parser_backend.as_str().to_string(),
                },
            )
            .await
            .map_err(|error| IngestionError::StateSink(error.to_string()))?;
    }

    let mut stored_multimodal_chunks = Vec::new();
    for multimodal_chunk in &chunk_plan.multimodal_chunks {
        let asset_id = asset_uuid_by_ref
            .get(&multimodal_chunk.asset_ref)
            .copied()
            .ok_or_else(|| {
                IngestionError::StateSink(format!(
                    "missing stored asset for multimodal block {}",
                    multimodal_chunk.block_id
                ))
            })?;
        let chunk_id =
            Uuid::parse_str(&multimodal_chunk.chunk_id).unwrap_or_else(|_| Uuid::new_v4());
        let stored_image_path = stored_asset_path_by_ref
            .get(&multimodal_chunk.asset_ref)
            .cloned()
            .flatten()
            .unwrap_or_else(|| multimodal_chunk.image_path.clone());

        processor
            .repo
            .store_multimodal_chunk(
                context,
                avrag_storage_pg::StoreMultimodalChunkParams {
                    chunk_id,
                    notebook_id,
                    document_id,
                    parse_run_id: Some(parse_run_id),
                    asset_id: Some(asset_id),
                    page: multimodal_chunk.page.map(|page| page as i32),
                    context_text: Some(multimodal_chunk.context_text.clone()),
                    caption: multimodal_chunk.caption.clone(),
                    normalized_text: multimodal_chunk.summary_text.clone(),
                    parser_backend: multimodal_chunk.parser_backend.as_str().to_string(),
                    metadata: serde_json::json!({
                        "block_id": multimodal_chunk.block_id,
                        "block_type": multimodal_chunk.block_type.as_str(),
                        "source_locator": multimodal_chunk.source_locator,
                        "source_image_path": multimodal_chunk.image_path,
                    }),
                },
            )
            .await
            .map_err(|error| IngestionError::StateSink(error.to_string()))?;

        stored_multimodal_chunks.push(StoredMultimodalChunk {
            chunk_id,
            asset_id,
            image_path: stored_image_path,
            caption: multimodal_chunk.caption.clone(),
            context_text: multimodal_chunk.summary_text.clone(),
            page: multimodal_chunk.page.map(i64::from),
            chunk_type: multimodal_chunk.block_type.as_str().to_string(),
            parser_backend: multimodal_chunk.parser_backend.as_str().to_string(),
            source_locator: Some(serde_json::json!(multimodal_chunk.source_locator)),
        });
    }

    if let Some(ref summary_gen) = processor.summary_generator {
        let user_uuid = task
            .requested_by
            .as_deref()
            .and_then(|value| Uuid::parse_str(value).ok());
        let mut skip_llm_summary = false;

        if let (Some(svc), Some(user_id)) = (&processor.usage_limit, user_uuid) {
            match svc.check_quota(context.org_id().into_uuid(), user_id).await {
                Ok(quota) => {
                    if quota.blocked_5h || quota.blocked_7d {
                        info!(
                            document_id = %document_id,
                            user_id = %user_id,
                            blocked_5h = quota.blocked_5h,
                            blocked_7d = quota.blocked_7d,
                            "skipping LLM summary generation because user quota is exhausted"
                        );
                        skip_llm_summary = true;
                    }
                }
                Err(error) => {
                    info!(
                        document_id = %document_id,
                        error = %error,
                        "quota check failed for summary generation; skipping LLM summary (fail-closed)"
                    );
                    skip_llm_summary = true;
                }
            }
        }

        if !skip_llm_summary {
            let title = document_ir.title.clone();
            let generated_summary = summary_gen
                .synthesize(&document_id.to_string(), &title, filename, &content)
                .await;
            match generated_summary {
                Ok((summary, llm_usage)) => {
                    if let Err(error) = processor
                        .repo
                        .update_document_summary(context, document_id, &summary)
                        .await
                    {
                        info!(document_id = %document_id, error = %error, "failed to update document summary with LLM result");
                    } else {
                        info!(document_id = %document_id, "successfully updated document summary with LLM result");
                    }
                    if let (Some(svc), Some(user_id)) = (&processor.usage_limit, user_uuid) {
                        let ctx = avrag_usage_limit::MeteringContext {
                            user_id,
                            org_id: context.org_id().into_uuid(),
                            feature: avrag_usage_limit::BillableFeature::Summary,
                            stage: "worker_summary".to_string(),
                            session_id: None,
                            document_id: Some(document_id),
                            request_id: None,
                            trace_id: None,
                        };
                        if let Err(error) = svc
                            .record_usage(
                                &ctx,
                                &llm_usage.provider,
                                &llm_usage.model,
                                llm_usage.prompt_tokens,
                                llm_usage.completion_tokens,
                                llm_usage.total_tokens,
                                avrag_usage_limit::UsageSource::Actual,
                            )
                            .await
                        {
                            info!(document_id = %document_id, error = %error, "failed to record summary usage");
                        }
                    }
                    if let (Some(analytics), Some(user_id)) = (&processor.analytics, user_uuid) {
                        let event = analytics::CostEvent {
                            event_id: Uuid::new_v4(),
                            event_time: chrono::Utc::now(),
                            user_id,
                            session_id: None,
                            notebook_id: None,
                            event_name: analytics::CostEventName::SummaryUsageMetered,
                            feature: "summary".to_string(),
                            provider: if llm_usage.provider.trim().is_empty() {
                                "unknown".to_string()
                            } else {
                                llm_usage.provider.clone()
                            },
                            model: if llm_usage.model.trim().is_empty() {
                                "unknown".to_string()
                            } else {
                                llm_usage.model.clone()
                            },
                            prompt_tokens: i64::from(llm_usage.prompt_tokens),
                            completion_tokens: i64::from(llm_usage.completion_tokens),
                            embedding_tokens: 0,
                            usage_units: avrag_usage_limit::compute_usage_units(
                                &llm_usage.provider,
                                &llm_usage.model,
                                llm_usage.prompt_tokens,
                                llm_usage.completion_tokens,
                            ),
                            storage_bytes_delta: 0,
                            external_call_count: 0,
                            source: "worker".to_string(),
                            metadata: serde_json::json!({
                                "task_id": task.task_id.clone(),
                                "document_id": document_id,
                                "filename": filename,
                            }),
                        };
                        if let Err(error) = analytics.record_cost_event(&event).await {
                            info!(document_id = %document_id, error = %error, "failed to record summary analytics event");
                        }
                    }
                }
                Err(error) => {
                    info!(document_id = %document_id, error = %error, "Summary generation failed, keeping naive fallback");
                }
            }
        }
    }

    let needs_text_vector_index =
        processor.qdrant.is_some() || processor.retrieval_data_plane.is_some();
    let text_index_records = if needs_text_vector_index {
        build_text_index_records(processor, &chunks).await?
    } else {
        Vec::new()
    };
    if !text_index_records.is_empty() {
        parse_run_state.outputs.text_vector_count = text_index_records.len();
    }

    let needs_multimodal_vector_index =
        processor.qdrant.is_some() || processor.retrieval_data_plane.is_some();
    let multimodal_index_records = if needs_multimodal_vector_index {
        build_multimodal_index_records(processor, &stored_multimodal_chunks).await?
    } else {
        Vec::new()
    };
    if !multimodal_index_records.is_empty() {
        parse_run_state.outputs.multimodal_vector_count = multimodal_index_records.len();
    }

    if let Some(qdrant) = &processor.qdrant {
        let collection = QdrantCollectionConfig {
            name: processor.qdrant_collection.clone(),
            vector_size: processor.embedding_dim as u64,
            distance: QdrantDistance::Cosine,
        };
        qdrant
            .ensure_collection(&collection)
            .await
            .map_err(|error| IngestionError::StateSink(error.to_string()))?;
        let filter = SecureQdrantFilterBuilder::with_doc_filter(context, document_id)
            .map_err(|error| IngestionError::StateSink(error.to_string()))?;
        qdrant
            .delete_points_by_filter(&processor.qdrant_collection, &filter)
            .await
            .map_err(|error| IngestionError::StateSink(error.to_string()))?;

        let points = text_index_records
            .iter()
            .map(|chunk| QdrantPointUpsert {
                chunk_id: chunk.chunk_id,
                doc_id: document_id,
                parse_run_id,
                org_id: context.org_id(),
                page: chunk.page,
                vector: chunk.vector.clone(),
                doc_version: 1,
            })
            .collect::<Vec<_>>();

        qdrant
            .upsert_points(&processor.qdrant_collection, &points)
            .await
            .map_err(|error| IngestionError::StateSink(error.to_string()))?;

        if !stored_multimodal_chunks.is_empty() {
            if !multimodal_index_records.is_empty() {
                let multimodal_vector_size = processor.mm_embedding_dim.ok_or_else(|| {
                    IngestionError::StateSink(
                        "MM_EMBEDDING_DIMENSIONS must be configured when multimodal indexing is enabled"
                            .to_string(),
                    )
                })?;
                let multimodal_collection = QdrantCollectionConfig {
                    name: processor.qdrant_multimodal_collection.clone(),
                    vector_size: multimodal_vector_size as u64,
                    distance: QdrantDistance::Cosine,
                };
                qdrant
                    .ensure_collection(&multimodal_collection)
                    .await
                    .map_err(|error| IngestionError::StateSink(error.to_string()))?;

                let multimodal_filter =
                    SecureQdrantFilterBuilder::with_doc_filter(context, document_id)
                        .map_err(|error| IngestionError::StateSink(error.to_string()))?;
                qdrant
                    .delete_points_by_filter(
                        &processor.qdrant_multimodal_collection,
                        &multimodal_filter,
                    )
                    .await
                    .map_err(|error| IngestionError::StateSink(error.to_string()))?;

                let multimodal_points = multimodal_index_records
                    .iter()
                    .map(|chunk| avrag_storage_qdrant::MultimodalQdrantPointUpsert {
                        chunk_id: chunk.chunk_id,
                        doc_id: document_id,
                        asset_id: chunk.asset_id,
                        parse_run_id,
                        org_id: context.org_id(),
                        page: chunk.page,
                        vector: chunk.vector.clone(),
                        caption: chunk.caption.clone(),
                        parser_backend: chunk.parser_backend.clone().unwrap_or_default(),
                        doc_version: 1,
                    })
                    .collect::<Vec<_>>();

                qdrant
                    .upsert_multimodal_points(
                        &processor.qdrant_multimodal_collection,
                        &multimodal_points,
                    )
                    .await
                    .map_err(|error| IngestionError::StateSink(error.to_string()))?;

                info!(
                    document_id = %document_id,
                    multimodal_chunks = multimodal_index_records.len(),
                    "Stored multimodal chunks"
                );
            }
        }
    }

    let graph_records = if processor.retrieval_data_plane.is_some() {
        let extraction = extract_triplets_for_index(
            processor,
            document_id,
            &text_index_records,
            parse_run_state,
        )
        .await;
        if extraction.total_tokens > 0 {
            let _ = processor
                .repo
                .record_usage_event(
                    context,
                    "triplet_extraction_tokens",
                    i64::from(extraction.total_tokens),
                    "worker_ingestion",
                )
                .await;
        }
        build_graph_index_records(processor, extraction.triplets, parse_run_state).await
    } else {
        GraphIndexRecords::default()
    };

    if let Some(data_plane) = &processor.retrieval_data_plane {
        let batch = build_document_index_batch(
            context,
            Some(notebook_id),
            document_id,
            parse_run_id,
            text_index_records,
            multimodal_index_records,
            graph_records,
        );
        let report = data_plane
            .replace_document_index(batch)
            .await
            .map_err(|error| {
                IngestionError::StateSink(format!("retrieval data plane indexing failed: {error}"))
            })?;
        parse_run_state.outputs.text_vector_count = report.text_chunk_count;
        parse_run_state.outputs.multimodal_vector_count = report.multimodal_chunk_count;
        parse_run_state.outputs.entity_count = report.entity_count;
        parse_run_state.outputs.relation_count = report.relation_count;
        parse_run_state.outputs.graph_passage_count = report.graph_passage_count;
    }

    Ok(IngestionPipelineMetrics {
        content,
        processed_chunk_count,
    })
}

async fn execute_local_parse(
    bytes: &[u8],
    filename: &str,
    document_id: Uuid,
    kind: &LocalParseKind,
) -> Result<DocumentIr, IngestionError> {
    let (doc_type, backend, parser): (DocumentType, ParseBackend, Box<dyn DocumentParser>) =
        match kind {
            LocalParseKind::Text => (
                DocumentType::Text,
                ParseBackend::TextLocal,
                Box::new(TextParser),
            ),
            LocalParseKind::Html => (
                DocumentType::Html,
                ParseBackend::HtmlLocal,
                Box::new(HtmlParser),
            ),
            LocalParseKind::Code => (
                DocumentType::Code,
                ParseBackend::CodeLocal,
                Box::new(CodeParser),
            ),
        };

    let parsed = parser.parse(bytes, filename).await.map_err(|error| {
        IngestionError::StateSink(format!("local parse failed for {filename}: {error}"))
    })?;
    Ok(document_ir_from_parsed_document(
        document_id,
        filename,
        doc_type,
        backend,
        parsed,
    ))
}

async fn execute_external_parse(
    processor: &PgTaskProcessor,
    bytes: &[u8],
    filename: &str,
    object_path: &str,
    document_id: Uuid,
    kind: &ExternalParseKind,
) -> Result<DocumentIr, IngestionError> {
    let mineru = processor.mineru_client.as_ref().ok_or_else(|| {
        IngestionError::StateSink(format!(
            "external parse selected for {filename}, but MINERU is not configured"
        ))
    })?;
    let source_url = resolve_mineru_source_url(processor, object_path).await?;

    match kind {
        ExternalParseKind::MineruImage => {
            let normalized = mineru
                .parse(bytes, filename, source_url.as_deref())
                .await
                .map_err(|error| {
                    IngestionError::StateSink(format!(
                        "MinerU precise parse failed for {filename}: {error}"
                    ))
                })?;
            let doc_type = DocumentType::from_filename(filename);
            Ok(DocumentIr::from_normalized_document(
                document_id.to_string(),
                doc_type,
                ParseBackend::MineruImage,
                &normalized,
            ))
        }
    }
}

async fn execute_office_parse(
    processor: &PgTaskProcessor,
    bytes: &[u8],
    filename: &str,
    document_id: Uuid,
    doc_type: &OfficeDocType,
) -> Result<DocumentIr, IngestionError> {
    let client = processor.office_parser_client.as_ref().ok_or_else(|| {
        IngestionError::StateSink(format!(
            "office parse selected for {filename}, but OFFICE_PARSER_BASE_URL is not configured"
        ))
    })?;

    let response = match doc_type {
        OfficeDocType::Docx => {
            client
                .parse_docx(bytes, filename, &document_id.to_string())
                .await
        }
        OfficeDocType::Xlsx => {
            client
                .parse_xlsx(bytes, filename, &document_id.to_string())
                .await
        }
        OfficeDocType::Ppt => {
            client
                .parse_ppt(bytes, filename, &document_id.to_string())
                .await
        }
        OfficeDocType::Pptx => {
            client
                .parse_pptx(bytes, filename, &document_id.to_string())
                .await
        }
        OfficeDocType::Doc => {
            return Err(IngestionError::StateSink(
                "legacy .doc is not supported by office-parser-jvm; convert to .docx".to_string(),
            ));
        }
        OfficeDocType::Xls => {
            return Err(IngestionError::StateSink(
                "legacy .xls is not supported by office-parser-jvm; convert to .xlsx".to_string(),
            ));
        }
    }
    .map_err(|error| {
        IngestionError::StateSink(format!("office parse failed for {filename}: {error}"))
    })?;

    let mut document_ir = response.document_ir;
    document_ir.document_id = document_id.to_string();
    if document_ir.title.trim().is_empty() {
        document_ir.title = filename.to_string();
    }
    document_ir.warnings.extend(response.warnings);
    Ok(document_ir)
}

async fn execute_pdf_parse(
    processor: &PgTaskProcessor,
    bytes: &[u8],
    filename: &str,
    object_path: &str,
    document_id: Uuid,
    plan: &ingestion::parser::PdfParsePlan,
) -> Result<DocumentIr, IngestionError> {
    let edge_pages = plan
        .pages
        .iter()
        .filter(|page| page.backend == PdfPageBackend::EdgeParse)
        .map(|page| page.page_number)
        .collect::<Vec<_>>();
    let ocr_pages = plan
        .pages
        .iter()
        .filter(|page| page.backend == PdfPageBackend::MineruOcr)
        .map(|page| page.page_number)
        .collect::<Vec<_>>();

    let digital_ir = if edge_pages.is_empty() {
        None
    } else {
        let parsed = PdfParser
            .parse_pages(bytes, filename, &edge_pages)
            .await
            .map_err(|error| {
                IngestionError::StateSink(format!(
                    "pdf digital parse failed for {filename}: {error}"
                ))
            })?;
        Some(
            document_ir_from_parsed_document(
                document_id,
                filename,
                DocumentType::Pdf,
                ParseBackend::EdgeParsePdf,
                parsed,
            )
            .with_pdf_defaults(ParseBackend::EdgeParsePdf),
        )
    };

    let mineru_ir = if ocr_pages.is_empty() {
        None
    } else {
        let mineru = processor.mineru_client.as_ref().ok_or_else(|| {
            IngestionError::StateSink(format!(
                "PDF OCR fallback selected for {filename}, but MINERU is not configured"
            ))
        })?;
        let source_url = resolve_mineru_source_url(processor, object_path).await?;
        let normalized = mineru
            .parse_pdf_pages(bytes, filename, &ocr_pages, source_url.as_deref())
            .await
            .map_err(|error| {
                IngestionError::StateSink(format!(
                    "MinerU OCR parse failed for {filename}: {error}"
                ))
            })?;
        Some(
            DocumentIr::from_normalized_document(
                document_id.to_string(),
                DocumentType::Pdf,
                ParseBackend::MineruPdfOcr,
                &normalized,
            )
            .with_pdf_defaults(ParseBackend::MineruPdfOcr),
        )
    };

    let title = digital_ir
        .as_ref()
        .map(|document| document.title.clone())
        .filter(|title| !title.trim().is_empty())
        .or_else(|| {
            mineru_ir
                .as_ref()
                .map(|document| document.title.clone())
                .filter(|title| !title.trim().is_empty())
        })
        .unwrap_or_else(|| filename.to_string());

    let mut merged = DocumentIr::new(
        document_id.to_string(),
        title,
        DocumentType::Pdf,
        if edge_pages.is_empty() {
            ParseBackend::MineruPdfOcr
        } else {
            ParseBackend::EdgeParsePdf
        },
    );
    if let Some(digital_ir) = &digital_ir {
        merged.metadata = digital_ir.metadata.clone();
        merged.warnings.extend(digital_ir.warnings.clone());
    }
    if let Some(mineru_ir) = &mineru_ir {
        if merged.metadata.is_empty() {
            merged.metadata = mineru_ir.metadata.clone();
        }
        merged.warnings.extend(mineru_ir.warnings.clone());
    }

    for page_plan in &plan.pages {
        let page_backend = match page_plan.backend {
            PdfPageBackend::EdgeParse => ParseBackend::EdgeParsePdf,
            PdfPageBackend::MineruOcr => ParseBackend::MineruPdfOcr,
        };
        let source_ir = if page_plan.backend == PdfPageBackend::EdgeParse {
            digital_ir.as_ref().ok_or_else(|| {
                IngestionError::StateSink(format!(
                    "PDF plan requested EdgeParse page {} but no EdgeParse result was produced",
                    page_plan.page_number
                ))
            })?
        } else {
            mineru_ir.as_ref().ok_or_else(|| {
                IngestionError::StateSink(format!(
                    "PDF plan requested MinerU page {} but OCR result is missing",
                    page_plan.page_number
                ))
            })?
        };
        if !document_ir_represents_page(source_ir, page_plan.page_number) {
            return Err(IngestionError::StateSink(format!(
                "{} did not produce requested page {} for {}",
                page_backend.as_str(),
                page_plan.page_number,
                filename
            )));
        }
        let page_slice = filter_document_ir_to_page(source_ir, page_plan.page_number);

        let mut page_row = page_slice.pages.into_iter().next().unwrap_or(PageIr {
            page_number: page_plan.page_number,
            width: None,
            height: None,
            backend: page_backend.clone(),
            text_char_count: 0,
            image_count: 0,
            metadata: Default::default(),
        });
        page_row.page_number = page_plan.page_number;
        page_row.backend = page_backend.clone();
        merged.pages.push(page_row);

        merged
            .blocks
            .extend(page_slice.blocks.into_iter().map(|mut block| {
                block.page = Some(page_plan.page_number);
                block.source_locator.page = Some(page_plan.page_number);
                block.parser_backend = page_backend.clone();
                block
            }));
        merged
            .assets
            .extend(page_slice.assets.into_iter().map(|mut asset| {
                asset.page = Some(page_plan.page_number);
                asset.parser_backend = page_backend.clone();
                asset
            }));
    }

    Ok(merged)
}

fn document_ir_from_parsed_document(
    document_id: Uuid,
    filename: &str,
    doc_type: DocumentType,
    backend: ParseBackend,
    parsed: ingestion::parser::ParsedDocument,
) -> DocumentIr {
    let normalized = normalize_parsed_document(&parsed, backend.as_str());
    let mut document_ir = DocumentIr::from_normalized_document(
        document_id.to_string(),
        doc_type,
        backend,
        &normalized,
    );
    if document_ir.title.trim().is_empty() {
        document_ir.title = filename.to_string();
    }
    document_ir
}

fn filter_document_ir_to_page(document_ir: &DocumentIr, page_number: u32) -> DocumentIr {
    let mut filtered = DocumentIr::new(
        document_ir.document_id.clone(),
        document_ir.title.clone(),
        document_ir.doc_type.clone(),
        document_ir.primary_backend.clone(),
    );
    filtered.backend_version = document_ir.backend_version.clone();
    filtered.language = document_ir.language.clone();
    filtered.metadata = document_ir.metadata.clone();
    filtered.pages = document_ir
        .pages
        .iter()
        .filter(|page| page.page_number == page_number)
        .cloned()
        .collect();
    filtered.blocks = document_ir
        .blocks
        .iter()
        .filter(|block| {
            block.page == Some(page_number) || block.source_locator.page == Some(page_number)
        })
        .cloned()
        .collect();
    filtered.assets = document_ir
        .assets
        .iter()
        .filter(|asset| asset.page == Some(page_number))
        .cloned()
        .collect();
    filtered.warnings = document_ir
        .warnings
        .iter()
        .filter(|warning| warning.page == Some(page_number))
        .cloned()
        .collect();
    filtered
}

fn document_ir_represents_page(document_ir: &DocumentIr, page_number: u32) -> bool {
    document_ir
        .pages
        .iter()
        .any(|page| page.page_number == page_number)
        || document_ir.blocks.iter().any(|block| {
            block.page == Some(page_number) || block.source_locator.page == Some(page_number)
        })
        || document_ir
            .assets
            .iter()
            .any(|asset| asset.page == Some(page_number))
}

fn build_parse_backend_summary(
    route_decision: &ingestion::parser::ParseRouteDecision,
    document_ir: Option<&DocumentIr>,
    outputs: &ParseRunOutputs,
) -> serde_json::Value {
    let page_backends = document_ir
        .map(|document| {
            document
                .pages
                .iter()
                .map(|page| {
                    serde_json::json!({
                        "page": page.page_number,
                        "backend": page.backend.as_str(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| match &route_decision.plan {
            ParsePlan::Pdf(plan) => plan
                .pages
                .iter()
                .map(|page| {
                    serde_json::json!({
                        "page": page.page_number,
                        "backend": match page.backend {
                            PdfPageBackend::EdgeParse => ParseBackend::EdgeParsePdf.as_str(),
                            PdfPageBackend::MineruOcr => ParseBackend::MineruPdfOcr.as_str(),
                        },
                    })
                })
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        });

    serde_json::json!({
        "route": &route_decision.route,
        "reason": &route_decision.reason,
        "plan": &route_decision.plan,
        "probe_result": &route_decision.probe_result,
        "page_backends": page_backends,
        "outputs": {
            "primary_backend": document_ir.map(|document| document.primary_backend.as_str()),
            "block_count": outputs.block_count,
            "asset_count": outputs.asset_count,
            "text_chunk_count": outputs.text_chunk_count,
            "multimodal_chunk_count": outputs.multimodal_chunk_count,
            "mirrored_asset_count": outputs.mirrored_asset_count,
            "text_vector_count": outputs.text_vector_count,
            "multimodal_vector_count": outputs.multimodal_vector_count,
            "entity_count": outputs.entity_count,
            "relation_count": outputs.relation_count,
            "graph_passage_count": outputs.graph_passage_count,
            "graph_degrade_count": outputs.graph_degrade_count,
            "graph_degrade_reasons": outputs.graph_degrade_reasons,
        },
    })
}

fn build_parse_warning_payload(
    document_ir: Option<&DocumentIr>,
    validation_warnings: &[ingestion::DocumentIrValidationIssue],
) -> serde_json::Value {
    let parse_warnings = document_ir
        .map(|document| {
            document
                .warnings
                .iter()
                .map(|warning| {
                    serde_json::json!({
                        "code": warning.code,
                        "message": warning.message,
                        "page": warning.page,
                        "backend": warning.backend.as_str(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let validation_warnings = validation_warnings
        .iter()
        .map(|warning| {
            serde_json::json!({
                "code": warning.code,
                "message": warning.message,
                "block_id": warning.block_id,
                "asset_id": warning.asset_id,
                "page": warning.page,
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "parse_warnings": parse_warnings,
        "validation_warnings": validation_warnings,
    })
}

fn build_document_block_rows(
    document_ir: &DocumentIr,
    parse_run_id: Uuid,
) -> Vec<avrag_storage_pg::StoredDocumentBlock> {
    document_ir
        .blocks
        .iter()
        .map(|block| avrag_storage_pg::StoredDocumentBlock {
            block_id: block.block_id.clone(),
            parse_run_id: Some(parse_run_id),
            page: block
                .page
                .or(block.source_locator.page)
                .map(|page| page as i32),
            block_type: block.block_type.as_str().to_string(),
            modality: block.modality.as_str().to_string(),
            text: block.text.clone(),
            summary_text: block.summary_text.clone(),
            caption: block.caption.clone(),
            asset_refs: serde_json::json!(block.asset_refs),
            section_path: serde_json::json!(block.section_path),
            source_locator_json: serde_json::json!(block.source_locator),
            parser_backend: block.parser_backend.as_str().to_string(),
            metadata_json: serde_json::json!(block.metadata),
        })
        .collect()
}

fn build_document_chunk_rows(
    chunk_plan: &ingestion::chunker::IrChunkPlan,
    parse_run_id: Uuid,
) -> Vec<avrag_storage_pg::StoreDocumentChunkParams> {
    chunk_plan
        .text_chunks
        .iter()
        .map(|chunk| avrag_storage_pg::StoreDocumentChunkParams {
            parse_run_id: Some(parse_run_id),
            page: chunk.page.map(|page| page as i32),
            content: chunk.text.clone(),
            metadata: serde_json::json!({
                "kind": chunk.block_type.as_str(),
                "cursor": chunk.cursor,
                "page": chunk.page,
                "block_id": chunk.block_id,
                "block_type": chunk.block_type.as_str(),
                "parser_backend": chunk.parser_backend.as_str(),
                "source_locator": chunk.source_locator,
                "section_path": chunk.section_path,
                "block_metadata": chunk.metadata,
            }),
        })
        .collect()
}

fn collect_document_text(chunk_plan: &ingestion::chunker::IrChunkPlan) -> String {
    chunk_plan
        .text_chunks
        .iter()
        .map(|chunk| chunk.text.as_str())
        .collect::<Vec<_>>()
        .join("\n\n")
}

async fn build_text_index_records(
    processor: &PgTaskProcessor,
    chunks: &[avrag_storage_pg::IndexedChunk],
) -> Result<Vec<TextChunkIndexRecord>, IngestionError> {
    let texts = chunks
        .iter()
        .map(|chunk| chunk.content.as_str())
        .collect::<Vec<_>>();
    let vectors = embed_text_vectors(processor, &texts).await?;

    chunks
        .iter()
        .zip(vectors)
        .map(|(chunk, vector)| {
            let chunk_id = Uuid::parse_str(&chunk.chunk_id)
                .map_err(|error| IngestionError::StateSink(format!("invalid chunk id: {error}")))?;
            Ok(TextChunkIndexRecord {
                chunk_id,
                content: chunk.content.clone(),
                vector,
                page: chunk.page,
                chunk_type: metadata_string(&chunk.metadata, "block_type")
                    .or_else(|| metadata_string(&chunk.metadata, "kind"))
                    .unwrap_or_else(|| "body".to_string()),
                parser_backend: metadata_string(&chunk.metadata, "parser_backend"),
                source_locator: metadata_value(&chunk.metadata, "source_locator"),
            })
        })
        .collect()
}

async fn build_multimodal_index_records(
    processor: &PgTaskProcessor,
    chunks: &[StoredMultimodalChunk],
) -> Result<Vec<MultimodalChunkIndexRecord>, IngestionError> {
    if chunks.is_empty() {
        return Ok(Vec::new());
    }

    let Some(client) = processor.mm_embedding_client.as_ref() else {
        info!(
            multimodal_chunks = chunks.len(),
            "MM embedding not configured; skipping multimodal vector indexing"
        );
        return Ok(Vec::new());
    };

    let semaphore = Arc::new(tokio::sync::Semaphore::new(4));
    let client = client.clone();
    let mut handles: Vec<tokio::task::JoinHandle<anyhow::Result<(usize, Vec<f32>)>>> =
        Vec::with_capacity(chunks.len());

    for (idx, chunk) in chunks.iter().enumerate() {
        let input = if is_remote_media_reference(&chunk.image_path) {
            MultiModalEmbeddingInput::text_image(
                chunk.context_text.clone(),
                chunk.image_path.clone(),
            )
        } else {
            MultiModalEmbeddingInput::text(chunk.context_text.clone())
        };
        let sem = semaphore.clone();
        let c = client.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem
                .acquire_owned()
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let vector = c.embed_multimodal_fused(&input, None).await?;
            Ok((idx, vector))
        }));
    }

    let mut indexed_embeddings = Vec::with_capacity(handles.len());
    for handle in handles {
        let (idx, vector) = handle
            .await
            .map_err(|error| IngestionError::StateSink(format!("task join: {error}")))?
            .map_err(|error| IngestionError::StateSink(format!("multimodal embedding: {error}")))?;
        indexed_embeddings.push((idx, vector));
    }
    indexed_embeddings.sort_by_key(|(idx, _)| *idx);

    Ok(chunks
        .iter()
        .zip(indexed_embeddings.into_iter().map(|(_, vector)| vector))
        .map(|(chunk, vector)| MultimodalChunkIndexRecord {
            chunk_id: chunk.chunk_id,
            asset_id: chunk.asset_id,
            context_text: chunk.context_text.clone(),
            caption: chunk.caption.clone(),
            image_path: Some(chunk.image_path.clone()),
            vector,
            page: chunk.page,
            chunk_type: chunk.chunk_type.clone(),
            parser_backend: Some(chunk.parser_backend.clone()),
            source_locator: chunk.source_locator.clone(),
        })
        .collect())
}

async fn embed_text_vectors(
    processor: &PgTaskProcessor,
    texts: &[&str],
) -> Result<Vec<Vec<f32>>, IngestionError> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    if let Some(ref client) = processor.embedding_client {
        client
            .embed(texts)
            .await
            .map_err(|error| IngestionError::StateSink(format!("embedding failed: {error}")))
    } else {
        Ok(vec![vec![0f32; processor.embedding_dim]; texts.len()])
    }
}

async fn extract_triplets_for_index(
    processor: &PgTaskProcessor,
    document_id: Uuid,
    text_chunks: &[TextChunkIndexRecord],
    parse_run_state: &mut ParseRunState,
) -> TripletExtractionOutput {
    let Some(llm) = processor.triplet_llm.clone() else {
        return TripletExtractionOutput::default();
    };

    let batches = build_triplet_extraction_batches(text_chunks);
    if batches.is_empty() {
        return TripletExtractionOutput::default();
    }

    let semaphore = Arc::new(tokio::sync::Semaphore::new(4));
    let mut handles = Vec::with_capacity(batches.len());
    for batch in batches {
        let llm = llm.clone();
        let sem = semaphore.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem
                .acquire_owned()
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let messages = build_triplet_extraction_messages(&batch);
            let response = llm.complete(&messages, Some(0.1)).await?;
            let raw_triplets = parse_triplet_response(&response.content)?;
            let triplets = raw_triplets
                .into_iter()
                .map(|(subject, predicate, object)| ExtractedTriplet {
                    subject,
                    predicate,
                    object,
                    supporting_chunk_ids: batch.chunk_ids.clone(),
                })
                .collect::<Vec<_>>();
            Ok::<_, anyhow::Error>((triplets, response.usage.total_tokens))
        }));
    }

    let mut output = TripletExtractionOutput::default();
    let mut seen = std::collections::HashSet::new();
    for handle in handles {
        match handle.await {
            Ok(Ok((triplets, total_tokens))) => {
                output.total_tokens = output.total_tokens.saturating_add(total_tokens);
                for triplet in triplets {
                    let key = (
                        triplet.subject.to_lowercase(),
                        triplet.predicate.to_lowercase(),
                        triplet.object.to_lowercase(),
                    );
                    if seen.insert(key) {
                        output.triplets.push(triplet);
                    }
                }
            }
            Ok(Err(error)) => {
                let reason = format!("triplet extraction failed: {error}");
                record_graph_degrade(&mut parse_run_state.outputs, reason.clone());
                info!(document_id = %document_id, error = %reason, "triplet extraction degraded");
            }
            Err(error) => {
                let reason = format!("triplet extraction task join failed: {error}");
                record_graph_degrade(&mut parse_run_state.outputs, reason.clone());
                info!(document_id = %document_id, error = %reason, "triplet extraction degraded");
            }
        }
    }

    output
}

fn build_triplet_extraction_batches(
    text_chunks: &[TextChunkIndexRecord],
) -> Vec<TripletExtractionBatch> {
    const TOKEN_BUDGET: i64 = 3_000;

    let mut batches = Vec::new();
    let mut current_ids = Vec::new();
    let mut current_chunks = Vec::new();
    let mut current_tokens = 0i64;

    for chunk in text_chunks {
        let chunk_tokens = estimate_token_count(&chunk.content).max(1);
        if !current_chunks.is_empty() && current_tokens + chunk_tokens > TOKEN_BUDGET {
            batches.push(TripletExtractionBatch {
                chunk_ids: std::mem::take(&mut current_ids),
                payload: serde_json::json!({ "chunks": std::mem::take(&mut current_chunks) }),
            });
            current_tokens = 0;
        }

        current_ids.push(chunk.chunk_id);
        current_chunks.push(serde_json::json!({
            "chunk_id": chunk.chunk_id.to_string(),
            "text": &chunk.content,
        }));
        current_tokens += chunk_tokens;
    }

    if !current_chunks.is_empty() {
        batches.push(TripletExtractionBatch {
            chunk_ids: current_ids,
            payload: serde_json::json!({ "chunks": current_chunks }),
        });
    }

    batches
}

fn build_triplet_extraction_messages(batch: &TripletExtractionBatch) -> Vec<ChatMessage> {
    vec![
        ChatMessage::system(
            "Extract factual subject-predicate-object triplets. Return only strict JSON.",
        ),
        ChatMessage::user(format!(
            "Return exactly this shape: {{\"triplets\":[[\"subject\",\"predicate\",\"object\"]]}}.\nChunks:\n{}",
            batch.payload
        )),
    ]
}

fn parse_triplet_response(content: &str) -> Result<Vec<(String, String, String)>> {
    let value: serde_json::Value = serde_json::from_str(content)?;
    let triplets = value
        .get("triplets")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("triplet response missing triplets array"))?;

    let mut parsed = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for item in triplets {
        let values = item
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("triplet item must be an array"))?;
        if values.len() != 3 {
            anyhow::bail!("triplet item must have exactly three strings");
        }
        let subject = normalize_triplet_component(&values[0])?;
        let predicate = normalize_triplet_component(&values[1])?;
        let object = normalize_triplet_component(&values[2])?;
        if subject.is_empty() || predicate.is_empty() || object.is_empty() {
            continue;
        }
        let key = (
            subject.to_lowercase(),
            predicate.to_lowercase(),
            object.to_lowercase(),
        );
        if seen.insert(key) {
            parsed.push((subject, predicate, object));
        }
    }
    Ok(parsed)
}

fn normalize_triplet_component(value: &serde_json::Value) -> Result<String> {
    let Some(text) = value.as_str() else {
        anyhow::bail!("triplet component must be a string");
    };
    Ok(text.trim().to_string())
}

async fn build_graph_index_records(
    processor: &PgTaskProcessor,
    triplets: Vec<ExtractedTriplet>,
    parse_run_state: &mut ParseRunState,
) -> GraphIndexRecords {
    if triplets.is_empty() {
        return GraphIndexRecords::default();
    }

    let mut entity_map: std::collections::BTreeMap<String, (String, Vec<Uuid>)> =
        std::collections::BTreeMap::new();
    for triplet in &triplets {
        for name in [&triplet.subject, &triplet.object] {
            let normalized = name.to_lowercase();
            let entry = entity_map
                .entry(normalized)
                .or_insert_with(|| (name.clone(), Vec::new()));
            for chunk_id in &triplet.supporting_chunk_ids {
                if !entry.1.contains(chunk_id) {
                    entry.1.push(*chunk_id);
                }
            }
        }
    }

    let entity_entries = entity_map.into_iter().collect::<Vec<_>>();
    let entity_texts = entity_entries
        .iter()
        .map(|(_, (name, _))| name.as_str())
        .collect::<Vec<_>>();
    let entity_vectors = match embed_text_vectors(processor, &entity_texts).await {
        Ok(vectors) => vectors,
        Err(error) => {
            record_graph_degrade(
                &mut parse_run_state.outputs,
                format!("graph entity embedding failed: {error}"),
            );
            return GraphIndexRecords::default();
        }
    };

    let relation_texts = triplets
        .iter()
        .map(|triplet| {
            format!(
                "{} {} {}",
                triplet.subject, triplet.predicate, triplet.object
            )
        })
        .collect::<Vec<_>>();
    let relation_text_refs = relation_texts
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let relation_vectors = match embed_text_vectors(processor, &relation_text_refs).await {
        Ok(vectors) => vectors,
        Err(error) => {
            record_graph_degrade(
                &mut parse_run_state.outputs,
                format!("graph relation embedding failed: {error}"),
            );
            return GraphIndexRecords::default();
        }
    };

    let entities = entity_entries
        .into_iter()
        .zip(entity_vectors)
        .map(
            |((normalized_name, (name, supporting_chunk_ids)), vector)| EntityIndexRecord {
                entity_id: Uuid::new_v4(),
                name,
                normalized_name,
                entity_type: None,
                vector,
                supporting_chunk_ids,
                metadata: Some(serde_json::json!({ "source": "worker_triplet_extraction" })),
            },
        )
        .collect::<Vec<_>>();

    let mut relations = Vec::with_capacity(triplets.len());
    let mut passages = Vec::with_capacity(triplets.len());
    for ((triplet, relation_text), vector) in triplets
        .into_iter()
        .zip(relation_texts)
        .zip(relation_vectors)
    {
        let relation_id = Uuid::new_v4();
        relations.push(RelationIndexRecord {
            relation_id,
            subject: triplet.subject.clone(),
            predicate: triplet.predicate.clone(),
            object: triplet.object.clone(),
            relation_text: relation_text.clone(),
            vector: vector.clone(),
            supporting_chunk_ids: triplet.supporting_chunk_ids.clone(),
            metadata: Some(serde_json::json!({ "source": "worker_triplet_extraction" })),
        });
        passages.push(GraphPassageIndexRecord {
            passage_id: Uuid::new_v4(),
            chunk_id: triplet.supporting_chunk_ids.first().copied(),
            text: relation_text,
            vector,
            relation_ids: vec![relation_id],
            metadata: Some(serde_json::json!({ "source": "worker_triplet_extraction" })),
        });
    }

    GraphIndexRecords {
        entities,
        relations,
        passages,
    }
}

fn build_document_index_batch(
    context: &AuthContext,
    workspace_id: Option<Uuid>,
    document_id: Uuid,
    parse_run_id: Uuid,
    text_chunks: Vec<TextChunkIndexRecord>,
    multimodal_chunks: Vec<MultimodalChunkIndexRecord>,
    graph_records: GraphIndexRecords,
) -> DocumentIndexBatch {
    DocumentIndexBatch {
        org_id: context.org_id(),
        workspace_id,
        document_id,
        parse_run_id,
        doc_version: 1,
        text_chunks,
        multimodal_chunks,
        entities: graph_records.entities,
        relations: graph_records.relations,
        graph_passages: graph_records.passages,
    }
}

fn metadata_string(metadata: &serde_json::Value, key: &str) -> Option<String> {
    metadata
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn metadata_value(metadata: &serde_json::Value, key: &str) -> Option<serde_json::Value> {
    metadata.get(key).cloned().filter(|value| !value.is_null())
}

fn record_graph_degrade(outputs: &mut ParseRunOutputs, reason: String) {
    outputs.graph_degrade_count += 1;
    outputs.graph_degrade_reasons.push(reason);
}

trait DocumentIrPdfExt {
    fn with_pdf_defaults(self, backend: ParseBackend) -> Self;
}

impl DocumentIrPdfExt for DocumentIr {
    fn with_pdf_defaults(mut self, backend: ParseBackend) -> Self {
        self.doc_type = DocumentType::Pdf;
        self.primary_backend = backend.clone();
        for page in &mut self.pages {
            page.backend = backend.clone();
        }
        for block in &mut self.blocks {
            if block.page.is_none() {
                block.page = block.source_locator.page;
            }
            block.parser_backend = backend.clone();
        }
        for asset in &mut self.assets {
            asset.parser_backend = backend.clone();
        }
        self
    }
}

fn build_asset_object_key(
    context: &AuthContext,
    notebook_id: &str,
    document_id: &str,
    asset_id: Uuid,
    source_path: &str,
) -> String {
    let extension = infer_asset_extension(source_path).unwrap_or("bin");
    format!(
        "{}/{}/{}/assets/{}.{}",
        context.org_id(),
        notebook_id,
        document_id,
        asset_id,
        extension
    )
}

fn infer_asset_extension(path: &str) -> Option<&'static str> {
    common::infer_image_extension(path)
}

async fn mirror_document_asset(
    object_store: &ObjectStoreHandle,
    context: &AuthContext,
    notebook_id: &str,
    document_id: &str,
    asset_id: Uuid,
    source_path: &str,
    ttl_secs: u64,
) -> Result<Option<String>> {
    if source_path.trim().is_empty() {
        return Ok(None);
    }

    let object_key =
        build_asset_object_key(context, notebook_id, document_id, asset_id, source_path);
    if is_remote_media_reference(source_path) {
        return mirror_remote_asset(object_store, source_path, &object_key, ttl_secs)
            .await
            .map(Some);
    }

    if let Some(local_path) = source_path.strip_prefix("temporary://") {
        let bytes = tokio::fs::read(local_path).await?;
        object_store.put(&object_key, &bytes).await?;
        return finalize_mirrored_asset_path(object_store, &object_key, ttl_secs)
            .await
            .map(Some);
    }

    let local_path = Path::new(source_path);
    if local_path.exists() {
        let bytes = tokio::fs::read(local_path).await?;
        object_store.put(&object_key, &bytes).await?;
        return finalize_mirrored_asset_path(object_store, &object_key, ttl_secs)
            .await
            .map(Some);
    }

    Ok(Some(source_path.to_string()))
}

async fn mirror_remote_asset(
    object_store: &ObjectStoreHandle,
    source_url: &str,
    object_key: &str,
    ttl_secs: u64,
) -> Result<String> {
    let response = reqwest::Client::new()
        .get(source_url)
        .send()
        .await?
        .error_for_status()?;
    let bytes = response.bytes().await?;
    object_store.put(object_key, &bytes).await?;
    finalize_mirrored_asset_path(object_store, object_key, ttl_secs).await
}

async fn finalize_mirrored_asset_path(
    object_store: &ObjectStoreHandle,
    object_key: &str,
    ttl_secs: u64,
) -> Result<String> {
    if object_store.is_remote() {
        object_store
            .presigned_get_url(object_key, ttl_secs.max(60))
            .await
    } else {
        Ok(object_key.to_string())
    }
}

fn load_prompt_template(config: &AppConfig, base_name: &str) -> Option<String> {
    let prompts_dir = PathBuf::from(config.prompts.dir.trim());
    let version = config.prompts.summary_version.trim();
    let mut candidates = Vec::new();
    if !version.is_empty() {
        candidates.push(prompts_dir.join(format!("{base_name}.{version}.tmpl")));
        candidates.push(prompts_dir.join(format!("{base_name}_{version}.tmpl")));
    }
    candidates.push(prompts_dir.join(format!("{base_name}.tmpl")));

    candidates.into_iter().find_map(|path| {
        fs::read_to_string(&path)
            .ok()
            .map(|template| template.trim().to_string())
            .filter(|template| !template.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingestion::parser::{
        ParsePlan, ParseRoute, ParseRouteDecision, PdfPagePlan, PdfParsePlan, RouteReason,
    };
    use std::{env, fs};
    use uuid::Uuid;

    #[test]
    fn load_prompt_template_prefers_versioned_file() {
        let temp_dir = env::temp_dir().join(format!("summary-template-{}", Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();
        fs::write(
            temp_dir.join("summary_generation.tmpl"),
            "default {{title}}",
        )
        .unwrap();
        fs::write(
            temp_dir.join("summary_generation.v2.tmpl"),
            "versioned {{title}}",
        )
        .unwrap();

        let mut config = AppConfig::default();
        config.prompts.dir = temp_dir.to_string_lossy().to_string();
        config.prompts.summary_version = "v2".to_string();

        let template = load_prompt_template(&config, "summary_generation").unwrap();
        assert_eq!(template, "versioned {{title}}");

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn build_parse_backend_summary_uses_fixed_contract_fields() {
        let route_decision = ParseRouteDecision {
            route: ParseRoute::Pdf,
            reason: RouteReason::ComplexPdf,
            probe_result: None,
            plan: ParsePlan::Pdf(PdfParsePlan {
                pages: vec![PdfPagePlan {
                    page_number: 2,
                    backend: PdfPageBackend::MineruOcr,
                    reason: RouteReason::ComplexPdf,
                }],
            }),
        };

        let summary = build_parse_backend_summary(
            &route_decision,
            None,
            &ParseRunOutputs {
                block_count: 3,
                asset_count: 1,
                text_chunk_count: 2,
                multimodal_chunk_count: 1,
                mirrored_asset_count: 1,
                text_vector_count: 2,
                multimodal_vector_count: 1,
                entity_count: 1,
                relation_count: 1,
                graph_passage_count: 1,
                graph_degrade_count: 1,
                graph_degrade_reasons: vec!["provider error".to_string()],
            },
        );

        assert!(summary.get("route").is_some());
        assert!(summary.get("reason").is_some());
        assert!(summary.get("plan").is_some());
        assert!(summary.get("probe_result").is_some());
        assert_eq!(summary["page_backends"][0]["page"], 2);
        assert_eq!(summary["outputs"]["text_vector_count"], 2);
        assert_eq!(summary["outputs"]["entity_count"], 1);
        assert_eq!(summary["outputs"]["graph_degrade_count"], 1);
    }

    #[test]
    fn parse_triplet_response_accepts_strict_json() {
        let triplets = parse_triplet_response(
            r#"{"triplets":[[" Alice ","founded","Acme"],["Alice","founded","Acme"]]}"#,
        )
        .unwrap();

        assert_eq!(
            triplets,
            vec![(
                "Alice".to_string(),
                "founded".to_string(),
                "Acme".to_string()
            )]
        );
    }

    #[test]
    fn parse_triplet_response_rejects_malformed_json() {
        let error = parse_triplet_response(r#"{"triplets":[{"subject":"Alice"}]}"#)
            .unwrap_err()
            .to_string();

        assert!(error.contains("triplet item must be an array"));
    }

    #[test]
    fn graph_degrade_reasons_are_counted() {
        let mut outputs = ParseRunOutputs::default();

        record_graph_degrade(&mut outputs, "malformed JSON".to_string());

        assert_eq!(outputs.graph_degrade_count, 1);
        assert_eq!(outputs.graph_degrade_reasons, vec!["malformed JSON"]);
    }

    #[test]
    fn build_document_index_batch_carries_parse_run_id() {
        let auth = AuthContext::new(OrgId::new(Uuid::from_u128(1)), SubjectKind::System);
        let document_id = Uuid::from_u128(2);
        let parse_run_id = Uuid::from_u128(3);
        let chunk_id = Uuid::from_u128(4);
        let relation_id = Uuid::from_u128(5);
        let batch = build_document_index_batch(
            &auth,
            Some(Uuid::from_u128(6)),
            document_id,
            parse_run_id,
            vec![TextChunkIndexRecord {
                chunk_id,
                content: "Alice founded Acme".to_string(),
                vector: vec![0.1, 0.2],
                page: Some(1),
                chunk_type: "paragraph".to_string(),
                parser_backend: Some("text_local".to_string()),
                source_locator: None,
            }],
            Vec::new(),
            GraphIndexRecords {
                entities: vec![EntityIndexRecord {
                    entity_id: Uuid::from_u128(7),
                    name: "Alice".to_string(),
                    normalized_name: "alice".to_string(),
                    entity_type: None,
                    vector: vec![0.1, 0.2],
                    supporting_chunk_ids: vec![chunk_id],
                    metadata: None,
                }],
                relations: vec![RelationIndexRecord {
                    relation_id,
                    subject: "Alice".to_string(),
                    predicate: "founded".to_string(),
                    object: "Acme".to_string(),
                    relation_text: "Alice founded Acme".to_string(),
                    vector: vec![0.1, 0.2],
                    supporting_chunk_ids: vec![chunk_id],
                    metadata: None,
                }],
                passages: vec![GraphPassageIndexRecord {
                    passage_id: Uuid::from_u128(8),
                    chunk_id: Some(chunk_id),
                    text: "Alice founded Acme".to_string(),
                    vector: vec![0.1, 0.2],
                    relation_ids: vec![relation_id],
                    metadata: None,
                }],
            },
        );

        assert_eq!(batch.document_id, document_id);
        assert_eq!(batch.parse_run_id, parse_run_id);
        assert_eq!(batch.text_chunks.len(), 1);
        assert_eq!(batch.entities.len(), 1);
        assert_eq!(batch.relations.len(), 1);
        assert_eq!(batch.graph_passages.len(), 1);
    }
}
