use std::collections::{HashMap, HashSet};

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{SearchConfig, SearchResponse, SearchResult, SearchStreamUpdate};

const BRAVE_LLM_CONTEXT_PATH: &str = "/res/v1/llm/context";
const BRAVE_NEWS_PATH: &str = "/res/v1/news/search";

pub(crate) async fn execute_brave_llm_context(
    config: &SearchConfig,
    client: &Client,
    query: &str,
) -> anyhow::Result<SearchResponse> {
    let api_key = configured_brave_api_key(config)?;
    let endpoint = brave_llm_context_url(config);
    let response = client
        .post(endpoint)
        .header("X-Subscription-Token", api_key)
        .header("Accept", "application/json")
        .json(&BraveLlmContextRequest::new(query, config))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("brave llm context api error {}: {}", status, body);
    }

    let context_response: BraveLlmContextResponse = response.json().await?;
    Ok(search_response_from_brave_context(context_response, query))
}

pub(crate) async fn stream_brave_llm_context(
    config: &SearchConfig,
    client: &Client,
    query: &str,
    on_update: &mut impl FnMut(SearchStreamUpdate),
) -> anyhow::Result<SearchResponse> {
    on_update(SearchStreamUpdate::Searching {
        queries: vec![query.trim().to_string()],
    });
    let mut response = execute_brave_llm_context(config, client, query).await?;
    crate::crw::auto_scrape_enrich_results(config, client, &mut response.results).await;
    on_update(SearchStreamUpdate::SourcesCollected {
        results: response.results.clone(),
    });
    Ok(response)
}

fn configured_brave_api_key(config: &SearchConfig) -> anyhow::Result<&str> {
    let api_key = config.api_key.trim();
    if api_key.is_empty() {
        anyhow::bail!("Brave LLM Context API key not configured");
    }
    Ok(api_key)
}

fn brave_llm_context_url(config: &SearchConfig) -> String {
    let base = config.base_url.trim().trim_end_matches('/');
    if base.ends_with(BRAVE_LLM_CONTEXT_PATH) {
        base.to_string()
    } else {
        format!("{base}{BRAVE_LLM_CONTEXT_PATH}")
    }
}

fn brave_news_url(config: &SearchConfig) -> String {
    let base = config.base_url.trim().trim_end_matches('/');
    if base.ends_with(BRAVE_NEWS_PATH) {
        base.to_string()
    } else {
        format!("{base}{BRAVE_NEWS_PATH}")
    }
}

#[derive(Debug, Serialize)]
struct BraveLlmContextRequest<'a> {
    q: &'a str,
    count: usize,
    maximum_number_of_urls: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    search_lang: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    country: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    freshness: Option<&'a str>,
}

impl<'a> BraveLlmContextRequest<'a> {
    fn new(query: &'a str, config: &'a SearchConfig) -> Self {
        let count = config.max_results.clamp(1, 50);
        Self {
            q: query,
            count,
            maximum_number_of_urls: count,
            search_lang: config.search_lang.as_deref(),
            country: config.country.as_deref(),
            freshness: config.freshness.as_deref(),
        }
    }
}

pub(crate) async fn execute_brave_news(
    config: &SearchConfig,
    client: &Client,
    query: &str,
) -> anyhow::Result<SearchResponse> {
    let api_key = configured_brave_api_key(config)?;
    let endpoint = brave_news_url(config);

    let mut request = client
        .get(endpoint)
        .header("X-Subscription-Token", api_key)
        .header("Accept", "application/json")
        .query(&[
            ("q", query),
            ("count", &config.max_results.clamp(1, 50).to_string()),
        ]);

    if let Some(lang) = config.search_lang.as_deref() {
        request = request.query(&[("search_lang", lang)]);
    }
    if let Some(country) = config.country.as_deref() {
        request = request.query(&[("country", country)]);
    }
    if let Some(freshness) = config.freshness.as_deref() {
        request = request.query(&[("freshness", freshness)]);
    }

    let response = request.send().await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("brave news api error {}: {}", status, body);
    }

    let news_response: BraveNewsResponse = response.json().await?;
    Ok(search_response_from_brave_news(news_response, query))
}

#[derive(Debug, Deserialize)]
struct BraveNewsResponse {
    #[serde(default)]
    results: Vec<BraveNewsItem>,
}

