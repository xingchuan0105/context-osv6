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
use app_core::AppConfig;
use avrag_cache_redis::DocumentLock;
use avrag_storage_pg::{BootstrapRepository, PgAppRepository, TenantPgPool};
use ingestion::{
    NoopAuditSink, NoopStateSink, NoopTaskProcessor, NoopTaskSource, WorkerRuntime, WorkerTick,
};
use std::sync::Arc;
use tokio::time::{Duration, interval};
use tracing::{error, info, warn};

use ingestion_guard::run_document_cleanup_once;
use pipeline::{EmbeddingDeps, LlmDeps, MeteringDeps, PgTaskProcessor, StorageDeps};
use runtime_support::{
    apply_e2e_object_store_overrides, build_worker_embedding_client,
    build_worker_embedding_client_from_secret, build_worker_ingestion_llm, build_worker_object_store,
    build_worker_retrieval_data_plane, describe_object_store_config, probe_object_store,
    spawn_health_listener, worker_health_port, worker_poll_interval, worker_runtime_mode,
    worker_system_tenant,
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
    let embedding_dim = config.milvus.text_vector_dim;
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
        let bootstrap = BootstrapRepository::connect(&database_url).await?;
        if config.auto_migrate {
            bootstrap.migrate().await?;
        }
        let repo = PgAppRepository {
            pool: TenantPgPool::new(bootstrap.raw().clone()),
        };
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
        // Exit metering: long-lived clients share TaskTenantUsageObserver; each task
        // rebinds org/user before LLM/embedding work (see PgTaskProcessor::process).
        // ADR-0010: billable + wallet so platform index spend debits the owner.
        let wallet_for_meter = std::sync::Arc::new(app_bootstrap::PgWalletStoreAdapter::new(
            std::sync::Arc::new(repo.clone()),
        )) as std::sync::Arc<dyn app_core::WalletStorePort>;
        let task_usage_observer = std::sync::Arc::new(
            app_billing::TaskTenantUsageObserver::new(
                usage_limit_store.clone(),
                worker_system_tenant(&config),
            )
            .with_wallet(wallet_for_meter.clone()),
        );
        let usage_observer: runtime_support::WorkerUsageObserver = {
            let obs: std::sync::Arc<dyn avrag_llm::UsageObserver> = task_usage_observer.clone();
            Some((obs, worker_system_tenant(&config)))
        };
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
        // ADR-0010 G4: resolve the local user's embedding secret once at startup.
        let provider_secrets: Option<Arc<dyn app_core::ProviderSecretStorePort>> =
            app_bootstrap::PgProviderSecretStoreAdapter::from_env(std::sync::Arc::new(
                repo.clone(),
            ))
            .ok()
            .map(|a| Arc::new(a) as Arc<dyn app_core::ProviderSecretStorePort>);
        let bootstrap_owner = worker_system_tenant(&config).owner_user_id;
        let embedding_secret = app_bootstrap::resolve_bootstrap_secret(
            &provider_secrets,
            bootstrap_owner,
            app_core::ProviderSecretPurpose::Embedding,
        )
        .await;
        let retrieval_data_plane =
            build_worker_retrieval_data_plane(&config, Some(repo.raw())).await?;
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
            {
                // Result-level completion cache for deterministic ingestion
                // calls (summary / section index / triplets). Shares the Redis
                // connection used for document locks.
                let completion_cache = {
                    let url = &config.redis.url;
                    if !url.trim().is_empty() {
                        avrag_cache_redis::CacheStore::new(url).ok().map(|store| {
                            avrag_llm::CompletionCache::new(std::sync::Arc::new(store))
                        })
                    } else {
                        None
                    }
                };
                PgTaskProcessor {
                    storage: StorageDeps {
                        repo: repo.clone(),
                        object_store: worker_object_store,
                        retrieval_data_plane,
                        asset_url_ttl_secs: config.object_storage.download_url_expire_sec,
                        redis_lock: {
                            let url = &config.redis.url;
                            if !url.trim().is_empty() {
                                DocumentLock::new(url).ok()
                            } else {
                                None
                            }
                        },
                    },
                    embedding: EmbeddingDeps {
                        embedding_dim,
                        embedding_client: build_worker_embedding_client(
                            &config.embedding,
                            "document_embedding",
                            &usage_observer,
                        )
                        .or_else(|| {
                            build_worker_embedding_client_from_secret(
                                embedding_secret.as_ref(),
                                "document_embedding",
                                &usage_observer,
                            )
                        }),
                        mm_embedding_client: build_worker_embedding_client(
                            &config.mm_embedding,
                            "document_embedding_mm",
                            &usage_observer,
                        ),
                    },
                    llm: LlmDeps {
                        ingestion_llm: build_worker_ingestion_llm(&config, &usage_observer),
                        completion_cache,
                    },
                    metering: MeteringDeps {
                        analytics: Some(analytics::AnalyticsService::new(analytics_pool)),
                        usage_limit: Some(avrag_billing::usage_limit::UsageLimitService::new(
                            usage_limit_store.clone(),
                        )),
                        task_usage_observer: Some(task_usage_observer),
                        wallet: Some(wallet_for_meter),
                        // Secrets kept for ops introspection only — index gate is balance-only.
                        provider_secrets: provider_secrets.clone(),
                    },
                    task_timeout_secs,
                }
            },
        );

        // SIGTERM (docker stop) joins ctrl_c as a shutdown signal; an in-flight
        // tick always runs to completion before the loop breaks.
        #[cfg(unix)]
        let shutdown = async {
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("install SIGTERM handler");
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = sigterm.recv() => {},
            }
        };
        #[cfg(not(unix))]
        let shutdown = async {
            let _ = tokio::signal::ctrl_c().await;
        };
        tokio::pin!(shutdown);

        // Side jobs (billing/outbox/usage-export/retention/analytics/memory) are
        // NOT claim-based — they must run on exactly one replica. Ingestion and
        // document-cleanup queues are claim-based (SKIP LOCKED) and replica-safe.
        let side_jobs = !matches!(
            std::env::var("AVRAG_WORKER_SIDE_JOBS").as_deref(),
            Ok("0") | Ok("false") | Ok("no")
        );

        loop {
            tokio::select! {
                _ = &mut shutdown => {
                    info!("worker shutdown signal received, finishing current tick then stopping");
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

                    if side_jobs {
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

                        // ADR 0006: usage export jobs + 365-day llm_usage_events retention.
                        let usage_store: std::sync::Arc<dyn app_core::UsageLimitStorePort> =
                            std::sync::Arc::new(app_bootstrap::PgUsageLimitStoreAdapter::new(
                                std::sync::Arc::new(cleanup_repo.clone()),
                            ));
                        match usage_store.process_next_usage_export_job().await {
                            Ok(true) => info!("worker processed usage export job"),
                            Ok(false) => {}
                            Err(error) => {
                                warn!(error = %error, "usage export job failed");
                            }
                        }
                        let cutoff = chrono::Utc::now() - chrono::Duration::days(365);
                        match usage_store.purge_llm_usage_older_than(cutoff, 5_000).await {
                            Ok(0) => {}
                            Ok(n) => info!(deleted = n, "purged llm_usage_events past 365d retention"),
                            Err(error) => {
                                warn!(error = %error, "llm_usage retention purge failed");
                            }
                        }
                    }
                }
                _ = heartbeat_interval.tick() => {
                    if side_jobs {
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
