use std::collections::{HashMap, HashSet};

use avrag_llm::LlmUsage;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{SearchConfig, SearchResponse, SearchResult, SearchStreamUpdate};

const PERPLEXITY_AGENT_URL: &str = "https://api.perplexity.ai/v1/agent";
const PERPLEXITY_SEARCH_PRESET: &str = "pro-search";
const BRAVE_LLM_CONTEXT_PATH: &str = "/res/v1/llm/context";

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
        .header("Accept-Encoding", "gzip")
        .json(&BraveLlmContextRequest::new(query, config.max_results))
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

pub(crate) async fn execute_perplexity_agent(
    config: &SearchConfig,
    client: &Client,
    query: &str,
) -> anyhow::Result<SearchResponse> {
    let api_key = configured_perplexity_api_key(config)?;
    let response = client
        .post(PERPLEXITY_AGENT_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&PerplexityAgentRequest::new(query, false))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("perplexity agent api error {}: {}", status, body);
    }

    let agent_response: PerplexityAgentResponse = response.json().await?;
    Ok(search_response_from_agent_response(agent_response, query))
}

pub(crate) async fn stream_perplexity_agent(
    config: &SearchConfig,
    client: &Client,
    query: &str,
    on_update: &mut impl FnMut(SearchStreamUpdate),
) -> anyhow::Result<SearchResponse> {
    let api_key = configured_perplexity_api_key(config)?;
    let mut response = client
        .post(PERPLEXITY_AGENT_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&PerplexityAgentRequest::new(query, true))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("perplexity agent api error {}: {}", status, body);
    }

    let mut parser = PerplexitySseParser::default();
    let mut completed = None;

    while let Some(chunk) = response.chunk().await? {
        parser.feed_chunk(&chunk, &mut |event_name, data| {
            if let Some(update) = stream_update_from_event(event_name, data)? {
                on_update(update);
            }
            if event_name == "response.completed" {
                completed = Some(search_response_from_completed_event(data, query)?);
            }
            Ok(())
        })?;
    }

    parser.finish(&mut |event_name, data| {
        if let Some(update) = stream_update_from_event(event_name, data)? {
            on_update(update);
        }
        if event_name == "response.completed" {
            completed = Some(search_response_from_completed_event(data, query)?);
        }
        Ok(())
    })?;

    completed.ok_or_else(|| anyhow::anyhow!("perplexity stream ended without response.completed"))
}

fn configured_brave_api_key(config: &SearchConfig) -> anyhow::Result<&str> {
    let api_key = config.api_key.trim();
    if api_key.is_empty() {
        anyhow::bail!("Brave LLM Context API key not configured");
    }
    Ok(api_key)
}

fn configured_perplexity_api_key(config: &SearchConfig) -> anyhow::Result<&str> {
    config
        .perplexity_api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Perplexity API key not configured"))
}

fn brave_llm_context_url(config: &SearchConfig) -> String {
    let base = config.base_url.trim().trim_end_matches('/');
    if base.ends_with(BRAVE_LLM_CONTEXT_PATH) {
        base.to_string()
    } else {
        format!("{base}{BRAVE_LLM_CONTEXT_PATH}")
    }
}

#[derive(Debug, Serialize)]
struct BraveLlmContextRequest<'a> {
    q: &'a str,
    count: usize,
    maximum_number_of_urls: usize,
}