#[derive(Debug, Deserialize)]
struct BraveNewsItem {
    title: String,
    url: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    age: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    meta_url: Option<BraveNewsMetaUrl>,
}

#[derive(Debug, Deserialize)]
struct BraveNewsMetaUrl {
    #[serde(default)]
    #[allow(dead_code)]
    hostname: Option<String>,
}

fn search_response_from_brave_news(
    response: BraveNewsResponse,
    original_query: &str,
) -> SearchResponse {
    let mut results = Vec::new();
    let mut seen_urls = HashSet::new();
    for item in response.results {
        let url = item.url.trim().to_string();
        if url.is_empty() || !seen_urls.insert(url.clone()) {
            continue;
        }
        let title = item.title.trim().to_string();
        let snippet = item
            .description
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                item.age
                    .as_deref()
                    .map(|age| format!("News article ({})", age))
                    .unwrap_or_else(|| "News article".to_string())
            });
        let citation_index = results.len() + 1;
        results.push(SearchResult {
            title,
            url,
            snippet,
            citation_index: Some(citation_index),
        });
    }

    let synthesized_answer = if results.is_empty() {
        format!(
            "No Brave News sources were found for: {}",
            original_query.trim()
        )
    } else {
        let source_lines = results
            .iter()
            .map(|result| {
                let index = result.citation_index.unwrap_or(0);
                if result.snippet.is_empty() {
                    format!("[[{index}]] {}", result.title)
                } else {
                    format!("[[{index}]] {}: {}", result.title, result.snippet)
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        format!(
            "Brave News returned these sources for '{}':\n\n{}",
            original_query.trim(),
            source_lines
        )
    };

    SearchResponse {
        query_type: "brave_news".to_string(),
        sub_queries: vec![original_query.trim().to_string()],
        results,
        synthesized_answer,
        llm_usage: None,
    }
}

#[derive(Debug, Deserialize)]
struct BraveLlmContextResponse {
    #[serde(default)]
    grounding: BraveGrounding,
    #[serde(default)]
    sources: HashMap<String, BraveSource>,
}

#[derive(Debug, Default, Deserialize)]
struct BraveGrounding {
    #[serde(default)]
    generic: Vec<BraveGroundingItem>,
    #[serde(default)]
    map: Vec<BraveGroundingItem>,
    #[serde(default)]
    poi: Option<BraveGroundingItem>,
}

#[derive(Debug, Deserialize)]
struct BraveGroundingItem {
    url: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    snippets: Vec<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BraveSource {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    hostname: Option<String>,
}

fn search_response_from_brave_context(
    response: BraveLlmContextResponse,
    original_query: &str,
) -> SearchResponse {
    let mut items = response.grounding.generic;
    if let Some(poi) = response.grounding.poi {
        items.push(poi);
    }
    items.extend(response.grounding.map);

    let mut results = Vec::new();
    let mut seen_urls = HashSet::new();
    for item in items {
        let url = item.url.trim().to_string();
        if url.is_empty() || !seen_urls.insert(url.clone()) {
            continue;
        }
        let source = response.sources.get(&url);
        let title = item
            .title
            .or(item.name)
            .or_else(|| source.and_then(|source| source.title.clone()))
            .or_else(|| source.and_then(|source| source.hostname.clone()))
            .unwrap_or_else(|| url.clone());
        let snippet = item
            .snippets
            .iter()
            .map(|snippet| snippet.trim())
            .filter(|snippet| !snippet.is_empty())
            .take(3)
            .collect::<Vec<_>>()
            .join("\n");
        let citation_index = results.len() + 1;
        results.push(SearchResult {
            title,
            url,
            snippet,
            citation_index: Some(citation_index),
        });
    }

    let synthesized_answer = if results.is_empty() {
        format!(
            "No Brave LLM Context sources were found for: {}",
            original_query.trim()
        )
    } else {
        let source_lines = results
            .iter()
            .map(|result| {
                let index = result.citation_index.unwrap_or(0);
                if result.snippet.is_empty() {
                    format!("[[{index}]] {}", result.title)
                } else {
                    format!("[[{index}]] {}: {}", result.title, result.snippet)
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        format!(
            "Brave LLM Context returned these sources for '{}':\n\n{}",
            original_query.trim(),
            source_lines
        )
    };

    SearchResponse {
        query_type: "brave_llm_context".to_string(),
        sub_queries: vec![original_query.trim().to_string()],
        results,
        synthesized_answer,
        llm_usage: None,
    }
}

// ── DeepSeek Responses API server web_search (B2 → Responses) ─────────────
// Docs: https://api-docs.deepseek.com/zh-cn/guides/responses_api
//
// Latency / reliability notes (2026-08-11 probes):
// - web_search is server-side agentic; heavy prompts ("Prefer authoritative
//   pages") induce multi-round open_page spirals (~30s, 10× tokens).
// - Non-stream POST holds a silent socket during that spiral; mid-body cutoffs
//   surface as reqwest "error decoding response body". Stream keeps the
//   connection alive and yields the full object on `response.completed`.
// - `max_tool_calls` / `parallel_tool_calls` are ignored by the API — only the
//   input prompt steers search depth. Use the bare user query as `input`.

/// Responses currently documents flash only; pro names map to flash for this path.
const DEEPSEEK_RESPONSES_WEB_MODEL: &str = "deepseek-v4-flash";
const DEEPSEEK_RESPONSES_WEB_TOOL: &str = "web_search";

pub(crate) async fn execute_deepseek_web(
    config: &SearchConfig,
    client: &Client,
    query: &str,
) -> anyhow::Result<SearchResponse> {
    let api_key = config.deepseek_api_key.trim();
    if api_key.is_empty() {
        anyhow::bail!("DeepSeek web search API key not configured (SEARCH_DEEPSEEK_API_KEY or AGENT_LLM_API_KEY)");
    }
    let endpoint = deepseek_responses_url(config);
    // Prefer configured model when it is flash; otherwise use documented Responses model.
    let model = {
        let m = config.deepseek_model.trim();
        if m.contains("flash") || m.is_empty() {
            if m.is_empty() {
                DEEPSEEK_RESPONSES_WEB_MODEL
            } else {
                m
            }
        } else {
            // pro / other → flash for Responses web_search support
            DEEPSEEK_RESPONSES_WEB_MODEL
        }
    };
    // Bare query only — heavy instruction text triggers server-side open_page spirals.
    let body = serde_json::json!({
        "model": model,
        "input": query.trim(),
        "tools": [{ "type": DEEPSEEK_RESPONSES_WEB_TOOL }],
        "tool_choice": "auto",
        "stream": true,
        "max_output_tokens": 2048,
        "reasoning": { "effort": "none" },
    });

    // Wall clock still needs headroom for multi-step server search; stream
    // events keep the TCP connection alive during tool rounds.
    let req_timeout_ms = config.timeout_ms.max(120_000);
    let response = client
        .post(&endpoint)
        .timeout(std::time::Duration::from_millis(req_timeout_ms))
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("deepseek responses web_search api error {status}: {body}");
    }

    let value = collect_deepseek_responses_sse(response).await?;
    // Auto-scrape (CRW) only fills thin snippets in SearchExecutor / stream path.
    Ok(search_response_from_deepseek_responses(value, query))
}

/// Consume DeepSeek / OpenAI-style Responses SSE until `response.completed`
/// (or incomplete/failed). Returns the nested `response` object for parsing.
async fn collect_deepseek_responses_sse(
    mut response: reqwest::Response,
) -> anyhow::Result<serde_json::Value> {
    // Prefer Response::chunk over bytes_stream so we don't need the reqwest
    // `stream` feature at the workspace level.
    let mut buf = String::new();
    let mut completed: Option<serde_json::Value> = None;
    let mut failed_payload: Option<String> = None;

    loop {
        let chunk = response.chunk().await.map_err(|e| {
            anyhow::anyhow!("deepseek responses SSE read failed: {e}")
        })?;
        let Some(chunk) = chunk else {
            break;
        };
        buf.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(idx) = buf.find("\n\n") {
            let block = buf[..idx].to_string();
            let rest = buf[idx + 2..].to_string();
            buf = rest;

            match fold_deepseek_sse_block(&block) {
                SseFold::Ignore => {}
                SseFold::Done => {
                    // Provider sentinel without a prior completed event.
                }
                SseFold::Completed(v) => {
                    completed = Some(v);
                }
                SseFold::Failed(msg) => {
                    failed_payload = Some(msg);
                }
            }
        }

        if completed.is_some() || failed_payload.is_some() {
            break;
        }
    }

    if let Some(v) = completed {
        return Ok(v);
    }
    if let Some(msg) = failed_payload {
        anyhow::bail!("deepseek responses stream failed: {msg}");
    }
    anyhow::bail!("deepseek responses stream ended without response.completed")
}

#[derive(Debug)]
enum SseFold {
    Ignore,
    Done,
    Completed(serde_json::Value),
    Failed(String),
}

/// Parse one SSE block (`event:` / `data:` lines). Pure for unit tests.
fn fold_deepseek_sse_block(block: &str) -> SseFold {
    let mut data_lines: Vec<&str> = Vec::new();
    for line in block.lines() {
        if let Some(rest) = line.strip_prefix("data:") {
            // SSE allows optional space after the colon.
            data_lines.push(rest.strip_prefix(' ').unwrap_or(rest));
        }
    }
    if data_lines.is_empty() {
        return SseFold::Ignore;
    }
    let raw = data_lines.join("\n");
    if raw.trim() == "[DONE]" {
        return SseFold::Done;
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return SseFold::Ignore;
    };
    match v.get("type").and_then(|t| t.as_str()) {
        Some("response.completed") | Some("response.incomplete") => {
            if let Some(resp) = v.get("response").cloned() {
                SseFold::Completed(resp)
            } else {
                SseFold::Completed(v)
            }
        }
        Some("response.failed") => SseFold::Failed(raw),
        _ => SseFold::Ignore,
    }
}

pub(crate) async fn stream_deepseek_web(
    config: &SearchConfig,
    client: &Client,
    query: &str,
    on_update: &mut impl FnMut(SearchStreamUpdate),
) -> anyhow::Result<SearchResponse> {
    on_update(SearchStreamUpdate::Searching {
        queries: vec![query.trim().to_string()],
    });
    let mut response = execute_deepseek_web(config, client, query).await?;
    // Skip CRW when Responses already filled rich snippets (min_snippet gate inside).
    crate::crw::auto_scrape_enrich_results(config, client, &mut response.results).await;
    on_update(SearchStreamUpdate::SourcesCollected {
        results: response.results.clone(),
    });
    Ok(response)
}

fn deepseek_responses_url(config: &SearchConfig) -> String {
    let mut base = config.deepseek_base_url.trim().trim_end_matches('/').to_string();
    // Accept mistaken anthropic suffix from shared AGENT_LLM_BASE_URL.
    if base.ends_with("/anthropic") {
        base.truncate(base.len() - "/anthropic".len());
        base = base.trim_end_matches('/').to_string();
    }
    format!("{base}/responses")
}

fn strip_ws_call_id_fragment(url: &str) -> String {
    // DeepSeek may append `#ws_call_id=…` for internal correlation.
    if let Some(idx) = url.find("#ws_call_id=") {
        url[..idx].to_string()
    } else {
        url.to_string()
    }
}

fn title_from_url(url: &str) -> String {
    url.trim_end_matches('/')
        .rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or(url)
        .replace('-', " ")
        .replace('_', " ")
}

/// Pull plaintext body if the API ever returns it on a node (forward-compatible).
fn extract_body_text(node: &serde_json::Value) -> String {
    for key in [
        "content",
        "text",
        "markdown",
        "snippet",
        "page_content",
        "body",
        "output_text",
    ] {
        if let Some(s) = node.get(key).and_then(|v| v.as_str()).map(str::trim) {
            if s.len() >= 40 {
                return s.to_string();
            }
        }
    }
    // Nested data.markdown / data.content
    if let Some(data) = node.get("data") {
        for key in ["markdown", "content", "text"] {
            if let Some(s) = data.get(key).and_then(|v| v.as_str()).map(str::trim) {
                if s.len() >= 40 {
                    return s.to_string();
                }
            }
        }
    }
    String::new()
}

fn push_result(
    results: &mut Vec<SearchResult>,
    seen: &mut HashSet<String>,
    url: &str,
    title: Option<&str>,
    snippet: &str,
) {
    let url = strip_ws_call_id_fragment(url.trim());
    if url.is_empty() || !url.starts_with("http") || !seen.insert(url.clone()) {
        return;
    }
    let title = title
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| title_from_url(&url));
    let citation_index = results.len() + 1;
    results.push(SearchResult {
        title,
        url,
        snippet: snippet.to_string(),
        citation_index: Some(citation_index),
    });
}

/// Map DeepSeek Responses API `response` object → SearchResponse.
///
/// Sources come from `output[]` items:
/// - `web_search_call` + `action.open_page` / `find_in_page` → URL
/// - `message` content `annotations` → URL
/// - any node with substantial content/text/markdown → snippet (if API provides body)
///
/// Probe note (2026-08-11): open_page often returns **URL only** (page body used
/// server-side for the model, not echoed). CRW auto-scrape still fills thin snippets.
pub(crate) fn search_response_from_deepseek_responses(
    value: serde_json::Value,
    original_query: &str,
) -> SearchResponse {
    let mut results = Vec::new();
    let mut seen = HashSet::new();
    let mut text_parts = Vec::new();
    let mut sub_queries = Vec::new();

    // Top-level convenience field some SDKs expose
    if let Some(ot) = value
        .get("output_text")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        text_parts.push(ot.to_string());
    }

    if let Some(items) = value.get("output").and_then(|c| c.as_array()) {
        for item in items {
            let ty = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match ty {
                "web_search_call" => {
                    let status = item.get("status").and_then(|s| s.as_str()).unwrap_or("");
                    // Still collect URLs from failed open_page (useful for CRW retry).
                    let action = item.get("action").cloned().unwrap_or(serde_json::Value::Null);
                    let action_ty = action.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    match action_ty {
                        "search" => {
                            if let Some(qs) = action.get("queries").and_then(|q| q.as_array()) {
                                for q in qs {
                                    if let Some(s) = q.as_str().map(str::trim).filter(|s| {
                                        !s.is_empty() && !s.starts_with("ws_call_id=")
                                    }) {
                                        if !sub_queries.iter().any(|x| x == s) {
                                            sub_queries.push(s.to_string());
                                        }
                                    }
                                }
                            }
                        }
                        "open_page" | "find_in_page" => {
                            if let Some(url) = action.get("url").and_then(|u| u.as_str()) {
                                let body = extract_body_text(item);
                                let body = if body.is_empty() {
                                    extract_body_text(&action)
                                } else {
                                    body
                                };
                                // Prefer completed pages when body exists; always keep URL.
                                let _ = status;
                                push_result(
                                    &mut results,
                                    &mut seen,
                                    url,
                                    action.get("title").and_then(|t| t.as_str()),
                                    &body,
                                );
                            }
                        }
                        _ => {
                            // Unknown action: still harvest URL fields.
                            if let Some(url) = action.get("url").and_then(|u| u.as_str()) {
                                let body = extract_body_text(item);
                                push_result(&mut results, &mut seen, url, None, &body);
                            }
                        }
                    }
                }
                "message" => {
                    if let Some(parts) = item.get("content").and_then(|c| c.as_array()) {
                        for part in parts {
                            if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                                let t = t.trim();
                                if !t.is_empty() {
                                    text_parts.push(t.to_string());
                                }
                            }
                            if let Some(anns) = part.get("annotations").and_then(|a| a.as_array()) {
                                for ann in anns {
                                    let url = ann
                                        .get("url")
                                        .or_else(|| ann.get("href"))
                                        .and_then(|u| u.as_str())
                                        .unwrap_or("");
                                    let title = ann.get("title").and_then(|t| t.as_str());
                                    let snip = extract_body_text(ann);
                                    push_result(&mut results, &mut seen, url, title, &snip);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Legacy Anthropic blocks (fallback if a gateway still returns messages shape)
    if results.is_empty() {
        if let Some(blocks) = value.get("content").and_then(|c| c.as_array()) {
            for block in blocks {
                if block.get("type").and_then(|t| t.as_str()) != Some("web_search_tool_result") {
                    continue;
                }
                if let Some(arr) = block.get("content").and_then(|v| v.as_array()) {
                    for item in arr {
                        if item.get("type").and_then(|t| t.as_str()) != Some("web_search_result")
                        {
                            continue;
                        }
                        let url = item.get("url").and_then(|u| u.as_str()).unwrap_or("");
                        let title = item.get("title").and_then(|t| t.as_str());
                        let snip = item
                            .get("snippet")
                            .and_then(|s| s.as_str())
                            .unwrap_or("")
                            .to_string();
                        push_result(&mut results, &mut seen, url, title, &snip);
                    }
                }
            }
        }
    }

    if sub_queries.is_empty() {
        sub_queries.push(original_query.trim().to_string());
    }

    let synthesized_answer = if !text_parts.is_empty() {
        text_parts.join("\n")
    } else if results.is_empty() {
        format!(
            "No DeepSeek web_search sources were found for: {}",
            original_query.trim()
        )
    } else {
        let source_lines = results
            .iter()
            .map(|result| {
                let index = result.citation_index.unwrap_or(0);
                if result.snippet.is_empty() {
                    format!("[[{index}]] {}", result.title)
                } else {
                    format!("[[{index}]] {}: {}", result.title, result.snippet)
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        format!(
            "DeepSeek web_search returned these sources for '{}':\n\n{}",
            original_query.trim(),
            source_lines
        )
    };

    SearchResponse {
        query_type: "deepseek_web".to_string(),
        sub_queries,
        results,
        synthesized_answer,
        llm_usage: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BraveLlmContextRequest, BraveLlmContextResponse, BraveNewsItem, BraveNewsResponse,
        SseFold, fold_deepseek_sse_block, search_response_from_brave_context,
        search_response_from_brave_news, search_response_from_deepseek_responses,
    };

    #[test]
    fn parses_brave_llm_context_grounding_into_sources() {
        let response: BraveLlmContextResponse = serde_json::from_value(serde_json::json!({
            "grounding": {
                "generic": [
                    {
                        "url": "https://example.com/atlas",
                        "title": "Atlas Checklist",
                        "snippets": ["Atlas uses the rollback checklist.", "Incident timeline details."]
                    },
                    {
                        "url": "https://example.com/atlas",
                        "title": "Duplicate",
                        "snippets": ["duplicate should be ignored"]
                    }
                ],
                "map": []
            },
            "sources": {
                "https://example.com/atlas": {
                    "title": "Atlas Checklist",
                    "hostname": "example.com"
                }
            }
        }))
        .unwrap();

        let search_response = search_response_from_brave_context(response, "atlas rollback");

        assert_eq!(search_response.query_type, "brave_llm_context");
        assert_eq!(
            search_response.sub_queries,
            vec!["atlas rollback".to_string()]
        );
        assert_eq!(search_response.results.len(), 1);
        assert_eq!(search_response.results[0].citation_index, Some(1));
        assert_eq!(search_response.results[0].url, "https://example.com/atlas");
        assert!(
            search_response.results[0]
                .snippet
                .contains("rollback checklist")
        );
    }

    #[test]
    fn brave_llm_context_request_omits_optional_params_when_none() {
        let config = crate::SearchConfig::default();
        let req = BraveLlmContextRequest::new("test query", &config);
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["q"], "test query");
        assert!(!json.as_object().unwrap().contains_key("search_lang"));
        assert!(!json.as_object().unwrap().contains_key("country"));
        assert!(!json.as_object().unwrap().contains_key("freshness"));
    }

    #[test]
    fn brave_llm_context_request_includes_search_lang_country_freshness() {
        let config = crate::SearchConfig {
            search_lang: Some("zh".to_string()),
            country: Some("CN".to_string()),
            freshness: Some("pd".to_string()),
            ..crate::SearchConfig::default()
        };
        let req = BraveLlmContextRequest::new("test query", &config);
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["search_lang"], "zh");
        assert_eq!(json["country"], "CN");
        assert_eq!(json["freshness"], "pd");
    }

    #[test]
    fn brave_news_response_maps_to_search_results() {
        let response = BraveNewsResponse {
            results: vec![
                BraveNewsItem {
                    title: "News Title 1".to_string(),
                    url: "https://example.com/1".to_string(),
                    description: Some("Description one".to_string()),
                    age: Some("2 hours ago".to_string()),
                    meta_url: None,
                },
                BraveNewsItem {
                    title: "News Title 2".to_string(),
                    url: "https://example.com/2".to_string(),
                    description: None,
                    age: Some("1 day ago".to_string()),
                    meta_url: None,
                },
                BraveNewsItem {
                    title: "Duplicate".to_string(),
                    url: "https://example.com/1".to_string(),
                    description: Some("Should be deduped".to_string()),
                    age: None,
                    meta_url: None,
                },
            ],
        };

        let search_response = search_response_from_brave_news(response, "test query");

        assert_eq!(search_response.query_type, "brave_news");
        assert_eq!(search_response.results.len(), 2);
        assert_eq!(search_response.results[0].title, "News Title 1");
        assert_eq!(search_response.results[0].snippet, "Description one");
        assert_eq!(search_response.results[0].citation_index, Some(1));
        assert_eq!(search_response.results[1].title, "News Title 2");
        assert_eq!(
            search_response.results[1].snippet,
            "News article (1 day ago)"
        );
        assert_eq!(search_response.results[1].citation_index, Some(2));
        assert!(
            search_response.synthesized_answer.contains("News Title 1"),
            "synthesized_answer should mention first title"
        );
    }

    #[test]
    fn brave_news_empty_results_fallback() {
        let response = BraveNewsResponse { results: vec![] };
        let search_response = search_response_from_brave_news(response, "obscure query");
        assert!(search_response.results.is_empty());
        assert!(
            search_response
                .synthesized_answer
                .contains("No Brave News sources"),
            "should return empty fallback message"
        );
    }

    #[test]
    fn parses_deepseek_responses_open_page_urls() {
        let value = serde_json::json!({
            "status": "completed",
            "output": [
                {
                    "type": "web_search_call",
                    "status": "completed",
                    "action": {
                        "type": "search",
                        "queries": ["capital of France river", "ws_call_id=x"]
                    }
                },
                {
                    "type": "web_search_call",
                    "status": "completed",
                    "action": {
                        "type": "open_page",
                        "url": "https://www.britannica.com/place/Seine-River#ws_call_id=call_01"
                    }
                },
                {
                    "type": "web_search_call",
                    "status": "completed",
                    "action": {
                        "type": "open_page",
                        "url": "https://en.wikipedia.org/wiki/Paris#ws_call_id=call_02"
                    }
                },
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": "Paris is the capital; the Seine runs through it.",
                        "annotations": [{
                            "type": "url_citation",
                            "url": "https://en.wikipedia.org/wiki/Paris",
                            "title": "Paris - Wikipedia"
                        }]
                    }]
                }
            ]
        });
        let resp = search_response_from_deepseek_responses(value, "capital of France");
        assert_eq!(resp.query_type, "deepseek_web");
        assert!(resp.sub_queries.iter().any(|q| q.contains("capital of France river")));
        // dedupe wiki URL from open_page + annotation
        assert_eq!(resp.results.len(), 2);
        assert!(resp.results.iter().any(|r| r.url.contains("britannica.com")));
        assert!(resp.results.iter().any(|r| r.url.contains("wikipedia.org/wiki/Paris")));
        assert!(!resp.results[0].url.contains("ws_call_id"));
        assert!(resp.synthesized_answer.contains("Seine") || resp.synthesized_answer.contains("Paris"));
    }

    #[test]
    fn responses_body_skips_crw_when_snippet_rich() {
        let value = serde_json::json!({
            "output": [{
                "type": "web_search_call",
                "status": "completed",
                "action": {
                    "type": "open_page",
                    "url": "https://example.com/page",
                    "markdown": "x".repeat(200)
                }
            }]
        });
        // body may be under action or item — extract_body_text checks both via push_result path
        let mut value = value;
        value["output"][0]["markdown"] = serde_json::json!("Y".repeat(120));
        let resp = search_response_from_deepseek_responses(value, "q");
        assert_eq!(resp.results.len(), 1);
        assert!(resp.results[0].snippet.chars().count() >= 80);
    }

    #[test]
    fn deepseek_sse_fold_extracts_nested_response_on_completed() {
        let block = concat!(
            "event: response.completed\n",
            r#"data: {"type":"response.completed","response":{"status":"completed","output":[{"type":"web_search_call","action":{"type":"open_page","url":"https://example.com/a"}}]}}"#,
            "\n",
        );
        match fold_deepseek_sse_block(block) {
            SseFold::Completed(v) => {
                assert_eq!(v["status"], "completed");
                assert!(v["output"].is_array());
                let resp = search_response_from_deepseek_responses(v, "q");
                assert_eq!(resp.results.len(), 1);
                assert!(resp.results[0].url.contains("example.com/a"));
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[test]
    fn deepseek_sse_fold_ignores_progress_and_handles_done() {
        assert!(matches!(
            fold_deepseek_sse_block("event: response.created\ndata: {\"type\":\"response.created\"}\n"),
            SseFold::Ignore
        ));
        assert!(matches!(
            fold_deepseek_sse_block("data: [DONE]\n"),
            SseFold::Done
        ));
        assert!(matches!(
            fold_deepseek_sse_block(
                "data: {\"type\":\"response.failed\",\"error\":{\"message\":\"boom\"}}\n"
            ),
            SseFold::Failed(_)
        ));
    }
}
