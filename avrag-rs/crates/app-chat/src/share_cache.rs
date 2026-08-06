//! Exact-match share Q&A cache (ADR-0010 §9) — process-local, TTL, no semantic match.
//!
//! When a visitor repeats the **same** query on the same share token within TTL,
//! platform LLM spend is skipped (near-zero marginal cost for scrape bots).

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

#[derive(Clone)]
struct Entry {
    answer: String,
    expires_at: Instant,
}

static CACHE: LazyLock<Mutex<HashMap<String, Entry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn ttl_secs() -> u64 {
    std::env::var("SHARE_QA_CACHE_TTL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(600)
}

fn enabled() -> bool {
    !matches!(
        std::env::var("SHARE_QA_CACHE_DISABLED").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

fn cache_key(share_token: &str, query: &str) -> String {
    let normalized = query.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut h = Sha256::new();
    h.update(share_token.trim().as_bytes());
    h.update(b"\0");
    h.update(normalized.to_lowercase().as_bytes());
    format!("{:x}", h.finalize())
}

/// Lookup cached answer for share exact query match.
pub fn get(share_token: &str, query: &str) -> Option<String> {
    if !enabled() || share_token.trim().is_empty() || query.trim().is_empty() {
        return None;
    }
    let key = cache_key(share_token, query);
    let mut guard = CACHE.lock().ok()?;
    let now = Instant::now();
    if let Some(entry) = guard.get(&key) {
        if entry.expires_at > now {
            return Some(entry.answer.clone());
        }
        guard.remove(&key);
    }
    // Opportunistic prune of a few expired keys.
    if guard.len() > 10_000 {
        guard.retain(|_, e| e.expires_at > now);
    }
    None
}

/// Store share answer for exact reuse within TTL.
pub fn put(share_token: &str, query: &str, answer: &str) {
    if !enabled() || share_token.trim().is_empty() || query.trim().is_empty() {
        return;
    }
    if answer.trim().is_empty() {
        return;
    }
    let key = cache_key(share_token, query);
    let ttl = Duration::from_secs(ttl_secs().max(30));
    if let Ok(mut guard) = CACHE.lock() {
        guard.insert(
            key,
            Entry {
                answer: answer.to_string(),
                expires_at: Instant::now() + ttl,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_exact_query() {
        put("tok-a", "What is X?", "answer-x");
        assert_eq!(get("tok-a", "What is X?").as_deref(), Some("answer-x"));
        assert_eq!(get("tok-a", "what  is   x?").as_deref(), Some("answer-x"));
        assert!(get("tok-b", "What is X?").is_none());
    }
}
