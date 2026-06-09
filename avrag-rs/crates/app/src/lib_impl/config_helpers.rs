use crate::agents::service::UnifiedAgentService;
use anyhow::Result as AnyResult;
use avrag_auth::{ActorId, AuthContext, OrgId, SubjectKind};
use avrag_llm::{EmbeddingClient, LlmClient, RerankerClient, RetrievalPlanner};
use avrag_rag_core::RagRuntime;
use avrag_search::SearchExecutor;
use avrag_storage_pg::{ObjectStoreHandle, PgStorageError, S3ObjectStore};
use common::AppError;
use hmac::{Hmac, Mac};

type HmacSha256 = Hmac<sha2::Sha256>;
use ingestion::parser::{DocumentParser, HtmlParser};
use reqwest::{Client as HttpClient, Url, header::CONTENT_TYPE, redirect::Policy};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{fs, time::Duration};
use uuid::Uuid;

use crate::lib_impl::*;

pub(crate) fn auth_context_from_config(config: &AppConfig) -> AuthContext {
    let org_uuid = Uuid::parse_str(&config.org_id).unwrap_or_else(|_| Uuid::nil());
    let user_uuid = Uuid::parse_str(&config.user_id).unwrap_or_else(|_| Uuid::nil());
    AuthContext::new(OrgId::from(org_uuid), SubjectKind::User)
        .with_actor_id(ActorId::new(user_uuid))
        .with_request_id("config-bootstrap")
}

impl ModelProviderConfig {
    pub fn to_llm_config(&self) -> Option<avrag_llm::ModelProviderConfig> {
        if self.api_key.is_empty() || self.base_url.is_empty() {
            return None;
        }
        Some(avrag_llm::ModelProviderConfig {
            base_url: self.base_url.clone(),
            api_key: self.api_key.clone(),
            model: self.model.clone(),
            timeout_ms: self.timeout_ms,
            api_style: self
                .api_style
                .as_deref()
                .and_then(avrag_llm::ApiStyle::from_config_str),
            dimensions: self.dimensions,
            enable_thinking: self.enable_thinking,
            enable_cache: self.enable_cache,
            rpm_limit: self.rpm_limit,
            tpm_limit: self.tpm_limit,
        })
    }
}

pub(crate) fn make_llm_client(config: &ModelProviderConfig) -> Option<LlmClient> {
    config.to_llm_config().map(LlmClient::new)
}

pub(crate) fn build_unified_agent_service(
    llm_client: Option<LlmClient>,
    search_executor: Option<Arc<SearchExecutor>>,
    rag_runtime: Option<Arc<RagRuntime>>,
    _prompts_dir: &str,
) -> Arc<UnifiedAgentService> {
    let search_provider: Option<Arc<dyn avrag_search::SearchProvider>> =
        search_executor.map(|executor| -> Arc<dyn avrag_search::SearchProvider> { executor });

    let agent = crate::agents::unified::UnifiedAgent::new(llm_client.clone())
        .with_rag_runtime(rag_runtime)
        .with_search_executor(search_provider);

    Arc::new(UnifiedAgentService::new(Box::new(agent)))
}

pub(crate) fn make_embedding_client(
    config: &ModelProviderConfig,
    cache: Option<Arc<avrag_cache_redis::CacheStore>>,
) -> Option<Arc<EmbeddingClient>> {
    config.to_llm_config().map(|c| {
        let client = EmbeddingClient::new(c);
        let client = if let Some(cache) = cache {
            client.with_cache(cache)
        } else {
            client
        };
        Arc::new(client)
    })
}

pub(crate) fn make_planner(
    config: &ModelProviderConfig,
    cache: Option<Arc<avrag_cache_redis::CacheStore>>,
) -> Option<Arc<RetrievalPlanner>> {
    config.to_llm_config().map(|c| {
        let planner = RetrievalPlanner::new(c);
        let planner = if let Some(cache) = cache {
            planner.with_cache(cache)
        } else {
            planner
        };
        Arc::new(planner)
    })
}

pub(crate) fn make_reranker(config: &ModelProviderConfig) -> Option<Arc<RerankerClient>> {
    config
        .to_llm_config()
        .map(|c| Arc::new(RerankerClient::new(c)))
}

pub(crate) fn default_object_root() -> String {
    format!(
        "{}/.local/share/avrag-dev/objects",
        std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string())
    )
}

pub(crate) async fn write_raw_object(
    object_root: &Path,
    object_path: &str,
    bytes: &[u8],
) -> std::io::Result<()> {
    let full_path = object_root.join(PathBuf::from(object_path));
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::write(full_path, bytes).await
}

