//! Result-level cache for deterministic LLM calls (ingestion pipeline).
//!
//! Prefix caching saves re-computing a shared prompt prefix; this layer saves
//! the *whole call* for identical (model, prompt version, messages) inputs —
//! re-ingests, retries, E2E force-ingest and benchmark re-runs hit it and pay
//! zero LLM tokens.
//!
//! Key = `llm_result:v1:{sha256(model || prompt_version || messages_json)}`.
//! `prompt_version` should carry the system-prompt content (or a hash of it)
//! so editing a prompt invalidates stale outputs automatically.
//!
//! Kill switch: `INGESTION_LLM_RESULT_CACHE=0` (or `false`) bypasses the cache.

use crate::schema::ChatMessage;
use avrag_rag_core_ports::CachePort;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

const RESULT_CACHE_TTL_SECS: u64 = 7 * 24 * 60 * 60; // 7 days
const KILL_SWITCH: &str = "INGESTION_LLM_RESULT_CACHE";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedCompletion {
    pub content: String,
    pub reasoning_content: Option<String>,
}

#[derive(Clone)]
pub struct CompletionCache {
    inner: Arc<dyn CachePort>,
    enabled: bool,
}

impl CompletionCache {
    pub fn new(inner: Arc<dyn CachePort>) -> Self {
        let enabled = std::env::var(KILL_SWITCH)
            .ok()
            .map(|v| !is_disabled(&v))
            .unwrap_or(true);
        Self { inner, enabled }
    }

    /// Test-only constructor that bypasses the process-global kill-switch env
    /// (parallel tests must not mutate `std::env`).
    #[cfg(test)]
    pub(crate) fn new_with_enabled(inner: Arc<dyn CachePort>, enabled: bool) -> Self {
        Self { inner, enabled }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub async fn get(
        &self,
        model: &str,
        prompt_version: &str,
        messages: &[ChatMessage],
    ) -> Option<CachedCompletion> {
        if !self.enabled {
            return None;
        }
        let key = completion_cache_key(model, prompt_version, messages)?;
        let raw = self.inner.get(&key).await?;
        serde_json::from_str::<CachedCompletion>(&raw).ok()
    }

    pub async fn store(
        &self,
        model: &str,
        prompt_version: &str,
        messages: &[ChatMessage],
        value: &CachedCompletion,
    ) {
        if !self.enabled {
            return;
        }
        let Some(key) = completion_cache_key(model, prompt_version, messages) else {
            return;
        };
        if let Ok(raw) = serde_json::to_string(value) {
            let _ = self.inner.set(&key, &raw, RESULT_CACHE_TTL_SECS).await;
        }
    }
}

/// `INGESTION_LLM_RESULT_CACHE` kill-switch parsing: `0` / `false` disables.
fn is_disabled(value: &str) -> bool {
    let value = value.trim();
    value == "0" || value.eq_ignore_ascii_case("false")
}

/// Build the cache key; `None` when messages fail to serialize (callers then
/// skip the cache rather than collapsing every request onto one key).
fn completion_cache_key(
    model: &str,
    prompt_version: &str,
    messages: &[ChatMessage],
) -> Option<String> {
    let messages_json = serde_json::to_string(messages).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(model.as_bytes());
    hasher.update([0u8]);
    hasher.update(prompt_version.as_bytes());
    hasher.update([0u8]);
    hasher.update(messages_json.as_bytes());
    Some(format!("llm_result:v1:{}", hex::encode(hasher.finalize())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::ChatMessage;
    use std::sync::Mutex;

    struct MemCache(Mutex<std::collections::HashMap<String, (String, u64)>>);

    #[async_trait::async_trait]
    impl CachePort for MemCache {
        async fn get(&self, key: &str) -> Option<String> {
            self.0.lock().unwrap().get(key).map(|(v, _)| v.clone())
        }
        async fn set(&self, key: &str, value: &str, ttl_secs: u64) -> Result<(), String> {
            self.0
                .lock()
                .unwrap()
                .insert(key.to_string(), (value.to_string(), ttl_secs));
            Ok(())
        }
    }

    #[tokio::test]
    async fn roundtrip_stores_and_returns_cached_completion() {
        let cache = CompletionCache::new(Arc::new(MemCache(Mutex::new(Default::default()))));
        let messages = vec![
            ChatMessage::system("You extract facts."),
            ChatMessage::user("chunk: hello world"),
        ];
        assert!(cache.get("m", "v1", &messages).await.is_none());
        cache
            .store(
                "m",
                "v1",
                &messages,
                &CachedCompletion {
                    content: "fact".to_string(),
                    reasoning_content: None,
                },
            )
            .await;
        let hit = cache.get("m", "v1", &messages).await.unwrap();
        assert_eq!(hit.content, "fact");
    }

    #[tokio::test]
    async fn different_messages_or_prompt_version_miss() {
        let cache = CompletionCache::new(Arc::new(MemCache(Mutex::new(Default::default()))));
        let messages = vec![ChatMessage::user("hello")];
        cache
            .store(
                "m",
                "v1",
                &messages,
                &CachedCompletion {
                    content: "x".to_string(),
                    reasoning_content: None,
                },
            )
            .await;
        assert!(cache.get("m", "v2", &messages).await.is_none());
        assert!(
            cache
                .get("m", "v1", &[ChatMessage::user("other")])
                .await
                .is_none()
        );
        assert!(cache.get("other-model", "v1", &messages).await.is_none());
    }

    #[tokio::test]
    async fn disabled_cache_never_hits_or_stores() {
        let cache = CompletionCache::new_with_enabled(
            Arc::new(MemCache(Mutex::new(Default::default()))),
            false,
        );
        let messages = vec![ChatMessage::user("hello")];
        assert!(!cache.is_enabled());
        assert!(cache.get("m", "v1", &messages).await.is_none());
        cache
            .store(
                "m",
                "v1",
                &messages,
                &CachedCompletion {
                    content: "x".to_string(),
                    reasoning_content: None,
                },
            )
            .await;
        assert!(cache.get("m", "v1", &messages).await.is_none());
    }

    #[test]
    fn kill_switch_parsing() {
        assert!(is_disabled("0"));
        assert!(is_disabled("false"));
        assert!(is_disabled(" FALSE "));
        assert!(!is_disabled("1"));
        assert!(!is_disabled("true"));
    }
}
