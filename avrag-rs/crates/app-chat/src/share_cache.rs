//! Share Q&A cache (ADR-0010 §9): exact match + embedding cosine semantic match.
//!
//! - **Exact:** normalized whitespace/case query hash. Optional `CachePort` L1
//!   (process `MemoryCache` at bootstrap) plus a process-local map.
//! - **Semantic:** cosine similarity on dense query embeddings when provided
//!   (from RagRuntime embedding client). Threshold default 0.90.
//!   Stays process-local: a miss costs one LLM call.
//!
//! Process-local layers are TTL-bounded. Near-zero platform cost on cache hit.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use std::time::{Duration, Instant};

use avrag_rag_core_ports::CachePort;
use contracts::chat::Citation;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Shared L1 exact-match cache (wired once at bootstrap; None = memory-only).
static SHARED_EXACT: OnceLock<Option<Arc<dyn CachePort>>> = OnceLock::new();

/// Wire the optional CachePort L1 for share Q&A exact matches. Called once
/// from bootstrap; safe to skip (in-map only, e.g. tests).
pub fn init_shared_cache(cache: Option<Arc<dyn CachePort>>) {
    let _ = SHARED_EXACT.set(cache);
}

fn shared_exact() -> Option<&'static Arc<dyn CachePort>> {
    SHARED_EXACT.get().and_then(|c| c.as_ref())
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CachedShareAnswer {
    pub answer: String,
    #[serde(default)]
    pub citations: Vec<Citation>,
}

#[derive(Clone)]
struct Entry {
    payload: CachedShareAnswer,
    embedding: Option<Vec<f32>>,
    expires_at: Instant,
}

/// Per share_token → list of recent answers (exact map is global by key).
static EXACT: LazyLock<Mutex<HashMap<String, Entry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static BY_TOKEN: LazyLock<Mutex<HashMap<String, Vec<String>>>> =
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

fn semantic_threshold() -> f32 {
    std::env::var("SHARE_QA_SEMANTIC_THRESHOLD")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.90)
}

