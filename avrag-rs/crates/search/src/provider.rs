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
    let response = execute_brave_llm_context(config, client, query).await?;
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
        .query(&[("q", query), ("count", &config.max_results.clamp(1, 50).to_string())]);

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























#[cfg(test)]
mod tests {
    use super::{
        BraveLlmContextRequest, BraveLlmContextResponse, BraveNewsItem, BraveNewsResponse,
                search_response_from_brave_context,
        search_response_from_brave_news,
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
        assert_eq!(search_response.results[1].snippet, "News article (1 day ago)");
        assert_eq!(search_response.results[1].citation_index, Some(2));
        assert!(
            search_response
                .synthesized_answer
                .contains("News Title 1"),
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
}
