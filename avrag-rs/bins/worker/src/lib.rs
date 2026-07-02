mod agent_memory_jobs;
mod analytics_jobs;
mod audit_log_jobs;
mod indexing;
mod ingestion_guard;
mod orphan_object_jobs;
mod pdf;
mod pipeline;
mod runtime_support;
mod sources;

use anyhow::Result;
use app_core::{AppConfig, load_prompt_template};
use avrag_cache_redis::DocumentLock;
use avrag_llm::SummaryGenerator;
use avrag_storage_pg::PgAppRepository;
use ingestion::parser::{
    OfficeParserServiceClient, OfficeParserServiceConfig, PdfRendererServiceClient,
    PdfRendererServiceConfig,
};
use ingestion::{
    NoopAuditSink, NoopStateSink, NoopTaskProcessor, NoopTaskSource, WorkerRuntime, WorkerTick,
};
use std::sync::Arc;
use tokio::time::{Duration, interval};
use tracing::{error, info, warn};

use ingestion_guard::run_document_cleanup_once;
use pipeline::PgTaskProcessor;
use runtime_support::{
    apply_e2e_object_store_overrides, build_worker_object_store,
    build_worker_retrieval_data_plane, build_worker_ingestion_llm, build_worker_triplet_llm,
    describe_object_store_config, probe_object_store, spawn_health_listener, worker_health_port,
    worker_poll_interval, worker_runtime_mode,
};
use sources::{PgAuditSink, PgStateSink, PgTaskSource};

pub(crate) use pipeline::helpers::ParseRunOutputs;