fn semantic_enabled() -> bool {
    !matches!(
        std::env::var("SHARE_QA_SEMANTIC_DISABLED").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

fn normalize_query(query: &str) -> String {
    query
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn exact_key(share_token: &str, query_norm: &str) -> String {
    let mut h = Sha256::new();
    h.update(share_token.trim().as_bytes());
    h.update(b"\0");
    h.update(query_norm.as_bytes());
    format!("{:x}", h.finalize())
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na <= 0.0 || nb <= 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

fn decode_cached(raw: &str) -> CachedShareAnswer {
    match serde_json::from_str::<CachedShareAnswer>(raw) {
        Ok(parsed) if !parsed.answer.is_empty() => parsed,
        _ => CachedShareAnswer {
            answer: raw.to_string(),
            citations: Vec::new(),
        },
    }
}

fn encode_cached(payload: &CachedShareAnswer) -> String {
    serde_json::to_string(payload).unwrap_or_else(|_| payload.answer.clone())
}

/// Lookup: exact first, then semantic cosine against same share_token entries.
pub async fn lookup(
    share_token: &str,
    query: &str,
    query_embedding: Option<&[f32]>,
) -> Option<CachedShareAnswer> {
    if !enabled() || share_token.trim().is_empty() || query.trim().is_empty() {
        return None;
    }
    let qn = normalize_query(query);
    let key = exact_key(share_token, &qn);
    let now = Instant::now();
    let thr = semantic_threshold();

    // Optional CachePort L1 (process MemoryCache at bootstrap).
    if let Some(cache) = shared_exact() {
        if let Some(raw) = cache.get(&format!("share-qa:{key}")).await {
            return Some(decode_cached(&raw));
        }
    }

    {
        let mut exact = EXACT.lock().ok()?;
        if let Some(entry) = exact.get(&key) {
            if entry.expires_at > now {
                return Some(entry.payload.clone());
            }
            exact.remove(&key);
        }
        if exact.len() > 10_000 {
            exact.retain(|_, e| e.expires_at > now);
        }
    }

    if !semantic_enabled() {
        return None;
    }
    let Some(q_emb) = query_embedding.filter(|e| !e.is_empty()) else {
        return None;
    };

    let keys: Vec<String> = {
        let by = BY_TOKEN.lock().ok()?;
        by.get(share_token.trim()).cloned().unwrap_or_default()
    };
    let mut best: Option<(f32, CachedShareAnswer)> = None;
    {
        let exact = EXACT.lock().ok()?;
        for k in keys {
            let Some(entry) = exact.get(&k) else {
                continue;
            };
            if entry.expires_at <= now {
                continue;
            }
            let Some(ref emb) = entry.embedding else {
                continue;
            };
            let sim = cosine(q_emb, emb);
            if sim >= thr {
                if best.as_ref().map(|(s, _)| sim > *s).unwrap_or(true) {
                    best = Some((sim, entry.payload.clone()));
                }
            }
        }
    }
    best.map(|(_, payload)| payload)
}

/// Store answer with optional embedding for semantic reuse.
pub async fn store(
    share_token: &str,
    query: &str,
    answer: &str,
    citations: &[Citation],
    embedding: Option<Vec<f32>>,
) {
    if !enabled() || share_token.trim().is_empty() || query.trim().is_empty() {
        return;
    }
    if answer.trim().is_empty() {
        return;
    }
    let qn = normalize_query(query);
    let key = exact_key(share_token, &qn);
    let ttl = Duration::from_secs(ttl_secs().max(30));
    let payload = CachedShareAnswer {
        answer: answer.to_string(),
        citations: citations.to_vec(),
    };
    // Optional CachePort L1 (process MemoryCache at bootstrap).
    if let Some(cache) = shared_exact() {
        let encoded = encode_cached(&payload);
        let _ = cache
            .set(&format!("share-qa:{key}"), &encoded, ttl.as_secs())
            .await;
    }
    let entry = Entry {
        payload,
        embedding,
        expires_at: Instant::now() + ttl,
    };
    if let Ok(mut exact) = EXACT.lock() {
        exact.insert(key.clone(), entry);
    }
    if let Ok(mut by) = BY_TOKEN.lock() {
        let list = by.entry(share_token.trim().to_string()).or_default();
        if !list.iter().any(|k| k == &key) {
            list.push(key);
        }
        // Cap per-token list.
        if list.len() > 256 {
            let drain = list.len() - 256;
            list.drain(0..drain);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn exact_roundtrip() {
        store("tok-a", "What is X?", "answer-x", &[], None).await;
        let hit = lookup("tok-a", "What is X?", None).await;
        assert_eq!(hit.as_ref().map(|h| h.answer.as_str()), Some("answer-x"));
        assert_eq!(
            lookup("tok-a", "what  is   x?", None)
                .await
                .as_ref()
                .map(|h| h.answer.as_str()),
            Some("answer-x")
        );
        assert!(lookup("tok-b", "What is X?", None).await.is_none());
    }

    #[tokio::test]
    async fn exact_roundtrip_keeps_citations() {
        let citations = vec![Citation {
            citation_id: 1,
            doc_id: "doc-1".to_string(),
            chunk_id: Some("chunk-1".to_string()),
            page: None,
            doc_name: "Doc One".to_string(),
            preview: None,
            content: Some("chunk body".to_string()),
            score: 0.9,
            layer: None,
            chunk_type: None,
            asset_id: None,
            caption: None,
            image_url: None,
            parser_backend: None,
            source_locator: None,
            parse_run_id: None,
        }];
        store("tok-cite", "What is X?", "answer [[cite:chunk-1]]", &citations, None).await;
        let hit = lookup("tok-cite", "What is X?", None).await.expect("cached");
        assert_eq!(hit.answer, "answer [[cite:chunk-1]]");
        assert_eq!(hit.citations.len(), 1);
        assert_eq!(hit.citations[0].chunk_id.as_deref(), Some("chunk-1"));
        assert_eq!(hit.citations[0].content.as_deref(), Some("chunk body"));
    }

    #[tokio::test]
    async fn semantic_cosine_hit() {
        let emb_a = vec![1.0, 0.0, 0.0];
        let emb_b = vec![0.99, 0.1, 0.0]; // high cosine with emb_a
        store(
            "tok-s",
            "how tall is the tower?",
            "Eiffel is 330m",
            &[],
            Some(emb_a),
        )
        .await;
        let hit = lookup("tok-s", "tower height?", Some(&emb_b)).await;
        assert_eq!(hit.as_ref().map(|h| h.answer.as_str()), Some("Eiffel is 330m"));
    }

    #[test]
    fn cosine_identical_is_one() {
        let v = vec![0.5, 0.5, 0.0];
        assert!((cosine(&v, &v) - 1.0).abs() < 1e-5);
    }
}
