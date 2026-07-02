use anyhow::{Context, Result};
use app_core::AppConfig;
use avrag_auth::{ActorId, AuthContext, OrgId, SubjectKind};
use avrag_retrieval_data_plane::RetrievalDataPlane;
use avrag_storage_milvus::{MilvusConfig as StorageMilvusConfig, MilvusDataPlane};
use avrag_storage_pg::{DocumentCleanupTask, ObjectStoreHandle, ObjectStoreHeadError, S3ObjectStore};
use ingestion::IngestionTask;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};
use uuid::Uuid;

pub(crate) fn task_context(task: &IngestionTask) -> AuthContext {
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

pub(crate) fn document_cleanup_task_context(task: &DocumentCleanupTask) -> AuthContext {
    let auth = AuthContext::new(OrgId::from(task.org_id), SubjectKind::System);
    if let Some(requested_by) = task.requested_by {
        return auth.with_actor_id(ActorId::new(requested_by));
    }
    auth
}

/// E2E subprocesses inherit repo `.env` via `dotenv()`. Mirror API bootstrap by forcing
/// local object storage and honoring `AVRAG_OBJECT_ROOT` from the test harness.
pub(crate) fn apply_e2e_object_store_overrides(config: &mut AppConfig) {
    if !std::env::var("E2E_ENABLED")
        .ok()
        .is_some_and(|value| value == "true" || value.eq_ignore_ascii_case("true"))
    {
        return;
    }
    config.object_storage.endpoint.clear();
    config.object_storage.bucket.clear();
    config.object_storage.access_key.clear();
    config.object_storage.secret_key.clear();
    if let Ok(root) = std::env::var("AVRAG_OBJECT_ROOT") {
        let root = root.trim();
        if !root.is_empty() {
            config.object_root = root.to_string();
        }
    }
}

pub(crate) async fn build_worker_object_store(config: &AppConfig) -> Result<ObjectStoreHandle> {
    if has_complete_s3_config(config) {
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

pub(crate) fn describe_object_store_config(config: &AppConfig) -> String {
    if has_complete_s3_config(config) {
        return format!(
            "backend=s3 endpoint={} bucket={}",
            mask_endpoint(&config.object_storage.endpoint),
            config.object_storage.bucket.trim()
        );
    }
    format!("backend=local root={}", config.object_root.trim())
}

pub(crate) async fn probe_object_store(config: &AppConfig) -> Result<()> {
    if worker_skip_storage_probe() {
        return Ok(());
    }

    if has_complete_s3_config(config) {
        let store = build_worker_object_store(config).await?;
        return match store.head(".worker-probe").await {
            Ok(_) | Err(ObjectStoreHeadError::NotFound { .. }) => Ok(()),
            Err(error) => Err(anyhow::anyhow!(error)).context("s3 object store probe failed"),
        };
    }

    let root = PathBuf::from(config.object_root.trim());
    tokio::fs::create_dir_all(&root)
        .await
        .with_context(|| format!("failed to create local object root {}", root.display()))?;

    let probe_path = root.join(".worker-probe");
    let payload = b"ok";
    tokio::fs::write(&probe_path, payload)
        .await
        .with_context(|| format!("failed to write probe file {}", probe_path.display()))?;
    let readback = tokio::fs::read(&probe_path)
        .await
        .with_context(|| format!("failed to read probe file {}", probe_path.display()))?;
    if readback != payload {
        return Err(anyhow::anyhow!(
            "local object store probe readback mismatch for {}",
            probe_path.display()
        ));
    }
    tokio::fs::remove_file(&probe_path)
        .await
        .with_context(|| format!("failed to delete probe file {}", probe_path.display()))?;
    Ok(())
}

pub(crate) async fn fetch_url_content(url: &str) -> Result<Vec<u8>> {
    common::validate_http_url_with_dns(url, true)
        .map_err(|error| anyhow::anyhow!("url fetch blocked: {error}"))?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let response = client.get(url).send().await?.error_for_status()?;
    let bytes = response.bytes().await?;
    Ok(bytes.to_vec())
}

pub(crate) fn url_to_filename(url: &str) -> String {
    url.rsplit('/')
        .next()
        .filter(|s| !s.is_empty() && s.contains('.'))
        .map(|s| s.to_string())
        .unwrap_or_else(|| "page.html".to_string())
}

pub(crate) async fn build_worker_retrieval_data_plane(
    config: &AppConfig,
) -> Result<Option<Arc<dyn RetrievalDataPlane>>> {
    if !config.enable_rag {
        return Ok(None);
    }
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
    let data_plane: Arc<dyn RetrievalDataPlane> = Arc::new(MilvusDataPlane::new(milvus_config));
    data_plane.ensure_schema().await?;
    Ok(Some(data_plane))
}

pub(crate) fn build_worker_triplet_llm(config: &AppConfig) -> Option<Arc<avrag_llm::LlmClient>> {
    config
        .triplet_llm
        .to_llm_config()
        .map(avrag_llm::LlmClient::new)
        .map(Arc::new)
}

pub(crate) fn build_worker_ingestion_llm(config: &AppConfig) -> Option<Arc<avrag_llm::LlmClient>> {
    config
        .ingestion_llm
        .to_llm_config()
        .map(avrag_llm::LlmClient::new)
        .map(Arc::new)
}

pub(crate) fn safe_relative_object_key(value: &str) -> bool {
    if value.is_empty()
        || value.contains("..")
        || value.contains("://")
        || value.starts_with('/')
        || value.starts_with('\\')
    {
        return false;
    }
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("s3://")
        || lower.starts_with("object://")
    {
        return false;
    }
    !Path::new(value).is_absolute()
}

pub(crate) fn worker_poll_interval() -> Duration {
    if let Ok(ms) = std::env::var("AVRAG_WORKER_POLL_MILLIS") {
        if let Ok(ms) = ms.parse::<u64>() {
            return Duration::from_millis(ms.max(50));
        }
    }
    let secs = std::env::var("AVRAG_WORKER_POLL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(5);
    Duration::from_secs(secs.max(1))
}

pub(crate) fn worker_health_port() -> u16 {
    std::env::var("AVRAG_WORKER_HEALTH_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8081)
}

fn publish_worker_health_port(port: u16) {
    let Ok(path) = std::env::var("AVRAG_WORKER_HEALTH_PORT_FILE") else {
        return;
    };
    if let Err(error) = std::fs::write(&path, port.to_string()) {
        warn!(%error, %path, "failed to publish worker health port");
    }
}

pub(crate) fn spawn_health_listener(port: u16) {
    tokio::spawn(async move {
        let bind_addr = if port == 0 {
            "127.0.0.1:0".to_string()
        } else {
            format!("127.0.0.1:{port}")
        };
        let listener = match tokio::net::TcpListener::bind(&bind_addr).await {
            Ok(listener) => listener,
            Err(error) => {
                warn!(%error, %bind_addr, "worker health listener failed to bind");
                return;
            }
        };
        let bound_port = listener
            .local_addr()
            .map(|addr| addr.port())
            .unwrap_or(port);
        publish_worker_health_port(bound_port);
        let addr = format!("127.0.0.1:{bound_port}");
        info!(%addr, "worker health listener started");
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                continue;
            };
            tokio::spawn(async move {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf).await;
                let body = b"ok";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.write_all(body).await;
            });
        }
    });
}

pub(crate) fn worker_runtime_mode(database_url: &Option<String>) -> &'static str {
    if database_url.is_some() {
        "postgres"
    } else {
        "memory"
    }
}

fn has_complete_s3_config(config: &AppConfig) -> bool {
    !config.object_storage.endpoint.trim().is_empty()
        && !config.object_storage.bucket.trim().is_empty()
        && !config.object_storage.access_key.trim().is_empty()
        && !config.object_storage.secret_key.trim().is_empty()
}

fn worker_skip_storage_probe() -> bool {
    std::env::var("AVRAG_WORKER_SKIP_STORAGE_PROBE")
        .ok()
        .map(|value| is_truthy(&value))
        .unwrap_or(false)
}

fn is_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn mask_endpoint(endpoint: &str) -> String {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return "(empty)".to_string();
    }
    if let Ok(mut url) = reqwest::Url::parse(trimmed) {
        if !url.username().is_empty() || url.password().is_some() {
            let _ = url.set_username("****");
            let _ = url.set_password(Some("****"));
        }
        url.set_query(None);
        return url.to_string();
    }
    trimmed.to_string()
}
