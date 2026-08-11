//! CRW (https://github.com/us/crw) scrape client — Firecrawl-compatible `/v1/scrape`.
//!
//! Used as host-side URL reader after web search (auto-enrich) and optionally for
//! product `client.fetch`. Prefer HTTP to a local Docker/binary process; do not
//! link the AGPL engine into this crate.

use std::collections::HashSet;
use std::time::Duration;

use reqwest::Client;
use tracing::{debug, info, warn};

use crate::{SearchConfig, SearchResult};

/// Default local CRW listen address (docker / binary).
/// Note: product Next.js uses :3000 — CRW defaults to :3100.
pub const DEFAULT_CRW_BASE_URL: &str = "http://127.0.0.1:3100";

/// Scrape one URL via CRW; returns markdown/text (may be empty on soft failure).
pub async fn scrape_url(
    client: &Client,
    base_url: &str,
    api_key: &str,
    url: &str,
    timeout_ms: u64,
    max_chars: usize,
) -> anyhow::Result<String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        anyhow::bail!("CRW base URL is empty");
    }
    let endpoint = format!("{base}/v1/scrape");
    let body = serde_json::json!({
        "url": url,
        "formats": ["markdown"],
    });

    let mut req = client
        .post(&endpoint)
        .timeout(Duration::from_millis(timeout_ms.max(1_000)))
        .header("Content-Type", "application/json")
        .json(&body);
    let key = api_key.trim();
    if !key.is_empty() {
        req = req.header("Authorization", format!("Bearer {key}"));
    }

    let response = req.send().await?;
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!("crw scrape HTTP {status}: {text}");
    }

    let value: serde_json::Value = response.json().await?;
    let markdown = extract_markdown_from_scrape_body(&value);
    if markdown.trim().is_empty() {
        anyhow::bail!("crw scrape returned empty markdown for {url}");
    }
    Ok(truncate_chars(markdown.trim(), max_chars.max(256)))
}

/// After web search: fill thin snippets by scraping top-K unique URLs via CRW.
///
/// No-op when auto-scrape disabled or `crw_base_url` empty. Failures on single
/// URLs are logged and skipped (search still returns title/url shells).
pub async fn auto_scrape_enrich_results(
    config: &SearchConfig,
    client: &Client,
    results: &mut [SearchResult],
) {
    if !config.auto_scrape_enabled {
        return;
    }
    let base = config.crw_base_url.trim();
    if base.is_empty() {
        debug!(target: "search", "auto_scrape enabled but CRW_BASE_URL empty; skip");
        return;
    }

    let top_k = config.auto_scrape_top_k.max(1);
    let min_snippet = config.auto_scrape_min_snippet;
    let max_chars = config.auto_scrape_max_chars.max(256);
    let timeout_ms = config.auto_scrape_timeout_ms.max(1_000);
    let concurrency = config.auto_scrape_concurrency.max(1);

    // Indices of results that need enrichment (thin snippet), first-seen URL wins.
    let mut seen = HashSet::new();
    let mut plan: Vec<(usize, String)> = Vec::new();
    for (idx, r) in results.iter().enumerate() {
        let url = r.url.trim();
        if url.is_empty() || !url.starts_with("http") {
            continue;
        }
        if r.snippet.trim().chars().count() >= min_snippet {
            continue;
        }
        if !seen.insert(url.to_string()) {
            continue;
        }
        plan.push((idx, url.to_string()));
        if plan.len() >= top_k {
            break;
        }
    }

    if plan.is_empty() {
        return;
    }

    info!(
        target: "search",
        n = plan.len(),
        top_k,
        base,
        "auto_scrape enrich starting"
    );

    // Chunk by concurrency.
    for chunk in plan.chunks(concurrency) {
        let futs: Vec<_> = chunk
            .iter()
            .map(|(idx, url)| {
                let client = client.clone();
                let base = base.to_string();
                let key = config.crw_api_key.clone();
                let url = url.clone();
                let idx = *idx;
                async move {
                    let out = scrape_url(&client, &base, &key, &url, timeout_ms, max_chars).await;
                    (idx, url, out)
                }
            })
            .collect();
        let settled = futures::future::join_all(futs).await;
        for (idx, url, out) in settled {
            match out {
                Ok(md) => {
                    if let Some(slot) = results.get_mut(idx) {
                        // Prefer scraped body as primary evidence text.
                        slot.snippet = md;
                    }
                    debug!(target: "search", %url, "auto_scrape ok");
                }
                Err(e) => {
                    warn!(target: "search", %url, error = %e, "auto_scrape failed; keep title/url");
                }
            }
        }
    }
}

fn extract_markdown_from_scrape_body(value: &serde_json::Value) -> String {
    // Cloud/native: { "success": true, "data": { "markdown": "..." } }
    if let Some(md) = value
        .pointer("/data/markdown")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return md.to_string();
    }
    if let Some(md) = value
        .get("markdown")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return md.to_string();
    }
    // Fallback fields some forks use.
    for path in ["/data/content", "/data/text", "/content", "/text"] {
        if let Some(s) = value
            .pointer(path)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            return s.to_string();
        }
    }
    String::new()
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_chars).collect();
    out.push_str("…[truncated]");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_nested_markdown() {
        let v = serde_json::json!({
            "success": true,
            "data": { "markdown": "# Hello\n\nWorld" }
        });
        assert_eq!(extract_markdown_from_scrape_body(&v), "# Hello\n\nWorld");
    }

    #[test]
    fn truncate_respects_char_budget() {
        let s = "你好".repeat(100);
        let t = truncate_chars(&s, 10);
        assert!(t.chars().count() <= 10 + "…[truncated]".chars().count());
        assert!(t.contains("truncated"));
    }
}
