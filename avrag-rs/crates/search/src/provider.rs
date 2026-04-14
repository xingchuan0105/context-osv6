use std::collections::HashSet;

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{SearchConfig, SearchResponse, SearchResult};

pub(crate) async fn execute_perplexity_agent(
    config: &SearchConfig,
    client: &Client,
    query: &str,
) -> anyhow::Result<SearchResponse> {
    let api_key = config
        .perplexity_api_key
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Perplexity API key not configured"))?;

    let request_body = PerplexityAgentRequest {
        model: config.perplexity_model.clone(),
        input: vec![PerplexityInput {
            role: "user".to_string(),
            content: query.to_string(),
        }],
        mode: "pro-search".to_string(),
    };

    let response = client
        .post("https://api.perplexity.ai/v1/agent")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("perplexity agent api error {}: {}", status, body);
    }

    let perplexity_response: PerplexityAgentResponse = response.json().await?;
    let synthesized_answer = perplexity_response
        .output
        .as_ref()
        .map(|output| output.text.clone())
        .unwrap_or_else(|| "No response from Perplexity.".to_string());

    let citations = perplexity_response.citations.unwrap_or_default();
    let mut results = Vec::new();
    let mut seen_urls = HashSet::new();
    let re = regex::Regex::new(r"\[(\d+)\]").unwrap();

    for caps in re.captures_iter(&synthesized_answer) {
        if let Ok(index) = caps.get(1).unwrap().as_str().parse::<usize>() {
            if index > 0 && index <= citations.len() {
                let url = citations[index - 1].clone();
                if seen_urls.insert(url.clone()) {
                    results.push(SearchResult {
                        title: url.clone(),
                        url,
                        snippet: format!("Source [{}]", index),
                        citation_index: Some(index),
                    });
                }
            }
        }
    }

    if results.is_empty() {
        results.push(SearchResult {
            title: "Perplexity Agent Response".to_string(),
            url: "https://perplexity.ai".to_string(),
            snippet: synthesized_answer.chars().take(200).collect(),
            citation_index: None,
        });
    }

    Ok(SearchResponse {
        query_type: "agentic".to_string(),
        sub_queries: vec![query.to_string()],
        results,
        synthesized_answer,
        planner_usage: None,
    })
}

pub(crate) async fn search_by_provider(
    config: &SearchConfig,
    client: &Client,
    query: &str,
) -> anyhow::Result<Vec<SearchResult>> {
    match config.provider.trim().to_lowercase().as_str() {
        "exa" => call_exa_api(config, client, query).await,
        "perplexity" => call_perplexity_agent_api(config, client, query).await,
        "mock" => call_mock_api(query).await,
        provider => anyhow::bail!("unsupported search provider: {provider}"),
    }
}

async fn call_mock_api(query: &str) -> anyhow::Result<Vec<SearchResult>> {
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let mock_response = format!(
        "This is a mock search result for '{}'. \
         It contains information about the query but is not from a real external source. \
         The Perplexity Agent API is currently unreachable due to network restrictions in this environment.",
        query
    );

    Ok(vec![
        SearchResult {
            title: format!("Mock Result for: {}", query),
            url: "https://example.com/mock-result".to_string(),
            snippet: mock_response,
            citation_index: None,
        },
        SearchResult {
            title: "Related Mock Article".to_string(),
            url: "https://example.com/related".to_string(),
            snippet: format!("Another mock snippet related to '{}'.", query),
            citation_index: None,
        },
    ])
}

async fn call_perplexity_agent_api(
    config: &SearchConfig,
    client: &Client,
    query: &str,
) -> anyhow::Result<Vec<SearchResult>> {
    let api_key = config
        .perplexity_api_key
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Perplexity API key not configured"))?;

    let request_body = PerplexityAgentRequest {
        model: config.perplexity_model.clone(),
        input: vec![PerplexityInput {
            role: "user".to_string(),
            content: query.to_string(),
        }],
        mode: "pro-search".to_string(),
    };

    let response = client
        .post("https://api.perplexity.ai/v1/agent")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("perplexity agent api error {}: {}", status, body);
    }

    let perplexity_response: PerplexityAgentResponse = response.json().await?;

    let results = perplexity_response
        .output
        .map(|output| SearchResult {
            title: "Perplexity Agent Response".to_string(),
            url: "https://perplexity.ai".to_string(),
            snippet: output.text.chars().take(500).collect(),
            citation_index: None,
        })
        .map(|result| vec![result])
        .unwrap_or_default();

    Ok(results)
}

async fn call_exa_api(config: &SearchConfig, client: &Client, query: &str) -> anyhow::Result<Vec<SearchResult>> {
    let search_request = ExaSearchRequest {
        query: query.to_string(),
        num_results: config.max_results,
        search_type: if config.extract_enabled {
            Some("auto".to_string())
        } else {
            None
        },
    };

    let response = client
        .post(format!(
            "{}/search",
            config.base_url.trim_end_matches('/')
        ))
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&search_request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("search provider error {}: {}", status, body);
    }

    #[derive(serde::Deserialize)]
    struct ExaSearchResponse {
        results: Vec<ExaResult>,
    }

    #[derive(serde::Deserialize)]
    struct ExaResult {
        title: Option<String>,
        url: String,
        snippet: Option<String>,
        text: Option<String>,
    }

    let exa_resp: ExaSearchResponse = response.json().await?;
    Ok(exa_resp
        .results
        .into_iter()
        .map(|result| SearchResult {
            title: result.title.unwrap_or_else(|| "Untitled".to_string()),
            url: result.url,
            snippet: result
                .snippet
                .or(result.text)
                .unwrap_or_default()
                .chars()
                .take(400)
                .collect(),
            citation_index: None,
        })
        .collect())
}

#[derive(serde::Serialize)]
struct ExaSearchRequest {
    query: String,
    #[serde(rename = "numResults")]
    num_results: usize,
    #[serde(rename = "type")]
    search_type: Option<String>,
}

#[derive(Serialize)]
struct PerplexityAgentRequest {
    model: String,
    input: Vec<PerplexityInput>,
    mode: String,
}

#[derive(Serialize)]
struct PerplexityInput {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct PerplexityAgentResponse {
    output: Option<PerplexityAgentOutput>,
    /// 引用 URL 数组，对应文本中的 [1], [2] 角标
    #[serde(default)]
    citations: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct PerplexityAgentOutput {
    text: String,
}
