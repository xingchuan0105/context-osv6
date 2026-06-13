use std::collections::HashMap;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use super::PaddleOcrPageResult;

#[derive(Debug, Clone)]
pub struct PaddleResultCacheConfig {
    pub enabled: bool,
    pub ttl: Duration,
}

impl Default for PaddleResultCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ttl: Duration::from_secs(86400),
        }
    }
}

impl PaddleResultCacheConfig {
    pub fn from_env() -> Self {
        let enabled = std::env::var("PADDLE_OCR_RESULT_CACHE_ENABLED")
            .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(true);
        let ttl_secs = std::env::var("PADDLE_OCR_RESULT_CACHE_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(86400);
        Self {
            enabled,
            ttl: Duration::from_secs(ttl_secs),
        }
    }
}

#[derive(Debug, Clone)]
struct CacheEntry {
    results: Vec<PaddleOcrPageResult>,
    inserted_at: Instant,
}

/// In-memory Paddle Job result cache (§13.3).
#[derive(Debug, Default)]
pub struct PaddleResultCache {
    config: PaddleResultCacheConfig,
    entries: HashMap<String, CacheEntry>,
}

impl PaddleResultCache {
    pub fn new(config: PaddleResultCacheConfig) -> Self {
        Self {
            config,
            entries: HashMap::new(),
        }
    }

    pub fn from_env() -> Self {
        Self::new(PaddleResultCacheConfig::from_env())
    }

    pub fn cache_key(
        file_bytes: &[u8],
        page_number: u32,
        model: &str,
        payload_hash: &str,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(file_bytes);
        hasher.update(page_number.to_le_bytes());
        hasher.update(model.as_bytes());
        hasher.update(payload_hash.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn get(&mut self, key: &str) -> Option<Vec<PaddleOcrPageResult>> {
        if !self.config.enabled {
            return None;
        }
        let entry = self.entries.get(key)?;
        if entry.inserted_at.elapsed() > self.config.ttl {
            self.entries.remove(key);
            return None;
        }
        Some(entry.results.clone())
    }

    pub fn put(&mut self, key: String, results: Vec<PaddleOcrPageResult>) {
        if !self.config.enabled {
            return;
        }
        self.entries.insert(
            key,
            CacheEntry {
                results,
                inserted_at: Instant::now(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_is_stable() {
        let a = PaddleResultCache::cache_key(b"pdf", 1, "model", "payload");
        let b = PaddleResultCache::cache_key(b"pdf", 1, "model", "payload");
        assert_eq!(a, b);
    }
}
