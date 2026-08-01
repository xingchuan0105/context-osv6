use common::AppError;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

use crate::config::ModelProviderConfig;

type HmacSha256 = Hmac<Sha256>;

pub(crate) fn default_object_root() -> String {
    format!(
        "{}/.local/share/avrag-dev/objects",
        std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string())
    )
}

pub(crate) fn build_redis_url(addr: &str, password: &str, db: i64) -> String {
    if password.trim().is_empty() {
        format!("redis://{addr}/{db}")
    } else {
        format!("redis://:{password}@{addr}/{db}")
    }
}

pub(crate) fn upload_signing_secret() -> String {
    std::env::var("AVRAG_UPLOAD_SIGNING_SECRET")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "context-osv6-local-upload-secret".to_string())
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

pub fn parse_uuid_or_app_error(
    value: &str,
    code: &'static str,
    message: &'static str,
) -> Result<Uuid, AppError> {
    Uuid::parse_str(value).map_err(|_| AppError::not_found(code, message))
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

pub(crate) fn is_remote_asset_reference(value: &str) -> bool {
    common::is_remote_url(value)
}

/// One fallback entry parsed from the `{PREFIX}_FALLBACKS` JSON array.
#[derive(serde::Deserialize)]
struct FallbackProviderJson {
    base_url: String,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    api_keys: Option<Vec<String>>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
    #[serde(default)]
    rpm_limit: Option<u32>,
    #[serde(default)]
    tpm_limit: Option<u32>,
    #[serde(default)]
    enable_thinking: Option<bool>,
    #[serde(default)]
    enable_cache: Option<bool>,
    #[serde(default)]
    api_style: Option<String>,
}

/// Build a multi-provider LLM pool from `{PREFIX}_API_KEYS` (comma-separated
/// extra keys for the primary provider) and `{PREFIX}_FALLBACKS` (JSON array
/// of fallback providers). Returns `None` when neither is configured, so the
/// caller keeps the single-route path unchanged.
pub(crate) fn llm_pool_config_from_env(
    prefix: &str,
    primary: &ModelProviderConfig,
) -> Option<avrag_llm::LlmPoolConfig> {
    let empty: Vec<String> = Vec::new();
    let primary_keys = env_csv(&format!("{prefix}_API_KEYS"), &empty);
    let fallback_json = env_optional_string(&format!("{prefix}_FALLBACKS"));
    if primary_keys.is_empty() && fallback_json.is_none() {
        return None;
    }

    let primary_llm = primary.to_llm_config()?;
    // `{PREFIX}_API_KEYS` are *extra* keys: the configured primary key stays
    // first in the rotation (deduplicated).
    let mut keys = primary_keys;
    if !primary.api_key.is_empty() && !keys.contains(&primary.api_key) {
        keys.insert(0, primary.api_key.clone());
    }
    if keys.is_empty() {
        return None;
    }
    let mut members = vec![avrag_llm::PoolMemberConfig::with_keys(primary_llm, keys)];

    if let Some(json) = fallback_json {
        match serde_json::from_str::<Vec<FallbackProviderJson>>(&json) {
            Ok(entries) => {
                for entry in entries {
                    if let Some(member) = fallback_member_from_json(entry, primary) {
                        members.push(member);
                    }
                }
            }
            Err(err) => tracing::warn!(
                "{prefix}_FALLBACKS is not a valid JSON array; ignoring fallbacks: {err}"
            ),
        }
    }

    let cooldown_secs = env_u64(
        &format!("{prefix}_FAILOVER_COOLDOWN_SECS"),
        avrag_llm::DEFAULT_COOLDOWN_SECS,
    );
    Some(avrag_llm::LlmPoolConfig {
        members,
        cooldown_secs,
    })
}

fn fallback_member_from_json(
    entry: FallbackProviderJson,
    primary: &ModelProviderConfig,
) -> Option<avrag_llm::PoolMemberConfig> {
    let api_keys = entry
        .api_keys
        .filter(|keys| !keys.is_empty())
        .or_else(|| entry.api_key.clone().map(|key| vec![key]))
        .unwrap_or_default();
    if entry.base_url.trim().is_empty() || api_keys.is_empty() {
        tracing::warn!("LLM fallback entry missing base_url/api_key; ignoring it");
        return None;
    }
    let config = avrag_llm::ModelProviderConfig {
        base_url: entry.base_url,
        api_key: api_keys.first().cloned().unwrap_or_default(),
        model: entry.model.unwrap_or_else(|| primary.model.clone()),
        timeout_ms: entry.timeout_ms.unwrap_or(primary.timeout_ms),
        api_style: entry
            .api_style
            .as_deref()
            .and_then(avrag_llm::ApiStyle::from_config_str),
        dimensions: None,
        enable_thinking: entry.enable_thinking,
        enable_cache: entry.enable_cache,
        rpm_limit: entry.rpm_limit,
        tpm_limit: entry.tpm_limit,
    };
    Some(avrag_llm::PoolMemberConfig::with_keys(config, api_keys))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Restores env vars on drop (edition-2024-safe wrapper around set_var).
    struct EnvGuard(Vec<(String, Option<String>)>);

    impl EnvGuard {
        fn set(key: &str, value: &str) -> Self {
            let old = std::env::var(key).ok();
            unsafe { std::env::set_var(key, value) };
            Self(vec![(key.to_string(), old)])
        }

        fn remove(key: &str) -> Self {
            let old = std::env::var(key).ok();
            unsafe { std::env::remove_var(key) };
            Self(vec![(key.to_string(), old)])
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, old) in &self.0 {
                match old {
                    Some(value) => unsafe { std::env::set_var(key, value) },
                    None => unsafe { std::env::remove_var(key) },
                }
            }
        }
    }

    fn primary() -> ModelProviderConfig {
        ModelProviderConfig {
            base_url: "https://api.deepseek.com".to_string(),
            api_key: "pk".to_string(),
            model: "deepseek-chat".to_string(),
            timeout_ms: 180000,
            temperature: Some(0.2),
            api_style: Some("openai".to_string()),
            dimensions: None,
            enable_thinking: Some(true),
            enable_cache: None,
            rpm_limit: None,
            tpm_limit: None,
        }
    }

    #[test]
    fn no_extra_config_returns_none() {
        let _g1 = EnvGuard::remove("TESTPOOL_API_KEYS");
        let _g2 = EnvGuard::remove("TESTPOOL_FALLBACKS");
        assert!(llm_pool_config_from_env("TESTPOOL", &primary()).is_none());
    }

    #[test]
    fn api_keys_csv_builds_multi_key_primary_member() {
        let _g = EnvGuard::set("TESTPOOL_API_KEYS", "k1,k2,k3");
        let pool = llm_pool_config_from_env("TESTPOOL", &primary()).expect("pool");
        assert_eq!(pool.members.len(), 1);
        // The configured primary key stays first; API_KEYS are extras.
        assert_eq!(pool.members[0].api_keys, vec!["pk", "k1", "k2", "k3"]);
        assert_eq!(pool.members[0].config.base_url, "https://api.deepseek.com");
    }

    #[test]
    fn fallbacks_json_appends_members_after_primary() {
        let _g1 = EnvGuard::remove("TESTPOOL_API_KEYS");
        let _g2 = EnvGuard::set(
            "TESTPOOL_FALLBACKS",
            r#"[{"base_url":"https://open.bigmodel.cn/api/paas/v4","api_key":"z1","model":"glm-4.6"},{"base_url":"https://api.siliconflow.cn/v1","api_keys":["s1","s2"]}]"#,
        );
        let pool = llm_pool_config_from_env("TESTPOOL", &primary()).expect("pool");
        assert_eq!(pool.members.len(), 3);
        assert_eq!(pool.members[0].config.base_url, "https://api.deepseek.com");
        assert_eq!(pool.members[1].api_keys, vec!["z1"]);
        assert_eq!(pool.members[1].config.model, "glm-4.6");
        // model falls back to the primary when omitted.
        assert_eq!(pool.members[2].config.model, "deepseek-chat");
        assert_eq!(pool.members[2].api_keys, vec!["s1", "s2"]);
    }

    #[test]
    fn invalid_fallback_json_keeps_primary_member() {
        let _g1 = EnvGuard::remove("TESTPOOL_API_KEYS");
        let _g2 = EnvGuard::set("TESTPOOL_FALLBACKS", "not-json");
        let pool = llm_pool_config_from_env("TESTPOOL", &primary()).expect("pool");
        assert_eq!(pool.members.len(), 1);
        assert_eq!(pool.members[0].api_keys, vec!["pk"]);
    }
}
