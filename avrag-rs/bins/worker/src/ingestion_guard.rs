use anyhow::Result;
use avrag_auth::AuthContext;
use avrag_retrieval_data_plane::RetrievalDataPlane;
use avrag_storage_pg::{
    DocumentCleanupTask, DocumentCleanupTaskCompletionOutcome, DocumentCleanupTaskFailureOutcome,
    ObjectStoreHandle, PgAppRepository,
};
use ingestion::{IngestionError, IngestionTask};
use sha2::{Digest, Sha256};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::time::{Duration, interval};
use tracing::{info, warn};
use uuid::Uuid;

use crate::runtime_support::{document_cleanup_task_context, safe_relative_object_key, task_context};

pub(crate) async fn ensure_ingestion_side_effects_allowed(
    repo: &PgAppRepository,
    context: &AuthContext,
    task: &IngestionTask,
    document_id: Uuid,
    phase: &str,
) -> Result<(), IngestionError> {
    let allowed = repo
        .document_allows_ingestion_side_effects(
            context,
            document_id,
            &task.task_id,
            task.lock_token.as_deref(),
        )
        .await
        .map_err(|error| IngestionError::StateSink(error.to_string()))?;
    if allowed {
        Ok(())
    } else {
        Err(IngestionError::StateSink(format!(
            "ingestion side effects aborted before {phase}: document is deleting/deleted or task lease was lost"
        )))
    }
}

pub(crate) async fn verify_uploaded_object_bytes(
    repo: &PgAppRepository,
    context: &AuthContext,
    document_id: Uuid,
    bytes: &[u8],
) -> Result<(), IngestionError> {
    let validation = repo
        .get_document_upload_validation(context, document_id)
        .await
        .map_err(|error| IngestionError::StateSink(error.to_string()))?;
    let Some(validation) = validation else {
        return Ok(());
    };

    if let Some(expected_size) = validation.upload_size_bytes {
        let actual_size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if expected_size != actual_size {
            return Err(IngestionError::StateSink(format!(
                "uploaded object size changed after validation: expected {expected_size} bytes, got {actual_size} bytes"
            )));
        }
    }

    if let Some(expected_sha256) = validation.upload_sha256.as_deref() {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let actual_sha256 = hex::encode(hasher.finalize());
        if expected_sha256 != actual_sha256 {
            return Err(IngestionError::StateSink(
                "uploaded object checksum changed after validation".to_string(),
            ));
        }
    }

    Ok(())
}

pub(crate) fn worker_task_kind(task: &IngestionTask) -> &'static str {
    match task.kind {
        ingestion::IngestionTaskKind::IngestDocument => "ingest_document",
        ingestion::IngestionTaskKind::ReindexDocument => "reindex_document",
        ingestion::IngestionTaskKind::IngestUrl => "ingest_url",
    }
}

const INGESTION_TASK_LOCK_HEARTBEAT_SECS: u64 = 60;

pub(crate) fn spawn_ingestion_task_lock_heartbeat(
    repo: PgAppRepository,
    task_id: String,
    lock_token: String,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut heartbeat = interval(Duration::from_secs(INGESTION_TASK_LOCK_HEARTBEAT_SECS));
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            heartbeat.tick().await;
            match repo.renew_ingestion_task_lock(&task_id, &lock_token).await {
                Ok(true) => {}
                Ok(false) => warn!(
                    task_id = %task_id,
                    "ingestion task lock heartbeat did not renew a processing row; lease may be lost"
                ),
                Err(error) => warn!(
                    task_id = %task_id,
                    error = %error,
                    "failed to renew ingestion task lock heartbeat"
                ),
            }
        }
    })
}

pub(crate) async fn stop_ingestion_task_lock_heartbeat(heartbeat: Option<tokio::task::JoinHandle<()>>) {
    let Some(heartbeat) = heartbeat else {
        return;
    };

    heartbeat.abort();
    if let Err(error) = heartbeat.await
        && !error.is_cancelled()
    {
        warn!(error = %error, "ingestion task lock heartbeat ended unexpectedly");
    }
}