impl<'a> BraveLlmContextRequest<'a> {
    fn new(query: &'a str, max_results: usize) -> Self {
        let count = max_results.clamp(1, 50);
        Self {
            q: query,
            count,
            maximum_number_of_urls: count,
        }
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

fn stream_update_from_event(
    event_name: &str,
    data: &str,
) -> anyhow::Result<Option<SearchStreamUpdate>> {
    match event_name {
        "response.reasoning.search_queries" => {
            let event: SearchQueriesEvent = serde_json::from_str(data)?;
            if event.queries.is_empty() {
                return Ok(None);
            }
            Ok(Some(SearchStreamUpdate::Searching {
                queries: event.queries,
            }))
        }
        "response.output_item.done" => {
            let event: OutputItemDoneEvent = serde_json::from_str(data)?;
            if event.item.item_type.as_deref() != Some("search_results") {
                return Ok(None);
            }
            let results = parse_results_from_value(event.item.value)?;
            if results.is_empty() {
                return Ok(None);
            }
            Ok(Some(SearchStreamUpdate::SourcesCollected { results }))
        }
        "response.output_text.delta" => {
            let event: OutputTextDeltaEvent = serde_json::from_str(data)?;
            if event.delta.is_empty() {
                return Ok(None);
            }
            Ok(Some(SearchStreamUpdate::TextDelta { delta: event.delta }))
        }
        _ => Ok(None),
    }
}

fn search_response_from_completed_event(data: &str, query: &str) -> anyhow::Result<SearchResponse> {
    let event: CompletedEvent = serde_json::from_str(data)?;
    Ok(search_response_from_agent_response(event.response, query))
}

fn search_response_from_agent_response(
    response: PerplexityAgentResponse,
    original_query: &str,
) -> SearchResponse {
    let raw_synthesized_answer = response
        .output_text()
        .unwrap_or_else(|| "No response from Perplexity.".to_string());
    let sub_queries = {
        let queries = response.search_queries();
        if queries.is_empty() {
            vec![original_query.trim().to_string()]
        } else {
            queries
        }
    };

    let mut results = response.search_results();
    if results.is_empty() {
        let citation_urls = response.citation_urls();
        results = citation_results_from_markers(&raw_synthesized_answer, &citation_urls);
        if results.is_empty() {
            results = citation_results_from_urls(&citation_urls);
        }
    }

    let synthesized_answer = strip_citation_markers(&raw_synthesized_answer, results.len());

    if results.is_empty() {
        results.push(SearchResult {
            title: "Perplexity Agent Response".to_string(),
            url: "https://perplexity.ai".to_string(),
            snippet: synthesized_answer.chars().take(200).collect(),
            citation_index: None,
        });
    }

    SearchResponse {
        query_type: "agentic".to_string(),
        sub_queries,
        results,
        synthesized_answer,
        llm_usage: response.llm_usage(),
    }
}

fn strip_citation_markers(answer: &str, max_citation_index: usize) -> String {
    let re = regex::Regex::new(r"\[(?:web:|citation:)?\s*(\d+)\]").unwrap();
    let whitespace_re = regex::Regex::new(r"[ \t]{2,}").unwrap();
    let stripped = re.replace_all(answer, |caps: &regex::Captures<'_>| {
        let marker = caps.get(0).unwrap();
        let previous_is_bracket =
            marker.start() > 0 && answer.as_bytes().get(marker.start() - 1) == Some(&b'[');
        let next_is_bracket = answer.as_bytes().get(marker.end()) == Some(&b']');
        if previous_is_bracket || next_is_bracket {
            return marker.as_str().to_string();
        }

        let index = caps
            .get(1)
            .and_then(|value| value.as_str().parse::<usize>().ok())
            .unwrap_or(0);
        if index > 0 && index <= max_citation_index {
            format!("[[{index}]]")
        } else {
            marker.as_str().to_string()
        }
    });
    stripped
        .lines()
        .map(|line| {
            let line = line.trim_end();
            let body_start = line
                .char_indices()
                .find(|(_, character)| *character != ' ' && *character != '\t')
                .map(|(index, _)| index)
                .unwrap_or(line.len());
            let (leading, body) = line.split_at(body_start);

            format!("{}{}", leading, whitespace_re.replace_all(body, " "))
                .replace(" .", ".")
                .replace(" ,", ",")
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn parse_results_from_value(value: Value) -> anyhow::Result<Vec<SearchResult>> {
    let item: SearchResultsItem = serde_json::from_value(value)?;
    Ok(item.results())
}

fn citation_results_from_markers(answer: &str, citation_urls: &[String]) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let mut seen_urls = HashSet::new();
    let re = regex::Regex::new(r"\[(\d+)\]").unwrap();

    for caps in re.captures_iter(answer) {
        let Ok(index) = caps.get(1).unwrap().as_str().parse::<usize>() else {
            continue;
        };
        if index == 0 || index > citation_urls.len() {
            continue;
        }
        let url = citation_urls[index - 1].clone();
        if seen_urls.insert(url.clone()) {
            results.push(SearchResult {
                title: url.clone(),
                url,
                snippet: format!("Source [{}]", index),
                citation_index: Some(index),
            });
        }
    }

    results
}

fn citation_results_from_urls(citation_urls: &[String]) -> Vec<SearchResult> {
    citation_urls
        .iter()
        .enumerate()
        .map(|(index, url)| SearchResult {
            title: url.clone(),
            url: url.clone(),
            snippet: format!("Source [{}]", index + 1),
            citation_index: Some(index + 1),
        })
        .collect()
}

#[derive(Serialize)]
struct PerplexityAgentRequest {
    preset: &'static str,
    input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

impl PerplexityAgentRequest {
    fn new(query: &str, stream: bool) -> Self {
        Self {
            preset: PERPLEXITY_SEARCH_PRESET,
            input: query.to_string(),
            stream: stream.then_some(true),
        }
    }
}

#[derive(Debug, Deserialize)]
struct PerplexityAgentResponse {
    #[serde(default)]
    output: Vec<Value>,
    #[serde(default)]
    citations: Option<Vec<String>>,
    #[serde(default)]
    usage: Option<PerplexityUsage>,
    #[serde(default)]
    model: Option<String>,
}

impl PerplexityAgentResponse {
    fn output_text(&self) -> Option<String> {
        let text = self
            .output
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some("message"))
            .filter_map(|item| item.get("content").and_then(Value::as_array))
            .flat_map(|content| content.iter())
            .filter_map(|chunk| chunk.get("text").and_then(Value::as_str))
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        let trimmed = text.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    }

    fn search_queries(&self) -> Vec<String> {
        self.output
            .iter()
            .find(|item| item.get("type").and_then(Value::as_str) == Some("search_results"))
            .and_then(|item| item.get("queries").and_then(Value::as_array))
            .map(|queries| {
                queries
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|query| !query.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    fn search_results(&self) -> Vec<SearchResult> {
        self.output
            .iter()
            .find(|item| item.get("type").and_then(Value::as_str) == Some("search_results"))
            .cloned()
            .and_then(|item| parse_results_from_value(item).ok())
            .unwrap_or_default()
    }

    fn citation_urls(&self) -> Vec<String> {
        let mut urls = Vec::new();
        let mut seen = HashSet::new();

        if let Some(citations) = self.citations.as_ref() {
            for url in citations {
                let trimmed = url.trim();
                if !trimmed.is_empty() && seen.insert(trimmed.to_string()) {
                    urls.push(trimmed.to_string());
                }
            }
        }

        for item in &self.output {
            let Some(content) = item.get("content").and_then(Value::as_array) else {
                continue;
            };
            for chunk in content {
                let Some(annotations) = chunk.get("annotations").and_then(Value::as_array) else {
                    continue;
                };
                for annotation in annotations {
                    let Some(url) = annotation.get("url").and_then(Value::as_str) else {
                        continue;
                    };
                    let trimmed = url.trim();
                    if !trimmed.is_empty() && seen.insert(trimmed.to_string()) {
                        urls.push(trimmed.to_string());
                    }
                }
            }
        }

        urls
    }

    fn llm_usage(&self) -> Option<LlmUsage> {
        self.usage.as_ref().map(|usage| LlmUsage {
            prompt_tokens: usage.input_tokens,
            completion_tokens: usage.output_tokens,
            total_tokens: usage.total_tokens,
            provider: "perplexity".to_string(),
            model: self
                .model
                .clone()
                .unwrap_or_else(|| PERPLEXITY_SEARCH_PRESET.to_string()),
        })
    }
}

#[derive(Debug, Deserialize)]
struct PerplexityUsage {
    input_tokens: u32,
    output_tokens: u32,
    total_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct SearchQueriesEvent {
    #[serde(default)]
    queries: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OutputTextDeltaEvent {
    delta: String,
}

#[derive(Debug, Deserialize)]
struct OutputItemDoneEvent {
    item: OutputItemValue,
}

#[derive(Debug, Deserialize)]
struct OutputItemValue {
    #[serde(flatten)]
    value: Value,
    #[serde(rename = "type")]
    item_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CompletedEvent {
    response: PerplexityAgentResponse,
}

#[derive(Debug, Deserialize)]
struct SearchResultsItem {
    #[serde(default)]
    results: Vec<PerplexitySearchResult>,
}

impl SearchResultsItem {
    fn results(self) -> Vec<SearchResult> {
        self.results
            .into_iter()
            .enumerate()
            .map(|(index, result)| SearchResult {
                title: result.title.unwrap_or_else(|| "Untitled".to_string()),
                url: result.url,
                snippet: result.snippet.unwrap_or_default(),
                citation_index: result.id.or(Some(index + 1)),
            })
            .collect()
    }
}

#[derive(Debug, Deserialize)]
struct PerplexitySearchResult {
    #[serde(default)]
    id: Option<usize>,
    url: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    snippet: Option<String>,
}

#[derive(Debug, Default)]
struct PerplexitySseParser {
    buffer: Vec<u8>,
    event_name: String,
    data_lines: Vec<String>,
}

impl PerplexitySseParser {
    fn feed_chunk(
        &mut self,
        chunk: &[u8],
        on_event: &mut impl FnMut(&str, &str) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        self.buffer.extend_from_slice(chunk);

        while let Some(newline_index) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let mut line = self.buffer.drain(..=newline_index).collect::<Vec<_>>();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let line = std::str::from_utf8(&line)?;
            self.push_line(&line, on_event)?;
        }

        Ok(())
    }

    fn finish(
        &mut self,
        on_event: &mut impl FnMut(&str, &str) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        if !self.buffer.is_empty() {
            let mut line = std::mem::take(&mut self.buffer);
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let line = std::str::from_utf8(&line)?;
            self.push_line(line, on_event)?;
        }
        self.flush(on_event)
    }

    fn push_line(
        &mut self,
        line: &str,
        on_event: &mut impl FnMut(&str, &str) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        if line.is_empty() {
            return self.flush(on_event);
        }
        if line.starts_with(':') {
            return Ok(());
        }

        let separator_index = line.find(':');
        let field = separator_index.map(|index| &line[..index]).unwrap_or(line);
        let mut value = separator_index
            .map(|index| &line[index + 1..])
            .unwrap_or_default();
        if value.starts_with(' ') {
            value = &value[1..];
        }

        match field {
            "event" => self.event_name = value.to_string(),
            "data" => self.data_lines.push(value.to_string()),
            _ => {}
        }

        Ok(())
    }

    fn flush(
        &mut self,
        on_event: &mut impl FnMut(&str, &str) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        if self.event_name.is_empty() && self.data_lines.is_empty() {
            return Ok(());
        }

        let event_name = std::mem::take(&mut self.event_name);
        let data = self.data_lines.join("\n");
        self.data_lines.clear();

        if event_name.is_empty() || data.trim().is_empty() {
            return Ok(());
        }

        on_event(&event_name, &data)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BraveLlmContextResponse, CompletedEvent, PerplexityAgentResponse, PerplexitySseParser,
        SearchResultsItem, search_response_from_agent_response, search_response_from_brave_context,
    };

    #[test]
    fn extracts_search_results_and_usage_from_completed_response() {
        let response: CompletedEvent = serde_json::from_value(serde_json::json!({
            "response": {
                "model": "openai/gpt-5.1",
                "usage": {
                    "input_tokens": 12,
                    "output_tokens": 34,
                    "total_tokens": 46
                },
                "output": [
                    {
                        "type": "search_results",
                        "queries": ["latest ai developments"],
                        "results": [
                            {
                                "id": 1,
                                "title": "AI News",
                                "url": "https://example.com/ai",
                                "snippet": "Latest AI news"
                            }
                        ]
                    },
                    {
                        "type": "message",
                        "content": [
                            {
                                "type": "output_text",
                                "text": "AI is moving fast."
                            }
                        ]
                    }
                ]
            }
        }))
        .unwrap();

        let search_response =
            search_response_from_agent_response(response.response, "latest ai developments");

        assert_eq!(search_response.query_type, "agentic");
        assert_eq!(
            search_response.sub_queries,
            vec!["latest ai developments".to_string()]
        );
        assert_eq!(search_response.results.len(), 1);
        assert_eq!(search_response.results[0].title, "AI News");
        assert_eq!(search_response.synthesized_answer, "AI is moving fast.");
        assert_eq!(
            search_response
                .llm_usage
                .as_ref()
                .map(|usage| usage.total_tokens),
            Some(46)
        );
    }

    #[test]
    fn parses_search_results_item_shape() {
        let item: SearchResultsItem = serde_json::from_value(serde_json::json!({
            "type": "search_results",
            "results": [
                {
                    "id": 3,
                    "title": "Example",
                    "url": "https://example.com",
                    "snippet": "Snippet"
                }
            ]
        }))
        .unwrap();

        let results = item.results();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].citation_index, Some(3));
        assert_eq!(results[0].url, "https://example.com");
    }

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
    fn sse_parser_handles_utf8_split_across_chunks() {
        let mut parser = PerplexitySseParser::default();
        let mut events = Vec::new();
        let bytes = "event: response.output_text.delta\ndata: {\"delta\":\"上海\"}\n\n".as_bytes();
        let split_index = bytes
            .windows("上".len())
            .position(|window| window == "上".as_bytes())
            .unwrap()
            + 1;

        parser
            .feed_chunk(&bytes[..split_index], &mut |event_name, data| {
                events.push((event_name.to_string(), data.to_string()));
                Ok(())
            })
            .unwrap();
        parser
            .feed_chunk(&bytes[split_index..], &mut |event_name, data| {
                events.push((event_name.to_string(), data.to_string()));
                Ok(())
            })
            .unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, "response.output_text.delta");
        assert_eq!(events[0].1, "{\"delta\":\"上海\"}");
    }

    #[test]
    fn preserves_legacy_output_object_citations_as_fallback() {
        let response: PerplexityAgentResponse = serde_json::from_value(serde_json::json!({
            "output": [
                {
                    "type": "message",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Legacy output [1]",
                            "annotations": [
                                {
                                    "type": "url_citation",
                                    "url": "https://example.com/legacy"
                                }
                            ]
                        }
                    ]
                }
            ]
        }))
        .unwrap();

        let search_response = search_response_from_agent_response(response, "legacy query");
        assert_eq!(search_response.results.len(), 1);
        assert_eq!(search_response.results[0].url, "https://example.com/legacy");
        assert_eq!(search_response.synthesized_answer, "Legacy output [[1]]");
    }

    mod strip_citation_markers {
        use super::super::strip_citation_markers;

        #[test]
        fn removes_bare_numeric_markers() {
            assert_eq!(
                strip_citation_markers("Answer [1] and [2] end.", 3),
                "Answer [[1]] and [[2]] end."
            );
        }

        #[test]
        fn removes_web_prefix_markers() {
            assert_eq!(
                strip_citation_markers("See [web:1] and [web:2] for details.", 3),
                "See [[1]] and [[2]] for details."
            );
        }

        #[test]
        fn removes_citation_prefix_markers() {
            assert_eq!(
                strip_citation_markers("Ref [citation:1] and [citation:3].", 3),
                "Ref [[1]] and [[3]]."
            );
        }

        #[test]
        fn preserves_markers_outside_valid_range() {
            assert_eq!(
                strip_citation_markers("Valid [1] but [5] is out of range.", 3),
                "Valid [[1]] but [5] is out of range."
            );
        }

        #[test]
        fn preserves_markers_with_index_zero() {
            assert_eq!(
                strip_citation_markers("Zero marker [0] should stay.", 3),
                "Zero marker [0] should stay."
            );
        }

        #[test]
        fn handles_multiline_answer() {
            assert_eq!(
                strip_citation_markers("Line 1 [1]\nLine 2 [2]\nLine 3", 3),
                "Line 1 [[1]]\nLine 2 [[2]]\nLine 3"
            );
        }

        #[test]
        fn cleans_double_space_artifacts() {
            assert_eq!(
                strip_citation_markers("End of sentence  [1].", 3),
                "End of sentence [[1]]."
            );
        }

        #[test]
        fn handles_markers_adjacent_to_text() {
            assert_eq!(
                strip_citation_markers("text[1]more[2]text", 3),
                "text[[1]]more[[2]]text"
            );
        }

        #[test]
        fn empty_answer_unchanged() {
            assert_eq!(strip_citation_markers("", 3), "");
        }

        #[test]
        fn no_markers_unchanged() {
            assert_eq!(
                strip_citation_markers("Plain answer with no citations.", 3),
                "Plain answer with no citations."
            );
        }

        #[test]
        fn whitespace_inside_brackets_handled() {
            assert_eq!(
                strip_citation_markers("See [web: 1] and [citation: 2].", 3),
                "See [[1]] and [[2]]."
            );
        }

        #[test]
        fn max_citation_index_zero_preserves_all_markers() {
            assert_eq!(
                strip_citation_markers("Markers [1] [2] [3] all stay.", 0),
                "Markers [1] [2] [3] all stay."
            );
        }
    }
}