pub async fn run() -> Result<()> {
    let _ = dotenvy::dotenv();
    telemetry::init("avrag-worker")?;
    spawn_health_listener(worker_health_port());
    let mut config = AppConfig::from_env();
    apply_e2e_object_store_overrides(&mut config);
    let database_url = config.database_url.clone();
    let embedding_dim = config.embedding.dimensions.unwrap_or(64);
    let heartbeat_secs = std::env::var("AVRAG_WORKER_HEARTBEAT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(30);
    let poll_interval_duration = worker_poll_interval();
    let poll_secs = poll_interval_duration.as_secs().max(1);
    let worker_id =
        std::env::var("AVRAG_WORKER_ID").unwrap_or_else(|_| format!("worker-{}", common::new_id()));
    let worker_queue_group =
        std::env::var("AVRAG_WORKER_QUEUE_GROUP").unwrap_or_else(|_| "default".to_string());
    let task_timeout_secs = std::env::var("AVRAG_INGESTION_TASK_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(300);
    let mut poll_interval = interval(poll_interval_duration);
    let mut heartbeat_interval = interval(Duration::from_secs(heartbeat_secs));

    info!(
        runtime_mode = worker_runtime_mode(&config.database_url),
        heartbeat_secs, poll_secs, "avrag worker skeleton started"
    );

    if let Some(database_url) = database_url {
        let repo = PgAppRepository::connect(&database_url).await?;
        if config.auto_migrate {
            repo.migrate().await?;
        }
        let analytics_pool = repo.raw().clone();
        let mut analytics_job_runner =
            analytics_jobs::AnalyticsJobRunner::from_env(analytics_pool.clone());
        let mut agent_memory_job_runner =
            agent_memory_jobs::AgentPreferenceConsolidationJobRunner::from_env(
                analytics_pool.clone(),
            );
        let mut audit_log_job_runner =
            audit_log_jobs::AuditLogJobRunner::from_env(repo.raw().clone());
        let usage_limit_store = std::sync::Arc::new(app_bootstrap::PgUsageLimitStoreAdapter::new(
            std::sync::Arc::new(repo.clone()),
        )) as std::sync::Arc<dyn app_core::UsageLimitStorePort>;
        let worker_object_store = build_worker_object_store(&config).await?;
        let object_store_config = describe_object_store_config(&config);
        let queue_group = std::env::var("AVRAG_WORKER_QUEUE_GROUP")
            .ok()
            .filter(|value| !value.trim().is_empty());
        if !config.object_root.trim().is_empty()
            && !config.object_storage.endpoint.trim().is_empty()
            && !config.object_storage.bucket.trim().is_empty()
            && !config.object_storage.access_key.trim().is_empty()
            && !config.object_storage.secret_key.trim().is_empty()
        {
            warn!(
                "object storage config ambiguous; S3 takes precedence per build_object_store rules"
            );
        }
        if let Some(queue_group) = queue_group.as_deref() {
            info!(
                worker_id,
                runtime_mode = worker_runtime_mode(&config.database_url),
                object_store = %object_store_config,
                queue_group,
                "worker storage startup config"
            );
        } else {
            info!(
                worker_id,
                runtime_mode = worker_runtime_mode(&config.database_url),
                object_store = %object_store_config,
                "worker storage startup config"
            );
        }
        let skip_storage_probe = std::env::var("AVRAG_WORKER_SKIP_STORAGE_PROBE")
            .ok()
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false);
        if skip_storage_probe {
            info!("worker storage probe skipped by AVRAG_WORKER_SKIP_STORAGE_PROBE");
        } else if let Err(probe_error) = probe_object_store(&config).await {
            error!(
                error = %probe_error,
                worker_id,
                "worker storage probe failed; exiting"
            );
            std::process::exit(1);
        }
        let cleanup_object_store = build_worker_object_store(&config).await?;
        let orphan_object_store = Arc::new(build_worker_object_store(&config).await?);
        let mut orphan_object_job_runner = orphan_object_jobs::OrphanObjectJobRunner::from_env(
            repo.raw().clone(),
            orphan_object_store,
        );
        let retrieval_data_plane = build_worker_retrieval_data_plane(&config).await?;
        let cleanup_retrieval_data_plane = retrieval_data_plane.clone();
        let cleanup_repo = repo.clone();
        let mut worker = WorkerRuntime::new(
            PgTaskSource {
                repo: repo.clone(),
                worker_id: worker_id.clone(),
                worker_queue_group: worker_queue_group.clone(),
            },
            PgAuditSink { repo: repo.clone() },
            PgStateSink { repo: repo.clone() },
            PgTaskProcessor {
                repo,
                object_store: worker_object_store,
                retrieval_data_plane,
                embedding_dim,
                embedding_client: {
                    let ec = &config.embedding;
                    ec.to_llm_config().map(avrag_llm::EmbeddingClient::new)
                },
                asset_url_ttl_secs: config.object_storage.download_url_expire_sec,
                mm_embedding_client: {
                    let ec = &config.mm_embedding;
                    ec.to_llm_config().map(avrag_llm::EmbeddingClient::new)
                },
                redis_lock: {
                    let url = &config.redis.url;
                    if !url.trim().is_empty() {
                        DocumentLock::new(url).ok()
                    } else {
                        None
                    }
                },
                summary_generator: {
                    let sc = &config.ingestion_llm;
                    if let Some(llm_config) = sc.to_llm_config() {
                        let mut generator = SummaryGenerator::new(llm_config);
                        if let Some(template) = load_prompt_template(
                            &config.prompts.dir,
                            &config.prompts.summary_version,
                            "summary-generation",
                        )
                        .await
                        {
                            generator = generator.with_prompt_template(template);
                        }
                        if let Some(template) = load_prompt_template(
                            &config.prompts.dir,
                            &config.prompts.summary_version,
                            "summary-generation-finalize",
                        )
                        .await
                        {
                            generator = generator.with_finalize_prompt_template(template);
                        }
                        Some(generator)
                    } else {
                        None
                    }
                },
                section_index_generator: {
                    let sc = &config.ingestion_llm;
                    sc.to_llm_config().map(|cfg| {
                        let mut section_gen = avrag_llm::SectionIndexGenerator::new(cfg);
                        if let Ok(system) = std::fs::read_to_string(format!(
                            "{}/pipeline/section-index.system.v1.md",
                            config.prompts.dir.trim()
                        )) {
                            if let Ok(user) = std::fs::read_to_string(format!(
                                "{}/templates/section-index-user.tmpl",
                                config.prompts.dir.trim()
                            )) {
                                section_gen = section_gen.with_prompts(system, user);
                            }
                        }
                        section_gen
                    })
                },
                triplet_llm: build_worker_triplet_llm(&config),
                analytics: Some(analytics::AnalyticsService::new(analytics_pool)),
                usage_limit: Some(avrag_billing::usage_limit::UsageLimitService::new(
                    usage_limit_store.clone(),
                )),
                office_parser_client: OfficeParserServiceConfig::from_env()
                    .map(OfficeParserServiceClient::new),
                pdf_renderer_client: PdfRendererServiceConfig::from_env()
                    .map(PdfRendererServiceClient::new),
                ingestion_llm: build_worker_ingestion_llm(&config),
                task_timeout_secs,
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
                        Ok(WorkerTick::Idle) => info!("worker ingestion poll completed with no tasks"),
                        Ok(WorkerTick::Processed(task)) => {
                            info!(task_id = task.task_id, kind = ?task.kind, "worker processed ingestion task");
                        }
                        Err(error) => {
                            info!(error = %error, "worker ingestion poll failed");
                        }
                    }
                    match run_document_cleanup_once(
                        &cleanup_repo,
                        &cleanup_object_store,
                        cleanup_retrieval_data_plane.as_ref(),
                        &worker_id,
                    ).await {
                        Ok(true) => info!("worker processed document cleanup task"),
                        Ok(false) => info!("worker document cleanup poll completed with no tasks"),
                        Err(error) => info!(error = %error, "worker document cleanup poll failed"),
                    }

                    let billing_store = std::sync::Arc::new(app_bootstrap::PgBillingStoreAdapter::new(
                        std::sync::Arc::new(cleanup_repo.clone()),
                    )) as std::sync::Arc<dyn app_core::BillingStorePort>;
                    if let Err(error) =
                        avrag_billing::expire_subscriptions(billing_store.clone()).await
                    {
                        warn!(error = %error, "billing expire subscriptions job failed");
                    }
                    if let Err(error) = avrag_billing::process_outbox(billing_store).await {
                        warn!(error = %error, "billing process outbox job failed");
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
                    if let Some(runner) = audit_log_job_runner.as_mut()
                        && let Err(error) = runner.maybe_run().await
                    {
                        telemetry::prometheus::record_dependency_failure("audit_log_prune");
                        info!(error = %error, worker_id, "audit log prune job failed");
                    }
                    if let Some(runner) = orphan_object_job_runner.as_mut()
                        && let Err(error) = runner.maybe_run().await
                    {
                        telemetry::prometheus::record_dependency_failure("orphan_object_cleanup");
                        info!(error = %error, worker_id, "orphan object cleanup job failed");
                    }
                    info!(runtime_mode = worker_runtime_mode(&config.database_url), worker_id, "worker heartbeat");
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
                info!(runtime_mode = worker_runtime_mode(&config.database_url), "worker heartbeat");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod main_tests;