pub(crate) fn upload_signing_secret() -> String {
    std::env::var("AVRAG_UPLOAD_SIGNING_SECRET")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "context-osv6-local-upload-secret".to_string())
}

pub(crate) async fn build_object_store(config: &AppConfig) -> AnyResult<ObjectStoreHandle> {
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

pub(crate) fn sign_upload_payload(
    secret: &str,
    document_id: &str,
    object_path: &str,
    expires: u64,
) -> Result<String, AppError> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|error| AppError::internal(format!("upload signer init failed: {error}")))?;
    mac.update(document_id.as_bytes());
    mac.update(b":");
    mac.update(object_path.as_bytes());
    mac.update(b":");
    mac.update(expires.to_string().as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

pub(crate) fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            other => other,
        })
        .collect()
}

const URL_IMPORT_MAX_BYTES: usize = 5 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct UrlImportPayload {
    pub(crate) filename: String,
    pub(crate) mime_type: String,
    pub(crate) raw_bytes: Vec<u8>,
    pub(crate) extracted_content: String,
}

pub(crate) async fn fetch_url_import(raw_url: &str) -> Result<UrlImportPayload, AppError> {
    let url = Url::parse(raw_url)
        .map_err(|_| AppError::validation("invalid_url", "url must be a valid absolute URL"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::validation(
            "invalid_url_scheme",
            "url must start with http:// or https://",
        ));
    }

    let client = HttpClient::builder()
        .redirect(Policy::limited(5))
        .timeout(Duration::from_secs(20))
        .user_agent("avrag-url-import/1.0")
        .build()
        .map_err(|error| AppError::internal(format!("failed to build url importer: {error}")))?;

    let response = client
        .get(url.clone())
        .send()
        .await
        .map_err(|error| AppError::validation("url_fetch_failed", error.to_string()))?;
    let status = response.status();
    if !status.is_success() {
        return Err(AppError::validation(
            "url_fetch_failed",
            format!("url returned HTTP {status}"),
        ));
    }
    if response
        .content_length()
        .is_some_and(|len| len as usize > URL_IMPORT_MAX_BYTES)
    {
        return Err(AppError::validation(
            "url_too_large",
            format!(
                "url content exceeds {} MB",
                URL_IMPORT_MAX_BYTES / 1024 / 1024
            ),
        ));
    }

    let final_url = response.url().clone();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    let raw_bytes = response
        .bytes()
        .await
        .map_err(|error| AppError::validation("url_fetch_failed", error.to_string()))?
        .to_vec();
    if raw_bytes.is_empty() {
        return Err(AppError::validation(
            "url_empty_content",
            "the fetched url returned empty content",
        ));
    }
    if raw_bytes.len() > URL_IMPORT_MAX_BYTES {
        return Err(AppError::validation(
            "url_too_large",
            format!(
                "url content exceeds {} MB",
                URL_IMPORT_MAX_BYTES / 1024 / 1024
            ),
        ));
    }

    let mime_type = infer_url_import_mime_type(&content_type, &raw_bytes).to_string();
    let provisional_filename = build_url_source_filename(&final_url, &mime_type, None);
    let (extracted_content, title_hint) =
        extract_url_import_content(&raw_bytes, &mime_type, &provisional_filename).await?;
    let extracted_content = normalize_imported_text(&extracted_content);
    if extracted_content.is_empty() {
        return Err(AppError::validation(
            "url_empty_content",
            "the fetched url did not contain readable text",
        ));
    }

    Ok(UrlImportPayload {
        filename: build_url_source_filename(&final_url, &mime_type, title_hint.as_deref()),
        mime_type,
        raw_bytes,
        extracted_content,
    })
}

pub(crate) fn infer_url_import_mime_type(content_type: &str, bytes: &[u8]) -> &'static str {
    let normalized = content_type
        .split(';')
        .next()
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if normalized.contains("html") || looks_like_html(bytes) {
        "text/html"
    } else if normalized.contains("json") {
        "application/json"
    } else if normalized.contains("xml") {
        "application/xml"
    } else {
        "text/plain"
    }
}

pub(crate) async fn extract_url_import_content(
    bytes: &[u8],
    mime_type: &str,
    filename: &str,
) -> Result<(String, Option<String>), AppError> {
    if mime_type == "text/html" {
        let parsed = HtmlParser
            .parse(bytes, filename)
            .await
            .map_err(map_anyhow_error)?;
        let content = parsed
            .pages
            .iter()
            .map(|page| page.content.trim())
            .filter(|page| !page.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        let title = (!parsed.title.trim().is_empty()).then_some(parsed.title);
        return Ok((content, title));
    }

    Ok((String::from_utf8_lossy(bytes).into_owned(), None))
}

pub(crate) fn looks_like_html(bytes: &[u8]) -> bool {
    let prefix = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).to_ascii_lowercase();
    prefix.contains("<html")
        || prefix.contains("<body")
        || prefix.contains("<article")
        || prefix.contains("<!doctype html")
}

