use super::ChatMessage;
use crate::ModelProviderConfig;

/// Process-wide shared token buckets keyed by (base_url, model, api_key hash,
/// limits). Platform clients are built once at bootstrap, but BYOK clients are
/// rebuilt per request — without this registry each request gets a fresh full
/// bucket and the RPM/TPM limits never actually throttle.
static SHARED_LIMITERS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, crate::SharedRateLimiter>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Hard cap on distinct buckets; on overflow the map is cleared (buckets reset
/// — acceptable for a pathological BYOK cardinality spike).
const MAX_SHARED_LIMITERS: usize = 4096;

fn shared_limiter(config: &ModelProviderConfig) -> crate::SharedRateLimiter {
    use std::hash::{Hash, Hasher};
    let rpm = config.effective_rpm_limit();
    let tpm = config.effective_tpm_limit();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    config.base_url.hash(&mut hasher);
    config.model.hash(&mut hasher);
    config.api_key.hash(&mut hasher);
    let key = format!("{}:{}:{}", hasher.finish(), rpm, tpm);
    let mut map = SHARED_LIMITERS.lock().unwrap_or_else(|p| p.into_inner());
    if map.len() >= MAX_SHARED_LIMITERS {
        map.clear();
    }
    map.entry(key)
        .or_insert_with(|| std::sync::Arc::new(crate::RateLimiter::new(rpm, tpm)))
        .clone()
}

#[derive(Debug, Clone)]
pub(crate) struct ClientRateLimit {
    limiter: Option<crate::SharedRateLimiter>,
}

impl ClientRateLimit {
    pub(crate) fn from_config(config: &ModelProviderConfig) -> Self {
        let limiter = if config.is_configured() {
            Some(shared_limiter(config))
        } else {
            None
        };
        Self { limiter }
    }

    pub(crate) fn estimate_input_tokens(&self, messages: &[ChatMessage]) -> usize {
        crate::count_chat_messages(messages)
    }

    pub(crate) fn check_rate_limit(&self, estimated_tokens: usize) -> anyhow::Result<usize> {
        if let Some(limiter) = &self.limiter {
            match limiter.check_request(estimated_tokens) {
                Ok(deducted) => Ok(deducted),
                Err(crate::RateLimitError::RpmExceeded) => {
                    anyhow::bail!("LLM rate limit exceeded: too many requests per minute")
                }
                Err(crate::RateLimitError::TpmExceeded) => {
                    anyhow::bail!("LLM rate limit exceeded: too many tokens per minute")
                }
            }
        } else {
            Ok(estimated_tokens)
        }
    }

    pub(crate) fn record_usage(&self, pre_deducted: usize, actual_tokens: usize) {
        if let Some(limiter) = &self.limiter {
            limiter.record_actual_usage(pre_deducted, actual_tokens);
        }
    }
}