pub(crate) fn spawn_document_cleanup_task_lock_heartbeat(
    repo: PgAppRepository,
    task_id: Uuid,
    lock_token: Uuid,
    lease_lost: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut heartbeat = interval(Duration::from_secs(INGESTION_TASK_LOCK_HEARTBEAT_SECS));
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            heartbeat.tick().await;
            match repo
                .renew_document_cleanup_task_lock(task_id, lock_token)
                .await
            {
                Ok(true) => {}
                Ok(false) => {
                    lease_lost.store(true, Ordering::SeqCst);
                    warn!(
                        task_id = %task_id,
                        "document cleanup task lock heartbeat did not renew a processing row; lease may be lost"
                    );
                    break;
                }
                Err(error) => warn!(
                    task_id = %task_id,
                    error = %error,
                    "failed to renew document cleanup task lock heartbeat"
                ),
            }
        }
    })
}

pub(crate) async fn stop_document_cleanup_task_lock_heartbeat(heartbeat: tokio::task::JoinHandle<()>) {
    heartbeat.abort();
    if let Err(error) = heartbeat.await
        && !error.is_cancelled()
    {
        warn!(error = %error, "document cleanup task lock heartbeat ended unexpectedly");
    }
}

pub(crate) async fn run_document_cleanup_once(
    repo: &PgAppRepository,
    object_store: &ObjectStoreHandle,
    retrieval_data_plane: Option<&Arc<dyn RetrievalDataPlane>>,
    worker_id: &str,
) -> Result<bool> {
    let Some(task) = repo
        .claim_next_document_cleanup_task(worker_id, None)
        .await?
    else {
        return Ok(false);
    };
    let lock_token = task
        .lock_token
        .ok_or_else(|| anyhow::anyhow!("claimed cleanup task missing lock token"))?;
    let lease_lost = Arc::new(AtomicBool::new(false));
    let heartbeat = spawn_document_cleanup_task_lock_heartbeat(
        repo.clone(),
        task.task_id,
        lock_token,
        lease_lost.clone(),
    );
    let result = process_document_cleanup_task(
        repo,
        object_store,
        retrieval_data_plane,
        &task,
        lease_lost.clone(),
    )
    .await;
    stop_document_cleanup_task_lock_heartbeat(heartbeat).await;

    if lease_lost.load(Ordering::SeqCst) {
        warn!(task_id = %task.task_id, document_id = %task.document_id, "document cleanup task lease lost; leaving task row unchanged");
        return Ok(true);
    }

    match result {
        Ok(()) => match repo
            .complete_document_cleanup_task(task.task_id, lock_token)
            .await?
        {
            DocumentCleanupTaskCompletionOutcome::Completed => {
                info!(task_id = %task.task_id, document_id = %task.document_id, "document cleanup task completed");
            }
            DocumentCleanupTaskCompletionOutcome::LeaseLost => {
                warn!(task_id = %task.task_id, document_id = %task.document_id, "document cleanup task lease lost before completion");
            }
        },
        Err(error) => match repo
            .fail_document_cleanup_task(task.task_id, lock_token, &error.to_string())
            .await?
        {
            DocumentCleanupTaskFailureOutcome::Requeued => {
                warn!(task_id = %task.task_id, document_id = %task.document_id, error = %error, "document cleanup task failed and was requeued");
            }
            DocumentCleanupTaskFailureOutcome::DeadLettered => {
                warn!(task_id = %task.task_id, document_id = %task.document_id, error = %error, "document cleanup task failed and was dead-lettered");
            }
            DocumentCleanupTaskFailureOutcome::LeaseLost => {
                warn!(task_id = %task.task_id, document_id = %task.document_id, error = %error, "document cleanup task failed but lease was lost");
            }
        },
    }
    Ok(true)
}