pub(crate) fn build_url_source_filename(
    url: &Url,
    mime_type: &str,
    title_hint: Option<&str>,
) -> String {
    let extension = match mime_type {
        "text/html" => "html",
        "application/json" => "json",
        "application/xml" => "xml",
        _ => "txt",
    };
    let base = title_hint
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(sanitize_filename)
        .or_else(|| {
            url.path_segments()
                .and_then(|segments| segments.rev().find(|segment| !segment.trim().is_empty()))
                .map(sanitize_filename)
                .filter(|value| !value.is_empty())
        })
        .filter(|value| value != "." && value != "..")
        .unwrap_or_else(|| {
            url.host_str()
                .map(sanitize_filename)
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "url-source".to_string())
        });

    if base
        .rsplit('.')
        .next()
        .is_some_and(|value| value == extension)
    {
        base
    } else {
        format!("{base}.{extension}")
    }
}

pub(crate) fn normalize_imported_text(content: &str) -> String {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn parse_uuid_or_app_error(
    value: &str,
    code: &'static str,
    message: &'static str,
) -> Result<Uuid, AppError> {
    Uuid::parse_str(value).map_err(|_| AppError::not_found(code, message))
}

pub(crate) fn map_pg_error(error: PgStorageError) -> AppError {
    match error {
        PgStorageError::NotFound(message) => AppError::not_found("not_found", message),
        other => AppError::internal(other.to_string()),
    }
}

pub(crate) fn map_anyhow_error(error: anyhow::Error) -> AppError {
    AppError::internal(error.to_string())
}

pub(crate) fn env_string(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

pub(crate) fn env_optional_string(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn env_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        })
        .unwrap_or(default)
}

pub(crate) fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

pub(crate) fn env_i64(key: &str, default: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .unwrap_or(default)
}

pub(crate) fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(default)
}

pub(crate) fn env_f32_optional(key: &str, default: Option<f32>) -> Option<f32> {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<f32>().ok())
        .or(default)
}

pub(crate) fn env_u32_optional(key: &str, default: Option<u32>) -> Option<u32> {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .or(default)
}

pub(crate) fn env_bool_optional(key: &str) -> Option<bool> {
    std::env::var(key).ok().map(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

pub(crate) fn env_usize_optional(key: &str) -> Option<usize> {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
}

pub(crate) fn env_csv(key: &str, default: &[String]) -> Vec<String> {
    std::env::var(key)
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|values| !values.is_empty())
        .unwrap_or_else(|| default.to_vec())
}

pub(crate) fn model_config_from_env(
    prefix: &str,
    default: &ModelProviderConfig,
    fallback_api_key: Option<String>,
) -> ModelProviderConfig {
    let api_key = env_optional_string(&format!("{prefix}_API_KEY"))
        .or(fallback_api_key)
        .unwrap_or_else(|| default.api_key.clone());
    let model = env_string(&format!("{prefix}_MODEL"), &default.model);
    ModelProviderConfig {
        base_url: env_string(&format!("{prefix}_BASE_URL"), &default.base_url),
        api_key,
        model: model.clone(),
        timeout_ms: env_u64(&format!("{prefix}_TIMEOUT_MS"), default.timeout_ms),
        temperature: env_f32_optional(&format!("{prefix}_TEMPERATURE"), default.temperature),
        api_style: env_optional_string(&format!("{prefix}_API_STYLE"))
            .or_else(|| default.api_style.clone()),
        dimensions: env_usize_optional(&format!("{prefix}_DIMENSIONS"))
            .or(default.dimensions)
            .or_else(|| inferred_embedding_dimensions(&model)),
        enable_thinking: env_bool_optional(&format!("{prefix}_ENABLE_THINKING"))
            .or(default.enable_thinking),
        enable_cache: env_bool_optional(&format!("{prefix}_ENABLE_CACHE")).or(default.enable_cache),
        rpm_limit: env_u32_optional(&format!("{prefix}_RPM_LIMIT"), default.rpm_limit),
        tpm_limit: env_u32_optional(&format!("{prefix}_TPM_LIMIT"), default.tpm_limit),
    }
}

pub(crate) fn inferred_embedding_dimensions(model: &str) -> Option<usize> {
    match model.trim() {
        "text-embedding-v4" | "text-embedding-v3" => Some(1024),
        "text-embedding-v2" => Some(1536),
        _ => None,
    }
}
