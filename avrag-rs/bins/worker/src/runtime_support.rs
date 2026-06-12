use anyhow::Result;
use avrag_auth::{ActorId, AuthContext, OrgId, SubjectKind};
use avrag_retrieval_data_plane::RetrievalDataPlane;
use avrag_storage_milvus::{MilvusConfig as StorageMilvusConfig, MilvusDataPlane};
use avrag_storage_pg::{DocumentCleanupTask, ObjectStoreHandle, PgAppRepository, S3ObjectStore};
use app_core::AppConfig;
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

pub(crate) async fn build_worker_object_store(config: &AppConfig) -> Result<ObjectStoreHandle> {
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

pub(crate) async fn fetch_url_content(url: &str) -> Result<Vec<u8>> {
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

pub(crate) fn spawn_health_listener(port: u16) {
    tokio::spawn(async move {
        let addr = format!("127.0.0.1:{port}");
        let listener = match tokio::net::TcpListener::bind(&addr).await {
            Ok(listener) => listener,
            Err(error) => {
                warn!(%error, %addr, "worker health listener failed to bind");
                return;
            }
        };
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
