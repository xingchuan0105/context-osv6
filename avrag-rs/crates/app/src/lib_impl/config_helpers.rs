use avrag_storage_pg::PgStorageError;
use common::AppError;
use hmac::{Hmac, Mac};

type HmacSha256 = Hmac<sha2::Sha256>;
use uuid::Uuid;

use app_core::ModelProviderConfig;

pub(crate) fn default_object_root() -> String {
    format!(
        "{}/.local/share/avrag-dev/objects",
        std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string())
    )
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