async fn ensure_document_cleanup_task_can_continue(
    repo: &PgAppRepository,
    task: &DocumentCleanupTask,
    lease_lost: &AtomicBool,
    phase: &str,
) -> Result<()> {
    if lease_lost.load(Ordering::SeqCst) {
        return Err(anyhow::anyhow!(
            "document cleanup task lease lost before {phase}"
        ));
    }
    let lock_token = task
        .lock_token
        .ok_or_else(|| anyhow::anyhow!("cleanup task missing lock token before {phase}"))?;
    if repo
        .document_cleanup_task_lease_is_current(task.task_id, lock_token)
        .await?
    {
        Ok(())
    } else {
        lease_lost.store(true, Ordering::SeqCst);
        Err(anyhow::anyhow!(
            "document cleanup task lease lost before {phase}"
        ))
    }
}

pub(crate) async fn process_document_cleanup_task(
    repo: &PgAppRepository,
    object_store: &ObjectStoreHandle,
    retrieval_data_plane: Option<&Arc<dyn RetrievalDataPlane>>,
    task: &DocumentCleanupTask,
    lease_lost: Arc<AtomicBool>,
) -> Result<()> {
    let context = document_cleanup_task_context(task);
    ensure_document_cleanup_task_can_continue(repo, task, &lease_lost, "target lookup").await?;
    let Some(targets) = repo
        .get_document_cleanup_targets(&context, task.document_id, &task.payload)
        .await?
    else {
        warn!(task_id = %task.task_id, document_id = %task.document_id, "document cleanup target document not found or is not deletable; completing task idempotently");
        return Ok(());
    };
    if !matches!(
        targets.status,
        contracts::documents::DocumentStatus::Deleting | contracts::documents::DocumentStatus::Deleted
    ) {
        return Err(anyhow::anyhow!(
            "document {} was not in deleting/deleted status during cleanup",
            task.document_id
        ));
    }

    ensure_document_cleanup_task_can_continue(repo, task, &lease_lost, "document object delete")
        .await?;
    if let Some(object_path) = targets
        .object_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if safe_relative_object_key(object_path) {
            object_store.delete(object_path).await.map_err(|error| {
                anyhow::anyhow!("delete document object {object_path}: {error}")
            })?;
        } else {
            warn!(
                task_id = %task.task_id,
                document_id = %task.document_id,
                object_path = %object_path,
                "skipping unsafe document object_path during cleanup"
            );
        }
    }

    ensure_document_cleanup_task_can_continue(repo, task, &lease_lost, "asset object deletes")
        .await?;
    for storage_path in &targets.asset_storage_paths {
        ensure_document_cleanup_task_can_continue(repo, task, &lease_lost, "asset object delete")
            .await?;
        let path = storage_path.trim();
        if safe_relative_object_key(path) {
            object_store
                .delete(path)
                .await
                .map_err(|error| anyhow::anyhow!("delete document asset object {path}: {error}"))?;
        } else {
            warn!(
                task_id = %task.task_id,
                document_id = %task.document_id,
                storage_path = %storage_path,
                "skipping unsafe document asset storage_path during cleanup"
            );
        }
    }

    ensure_document_cleanup_task_can_continue(repo, task, &lease_lost, "retrieval index delete")
        .await?;
    if let Some(data_plane) = retrieval_data_plane {
        data_plane
            .delete_document_index(&context, task.document_id)
            .await
            .map_err(|error| {
                anyhow::anyhow!(
                    "delete retrieval index for document {}: {error}",
                    task.document_id
                )
            })?;
    }

    ensure_document_cleanup_task_can_continue(repo, task, &lease_lost, "derived row cleanup")
        .await?;
    if !repo
        .cleanup_document_derived_rows(&context, task.document_id)
        .await?
    {
        return Err(anyhow::anyhow!(
            "document {} was not in deleting/deleted status during derived row cleanup",
            task.document_id
        ));
    }
    ensure_document_cleanup_task_can_continue(repo, task, &lease_lost, "mark document deleted")
        .await?;
    if !repo
        .mark_document_deleted(&context, task.document_id)
        .await?
    {
        return Err(anyhow::anyhow!(
            "document {} was not in deleting/deleted status during cleanup",
            task.document_id
        ));
    }
    info!(
        task_id = %task.task_id,
        org_id = %targets.org_id,
        notebook_id = %targets.notebook_id,
        document_id = %targets.document_id,
        status = ?targets.status,
        "document cleanup succeeded"
    );
    Ok(())
}
