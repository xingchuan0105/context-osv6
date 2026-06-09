//! Mock HTTP servers for Product E2E (LLM, Embedding, Search).

use axum::{
    Json, Router,
    extract::Query,
    response::IntoResponse,
    routing::{get, post},
};
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

static MOCK_RAG_CODEGEN_CHUNK_ID: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static MOCK_RAG_CODEGEN_CHUNK_IDS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
static MOCK_RAG_CODEGEN_QUERY: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static MOCK_RAG_SKIP_CODEGEN: OnceLock<AtomicBool> = OnceLock::new();

fn mock_rag_chunk_id_cell() -> &'static Mutex<Option<String>> {
    MOCK_RAG_CODEGEN_CHUNK_ID.get_or_init(|| Mutex::new(None))
}

fn mock_rag_chunk_ids_cell() -> &'static Mutex<Vec<String>> {
    MOCK_RAG_CODEGEN_CHUNK_IDS.get_or_init(|| Mutex::new(Vec::new()))
}

fn mock_rag_codegen_query_cell() -> &'static Mutex<Option<String>> {
    MOCK_RAG_CODEGEN_QUERY.get_or_init(|| Mutex::new(None))
}

fn mock_rag_skip_codegen_flag() -> &'static AtomicBool {
    MOCK_RAG_SKIP_CODEGEN.get_or_init(|| AtomicBool::new(false))
}

/// Reset per-test mock RAG state (call from TestContext setup).
pub fn reset_mock_rag_state() {
    *mock_rag_chunk_id_cell().lock().unwrap() = None;
    mock_rag_chunk_ids_cell().lock().unwrap().clear();
    *mock_rag_codegen_query_cell().lock().unwrap() = None;
    mock_rag_skip_codegen_flag().store(false, Ordering::SeqCst);
}

/// Pin the chunk id embedded in mock codegen stdout (RAG smoke happy path).
pub fn set_mock_rag_codegen_chunk_id(id: impl Into<String>) {
    let id = id.into();
    *mock_rag_chunk_id_cell().lock().unwrap() = Some(id.clone());
    *mock_rag_chunk_ids_cell().lock().unwrap() = vec![id];
}

/// Pin multiple chunk ids for multi-document mock synthesis.
pub fn set_mock_rag_codegen_chunk_ids(ids: Vec<String>) {
    if let Some(first) = ids.first() {
        *mock_rag_chunk_id_cell().lock().unwrap() = Some(first.clone());
    }
    *mock_rag_chunk_ids_cell().lock().unwrap() = ids;
}

/// Force RAG retrieve rounds to return empty content (exercises auto_fallback).
pub fn set_mock_rag_skip_codegen(skip: bool) {
    mock_rag_skip_codegen_flag().store(skip, Ordering::SeqCst);
}

/// Pin the dense_search query embedded in mock codegen (defaults to `"antifragility"`).
pub fn set_mock_rag_codegen_query(query: impl Into<String>) {
    *mock_rag_codegen_query_cell().lock().unwrap() = Some(query.into());
}

/// Build mock codegen body that exercises the sandbox retrieval bridge.
///
/// The query defaults to `"antifragility"`, which matches the standard smoke fixture
/// `antifragile.txt`. Override via [`set_mock_rag_codegen_query`] when using other fixtures.
pub fn format_mock_rag_codegen_response(_chunk_id: &str) -> String {
    let query = mock_rag_codegen_query_cell()
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| "antifragility".to_string());
    let query_json = serde_json::to_string(&query).unwrap_or_else(|_| "\"antifragility\"".to_string());
    format!(
        r#"<code language="python">
chunks = await client.dense_search(query={query_json}, top_k=10)
import json
print(json.dumps(chunks))
</code>"#
    )
}

/// Names of the canned LLM responses served by [`mock_llm_handler`].
///
/// Tests can pin a call to a specific route by sending the
/// `X-Mock-Route` request header (the production LLM client never
/// sets this header, so production calls always fall through to
/// system-prompt matching).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MockLlmRoute {
    RagAnswer,
    SearchAnswer,
    FormatSkillPpt,
    FormatSkillHtml,
    ChatAnswer,
    Fallback,
}

impl MockLlmRoute {
    /// Return the canned response body for this route.
    fn canned_response(self) -> &'static str {
        match self {
            Self::RagAnswer => {
                "Based on the document, antifragility is a property of systems that increase in capability, resilience, or robustness as a result of stressors, shocks, volatility, noise, mistakes, faults, attacks, or failures. The concept was developed by Nassim Nicholas Taleb."
            }
            Self::SearchAnswer => "The weather in Tokyo today is sunny with a high of 25°C [[1]].",
            Self::FormatSkillPpt => {
                "<html><body><div class=\"slide\"><h1>Slide 1</h1><p>Summary of antifragility</p></div><div class=\"slide\"><h1>Slide 2</h1><p>Key concepts</p></div></body></html>"
            }
            Self::FormatSkillHtml => {
                "<html><body><h1>Antifragility</h1><p>Antifragility is a property of systems that benefit from stress.</p></body></html>"
            }
            Self::ChatAnswer => {
                "Hello! I'm the general chat assistant for Context OS. How can I help you today?"
            }
            Self::Fallback => {
                "This document discusses antifragility, a concept by Nassim Nicholas Taleb describing systems that benefit from shock and disorder."
            }
        }
    }

    /// Resolve a route from the optional `X-Mock-Route` header value.
    /// Returns `None` if the header is missing or has an unknown value.
    pub(crate) fn from_header(value: &str) -> Option<Self> {
        match value.trim() {
            "rag-answer" => Some(Self::RagAnswer),
            "search-answer" => Some(Self::SearchAnswer),
            "format-ppt" => Some(Self::FormatSkillPpt),
            "format-html" => Some(Self::FormatSkillHtml),
            "chat-answer" => Some(Self::ChatAnswer),
            "fallback" => Some(Self::Fallback),
            _ => None,
        }
    }

    /// Resolve a route by inspecting the system prompt text. This is the
    /// fallback path used by the production LLM client (which does NOT
    /// set `X-Mock-Route`).
    ///
    /// ## Order matters
    ///
    /// The format-skill catalog (`- ppt-generation (v1.0): ...`,
    /// `- html-renderer (v1.0): ...`) is appended to **every** RAG
    /// answer-phase system prompt, so the format-skill checks must
    /// come BEFORE the generic RAG answer check. Same logic for the
    /// search answer: the user prompt template always includes a
    /// `Search results:` line, so the search-answer check must be
    /// early enough to not be masked by later fallbacks.
    pub(crate) fn from_system_prompt(system_prompt: &str, user_prompt: &str) -> Self {
        if system_prompt.contains("general chat assistant for Context OS") {
            Self::ChatAnswer
        // Synthesis answer contracts (before format-skill catalog lines).
        } else if system_prompt.contains("Context OS RAG answer agent")
            || system_prompt.contains("internal_answer_v1")
        {
            Self::RagAnswer
        } else if system_prompt.contains("Context OS Web Search answer agent")
            || system_prompt.contains("internal_search_answer_v1")
            || user_prompt.contains("Search results:")
        {
            Self::SearchAnswer
        // Format skills (catalog appears in synthesis system prompts).
        } else if system_prompt.contains("ppt-generation") {
            Self::FormatSkillPpt
        } else if system_prompt.contains("html-renderer") {
            Self::FormatSkillHtml
        } else {
            Self::Fallback
        }
    }
}
pub(crate) async fn start_mock_llm_server() -> (String, tokio::sync::oneshot::Sender<()>) {
    let app = Router::new().route(
        "/chat/completions",
        post(mock_llm_handler).layer(axum::extract::DefaultBodyLimit::max(8 * 1024 * 1024)),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock llm");
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{port}");

    let (abort_tx, abort_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let server = axum::serve(listener, app);
        tokio::select! {
            _ = server => {},
            _ = abort_rx => {},
        }
    });

    (base_url, abort_tx)
}

/// Start a mock Embedding HTTP server on an ephemeral port.
///
/// Returns (base_url, abort_sender, embedding_should_503_flag, call_count).
pub(crate) async fn start_mock_embedding_server() -> (
    String,
    tokio::sync::oneshot::Sender<()>,
    Arc<AtomicBool>,
    Arc<AtomicUsize>,
) {
    let embedding_should_503 = Arc::new(AtomicBool::new(false));
    let embedding_call_count = Arc::new(AtomicUsize::new(0));
    let flag = embedding_should_503.clone();
    let call_count = embedding_call_count.clone();

    let app = Router::new().route(
        "/embeddings",
        post(move |req| mock_embedding_handler(req, flag.clone(), call_count.clone())),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock embedding");
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{port}");

    let (abort_tx, abort_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let server = axum::serve(listener, app);
        tokio::select! {
            _ = server => {},
            _ = abort_rx => {},
        }
    });

    (
        base_url,
        abort_tx,
        embedding_should_503,
        embedding_call_count,
    )
}

fn mock_tool_names(req: &serde_json::Value) -> Vec<String> {
    req.get("tools")
        .and_then(|tools| tools.as_array())
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| {
                    tool.get("function")
                        .and_then(|function| function.get("name"))
                        .and_then(|name| name.as_str())
                        .map(str::to_owned)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn mock_native_tool_call(tool_names: &[String], user_prompt: &str) -> Option<serde_json::Value> {
    let query = user_prompt
        .trim()
        .trim_start_matches("[prior_user_query]")
        .trim()
        .to_string();
    let query = if query.is_empty() {
        "context os query".to_string()
    } else {
        query
    };

    if tool_names.iter().any(|name| name == "web_search") {
        return Some(json!({
            "id": "call_web_search_0",
            "type": "function",
            "function": {
                "name": "web_search",
                "arguments": serde_json::to_string(&json!({
                    "query": query,
                    "vertical": "web",
                })).unwrap_or_else(|_| "{}".to_string()),
            }
        }));
    }

    None
}

fn is_placeholder_chunk_id(id: &str) -> bool {
    let trimmed = id.trim().trim_matches('"');
    if trimmed.is_empty() || trimmed == "null" {
        return true;
    }
    let lower = trimmed.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "..." | "chunk_id" | "uuid" | "mock-chunk-1" | "ch" | "chunk-uuid"
    ) || lower.starts_with('<')
        || trimmed.contains("...")
        || trimmed.contains("CHUNK_ID")
}

fn looks_like_chunk_uuid(id: &str) -> bool {
    let id = id.trim().trim_matches('"');
    let parts: Vec<&str> = id.split('-').collect();
    parts.len() == 5
        && parts[0].len() == 8
        && parts[1].len() == 4
        && parts[2].len() == 4
        && parts[3].len() == 4
        && parts[4].len() == 12
        && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

fn collect_chunk_ids_from_text(text: &str, out: &mut Vec<String>) {
    let mut rest = text;
    while let Some(start) = rest.find("chunk_id") {
        let tail = &rest[start..];
        let after_key = tail.strip_prefix("chunk_id").unwrap_or(tail);
        let after_colon = after_key
            .split_once(':')
            .map(|(_, v)| v)
            .unwrap_or(after_key);
        let trimmed = after_colon.trim().trim_matches('"');
        if !trimmed.is_empty() && trimmed.len() <= 128 {
            let id = trimmed
                .split(|c: char| c == '"' || c == ',' || c == '}' || c.is_whitespace())
                .next()
                .unwrap_or(trimmed);
            if !id.is_empty() && !is_placeholder_chunk_id(id) {
                out.push(id.to_string());
            }
        }
        rest = &rest[start + 8..];
    }
}

fn first_chunk_id_in_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                if let Some(id) = item
                    .get("chunk_id")
                    .and_then(|v| v.as_str())
                    .filter(|id| !is_placeholder_chunk_id(id))
                {
                    return Some(id.to_string());
                }
            }
        }
        serde_json::Value::Object(map) => {
            if let Some(chunks) = map.get("chunks") {
                if let Some(id) = first_chunk_id_in_value(chunks) {
                    return Some(id);
                }
            }
            for v in map.values() {
                if let Some(id) = first_chunk_id_in_value(v) {
                    return Some(id);
                }
            }
        }
        _ => {}
    }
    None
}

fn extract_chunk_id_from_tool_results_block(transcript: &str) -> Option<String> {
    let start = transcript.find("<tool_results>")?;
    let after = &transcript[start + "<tool_results>".len()..];
    let end = after.find("</tool_results>")?;
    let inner = after[..end].trim();
    let parsed = serde_json::from_str::<serde_json::Value>(inner).ok()?;
    if let serde_json::Value::Array(tool_results) = parsed {
        for result in tool_results {
            if let Some(data) = result.get("data") {
                if let Some(id) = first_chunk_id_in_value(data) {
                    return Some(id);
                }
            }
        }
    }
    None
}

fn extract_chunk_id_from_code_execution_block(transcript: &str) -> Option<String> {
    let start = transcript.find("<code_execution_result>")?;
    let after = &transcript[start + "<code_execution_result>".len()..];
    let end = after.find("</code_execution_result>")?;
    let inner = &after[..end];

    for segment in inner.split("[block ") {
        let Some(stdout_part) = segment.split_once("stdout:") else {
            continue;
        };
        let after_stdout = stdout_part.1;
        let stdout = after_stdout
            .split_once("stderr:")
            .map(|(stdout, _)| stdout)
            .unwrap_or(after_stdout)
            .trim();
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(stdout) {
            if let Some(id) = first_chunk_id_in_value(&parsed) {
                return Some(id);
            }
        }
        let mut ids = Vec::new();
        collect_chunk_ids_from_text(segment, &mut ids);
        if let Some(id) = ids.iter().find(|id| looks_like_chunk_uuid(id)) {
            return Some(id.clone());
        }
        if let Some(id) = ids.into_iter().find(|id| !is_placeholder_chunk_id(id)) {
            return Some(id);
        }
    }
    None
}

fn extract_retrieval_chunk_id(transcript: &str) -> Option<String> {
    // Primary path: ReAct codegen observation.
    if let Some(id) = extract_chunk_id_from_code_execution_block(transcript) {
        return Some(id);
    }
    if let Some(id) = extract_chunk_id_from_tool_results_block(transcript) {
        return Some(id);
    }

    // Fallback safety net: server-side auto_fallback observation.
    let fallback_markers = ["自动兜底检索结果:", "自动兜底检索结果"];
    for marker in fallback_markers {
        let Some(idx) = transcript.find(marker) else {
            continue;
        };
        let mut ids = Vec::new();
        collect_chunk_ids_from_text(&transcript[idx..], &mut ids);
        if let Some(id) = ids.iter().find(|id| looks_like_chunk_uuid(id)) {
            return Some(id.clone());
        }
        if let Some(id) = ids.into_iter().find(|id| !is_placeholder_chunk_id(id)) {
            return Some(id);
        }
    }
    None
}

fn collect_unique_chunk_ids_from_transcript(transcript: &str) -> Vec<String> {
    let mut ids = chunk_ids_one_per_doc_from_transcript(transcript);
    if ids.is_empty() {
        collect_chunk_ids_from_text(transcript, &mut ids);
        let mut unique = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for id in ids {
            if seen.insert(id.clone()) {
                unique.push(id);
            }
        }
        ids = unique;
    }
    ids
}

fn chunk_ids_one_per_doc_from_transcript(transcript: &str) -> Vec<String> {
    let Some(start) = transcript.find("<tool_results>") else {
        return Vec::new();
    };
    let after = &transcript[start + "<tool_results>".len()..];
    let Some(end) = after.find("</tool_results>") else {
        return Vec::new();
    };
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(after[..end].trim()) else {
        return Vec::new();
    };
    let serde_json::Value::Array(tool_results) = parsed else {
        return Vec::new();
    };

    let mut by_doc = std::collections::BTreeMap::<String, String>::new();
    for result in tool_results {
        let Some(items) = result.get("data").and_then(|data| data.as_array()) else {
            continue;
        };
        for item in items {
            let (Some(chunk_id), Some(doc_id)) = (
                item.get("chunk_id").and_then(|v| v.as_str()),
                item.get("doc_id").and_then(|v| v.as_str()),
            ) else {
                continue;
            };
            if is_placeholder_chunk_id(chunk_id) || chunk_id.is_empty() || doc_id.is_empty() {
                continue;
            }
            by_doc.entry(doc_id.to_string()).or_insert_with(|| chunk_id.to_string());
        }
    }
    by_doc.into_values().collect()
}

fn mock_synthesis_json_rag(transcript: &str, system_prompt: &str) -> String {
    let chunk_ids = collect_unique_chunk_ids_from_transcript(transcript);
    let mut chunk_ids = if chunk_ids.is_empty() {
        vec![extract_retrieval_chunk_id(transcript)
            .or_else(|| mock_rag_chunk_id_cell().lock().unwrap().clone())
            .unwrap_or_else(|| {
                let preview: String = transcript.chars().take(500).collect();
                panic!(
                    "mock RAG synthesis could not resolve chunk_id; transcript preview: {preview}"
                )
            })]
    } else {
        chunk_ids
    };
    for pinned in mock_rag_chunk_ids_cell().lock().unwrap().iter() {
        if !chunk_ids.contains(pinned) {
            chunk_ids.push(pinned.clone());
        }
    }
    let answer_text = if system_prompt.contains("User prefers format skill: ppt-generation") {
        let cite = chunk_ids
            .first()
            .map(|id| format!(" [[cite:{id}]]"))
            .unwrap_or_default();
        format!(
            "{}{}",
            MockLlmRoute::FormatSkillPpt.canned_response(),
            cite
        )
    } else if system_prompt.contains("User prefers format skill: html-renderer") {
        let cite = chunk_ids
            .first()
            .map(|id| format!(" [[cite:{id}]]"))
            .unwrap_or_default();
        format!(
            "{}{}",
            MockLlmRoute::FormatSkillHtml.canned_response(),
            cite
        )
    } else {
        let cites = chunk_ids
            .iter()
            .map(|id| format!("[[cite:{id}]]"))
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "Based on the document, antifragility is a property of systems that benefit from stress and disorder {cites}. The concept was developed by Nassim Nicholas Taleb."
        )
    };
    let citations: Vec<serde_json::Value> = chunk_ids
        .iter()
        .map(|chunk_id| json!({"chunk_id": chunk_id}))
        .collect();
    serde_json::json!({
        "schema_version": "internal_answer_v1",
        "answer_text": answer_text,
        "citations": citations,
        "coverage": "full",
        "refusal_reason": null
    })
    .to_string()
}

fn mock_synthesis_json_search() -> String {
    serde_json::json!({
        "schema_version": "internal_search_answer_v1",
        "answer_text": "The weather in Tokyo today is sunny with a high of 25°C [[1]].",
        "citations": [{"index": 1}],
        "coverage": "full",
        "refusal_reason": null
    })
    .to_string()
}

fn resolve_mock_content(route: MockLlmRoute, system_prompt: &str, transcript: &str) -> String {
    match route {
        MockLlmRoute::RagAnswer if system_prompt.contains("internal_answer_v1") => {
            mock_synthesis_json_rag(transcript, system_prompt)
        }
        MockLlmRoute::SearchAnswer if system_prompt.contains("internal_search_answer_v1") => {
            mock_synthesis_json_search()
        }
        _ => route.canned_response().to_string(),
    }
}

fn detect_synthesis_route(
    system_prompt: &str,
    _messages: &[serde_json::Value],
) -> Option<MockLlmRoute> {
    // Synthesis contract must win over format-skill catalog lines in the same system prompt.
    if system_prompt.contains("internal_search_answer_v1")
        || system_prompt.contains("Context OS Web Search answer agent")
    {
        return Some(MockLlmRoute::SearchAnswer);
    }
    if system_prompt.contains("internal_answer_v1")
        || system_prompt.contains("Context OS RAG answer agent")
    {
        return Some(MockLlmRoute::RagAnswer);
    }
    if system_prompt.contains("ppt-generation") {
        return Some(MockLlmRoute::FormatSkillPpt);
    }
    if system_prompt.contains("html-renderer") {
        return Some(MockLlmRoute::FormatSkillHtml);
    }
    None
}

fn messages_have_code_execution_result(messages: &[serde_json::Value]) -> bool {
    messages.iter().any(|message| {
        message
            .get("content")
            .and_then(|content| content.as_str())
            .is_some_and(|content| content.contains("<code_execution_result>"))
    })
}

fn mock_rag_codegen_response() -> String {
    format_mock_rag_codegen_response("")
}

async fn mock_llm_handler(
    headers: axum::http::HeaderMap,
    Json(req): Json<serde_json::Value>,
) -> axum::response::Response {
    let messages = req["messages"].as_array().cloned().unwrap_or_default();
    let system_prompt = messages
        .iter()
        .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("system"))
        .and_then(|m| m["content"].as_str())
        .unwrap_or("");
    let user_prompt = messages
        .iter()
        .rev()
        .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
        .and_then(|m| m["content"].as_str())
        .unwrap_or("");

    let is_stream = req.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);
    let tool_names = mock_tool_names(&req);
    let has_tool_results = messages
        .iter()
        .any(|m| m.get("role").and_then(|r| r.as_str()) == Some("tool"));

    // ReAct loop: first tools-enabled turn should emit native tool calls.
    if !is_stream && !tool_names.is_empty() && !has_tool_results {
        if let Some(tool_call) = mock_native_tool_call(&tool_names, user_prompt) {
            return axum::Json(json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "",
                        "tool_calls": [tool_call],
                    }
                }],
                "usage": {"prompt_tokens": 100, "completion_tokens": 1, "total_tokens": 101},
                "model": "mock-llm"
            }))
            .into_response();
        }
    }

    // ReAct loop: after tool results, stop iterating and proceed to synthesis.
    if !is_stream && !tool_names.is_empty() && has_tool_results {
        return axum::Json(json!({
            "choices": [{"message": {"role": "assistant", "content": ""}}],
            "usage": {"prompt_tokens": 50, "completion_tokens": 1, "total_tokens": 51},
            "model": "mock-llm"
        }))
        .into_response();
    }

    let is_synthesis_contract = system_prompt.contains("internal_answer_v1")
        || system_prompt.contains("internal_search_answer_v1");
    // ReAct retrieve without native tools: RAG codegen on first round, then end loop.
    if !is_stream
        && tool_names.is_empty()
        && !is_synthesis_contract
        && system_prompt.contains("检索 → 评估 → 合成")
    {
        if mock_rag_skip_codegen_flag().load(Ordering::SeqCst) {
            return axum::Json(json!({
                "choices": [{"message": {"role": "assistant", "content": ""}}],
                "usage": {"prompt_tokens": 40, "completion_tokens": 1, "total_tokens": 41},
                "model": "mock-llm"
            }))
            .into_response();
        }
        let content = if messages_have_code_execution_result(&messages) {
            String::new()
        } else {
            mock_rag_codegen_response()
        };
        return axum::Json(json!({
            "choices": [{"message": {"role": "assistant", "content": content}}],
            "usage": {"prompt_tokens": 40, "completion_tokens": content.len().max(1), "total_tokens": 41},
            "model": "mock-llm"
        }))
        .into_response();
    }
    if !is_stream
        && tool_names.is_empty()
        && !is_synthesis_contract
        && system_prompt.contains("搜索 → 验证 → 合成")
    {
        return axum::Json(json!({
            "choices": [{"message": {"role": "assistant", "content": ""}}],
            "usage": {"prompt_tokens": 40, "completion_tokens": 1, "total_tokens": 41},
            "model": "mock-llm"
        }))
        .into_response();
    }

    // 1. Header-based routing (explicit, takes priority).
    let route = headers
        .get("x-mock-route")
        .and_then(|v| v.to_str().ok())
        .and_then(MockLlmRoute::from_header)
        .or_else(|| detect_synthesis_route(system_prompt, &messages))
        .unwrap_or_else(|| MockLlmRoute::from_system_prompt(system_prompt, user_prompt));

    let transcript = messages
        .iter()
        .filter_map(|message| message.get("content").and_then(|content| content.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    let content = resolve_mock_content(route, system_prompt, &transcript);
    let emit_mock_reasoning = matches!(
        route,
        MockLlmRoute::RagAnswer
            | MockLlmRoute::SearchAnswer
            | MockLlmRoute::ChatAnswer
            | MockLlmRoute::FormatSkillHtml
            | MockLlmRoute::FormatSkillPpt
    );

    if is_stream {
        // SSE format expected by ChatCompletionStreamParser.
        // Emit 1-char deltas (token-by-token) so `MessageDelta`
        // events fire frequently and the production `complete_stream`
        // path is exercised end-to-end.
        let mut body = String::new();
        if emit_mock_reasoning {
            for ch in "Mock stream reasoning. ".chars() {
                let delta_json = json!({
                    "choices": [{
                        "delta": {"reasoning_content": ch.to_string()},
                        "index": 0
                    }],
                    "model": "mock-llm"
                });
                body.push_str(&format!("data: {delta_json}\n\n"));
            }
        }
        for (i, ch) in content.chars().enumerate() {
            let delta_json = json!({
                "choices": [{
                    "delta": {"content": ch.to_string()},
                    "index": 0
                }],
                "model": "mock-llm"
            });
            body.push_str(&format!("data: {delta_json}\n\n"));
            // Small inter-chunk gap so the client sees multiple chunks
            // (production has variable latency between tokens).
            if i % 8 == 7 {
                body.push_str(": keep-alive\n\n");
            }
        }
        // Final chunk with usage so the parser records it.
        let final_json = json!({
            "choices": [{"delta": {}, "index": 0, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 100, "completion_tokens": content.len(), "total_tokens": 100 + content.len()},
            "model": "mock-llm"
        });
        body.push_str(&format!("data: {final_json}\n\n"));
        body.push_str("data: [DONE]\n\n");

        axum::response::Response::builder()
            .status(200)
            .header("content-type", "text/event-stream")
            .header("cache-control", "no-cache")
            .body(axum::body::Body::from(body))
            .unwrap()
    } else {
        let mut message = json!({"role": "assistant", "content": content});
        if emit_mock_reasoning && !content.is_empty() {
            message["reasoning_content"] = json!("Mock model reasoning for offline E2E.");
        }
        axum::Json(json!({
            "choices": [{"message": message}],
            "usage": {"prompt_tokens": 100, "completion_tokens": content.len(), "total_tokens": 100 + content.len()},
            "model": "mock-llm"
        }))
        .into_response()
    }
}
async fn mock_embedding_handler(
    Json(req): Json<serde_json::Value>,
    embedding_should_503: Arc<AtomicBool>,
    embedding_call_count: Arc<AtomicUsize>,
) -> axum::response::Response {
    embedding_call_count.fetch_add(1, Ordering::SeqCst);

    if embedding_should_503.load(Ordering::SeqCst) {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "embedding service unavailable" })),
        )
            .into_response();
    }

    let texts = req["input"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();
    let dim = req["dimensions"].as_u64().unwrap_or(1024) as usize;
    // All vectors identical so dense retrieval always returns high similarity.
    let vec: Vec<f32> = (0..dim).map(|j| 0.1_f32 + (j % 10) as f32 * 0.01).collect();
    let data: Vec<serde_json::Value> = texts.iter().map(|_| json!({"embedding": vec})).collect();

    Json(json!({ "data": data, "model": "mock-embedding" })).into_response()
}

/// Start a mock Brave Search HTTP server on an ephemeral port.
///
/// Returns (base_url, abort_sender, search_should_429_flag).
pub(crate) async fn start_mock_search_server()
-> (String, tokio::sync::oneshot::Sender<()>, Arc<AtomicBool>) {
    let search_should_429 = Arc::new(AtomicBool::new(false));
    let flag = search_should_429.clone();

    let flag2 = flag.clone();
    let flag3 = flag.clone();
    let app = Router::new()
        .route(
            "/res/v1/llm/context",
            post(move |req| mock_search_handler(req, flag.clone())),
        )
        .route(
            "/res/v1/news/search",
            get(move |Query(params): Query<MockNewsQuery>| async move {
                mock_news_search_handler(params, flag2.clone())
            })
            .post(move |req| mock_search_handler(req, flag3.clone())),
        );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock search");
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{port}");

    let (abort_tx, abort_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let server = axum::serve(listener, app);
        tokio::select! {
            _ = server => {},
            _ = abort_rx => {},
        }
    });

    (base_url, abort_tx, search_should_429)
}

#[derive(Debug, serde::Deserialize)]
struct MockNewsQuery {
    q: Option<String>,
}

fn mock_news_search_handler(
    params: MockNewsQuery,
    search_should_429: Arc<AtomicBool>,
) -> axum::response::Response {
    if search_should_429.load(Ordering::SeqCst) {
        return (
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(json!({ "error": "rate limit exceeded" })),
        )
            .into_response();
    }

    let _query = params.q.as_deref().unwrap_or("unknown");
    Json(json!({
        "results": [
            {
                "title": "Tokyo Weather Today",
                "url": "https://example.com/weather-tokyo",
                "description": "Sunny with a high of 25°C in Tokyo today."
            }
        ]
    }))
    .into_response()
}

async fn mock_search_handler(
    Json(req): Json<serde_json::Value>,
    search_should_429: Arc<AtomicBool>,
) -> axum::response::Response {
    if search_should_429.load(Ordering::SeqCst) {
        return (
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(json!({ "error": "rate limit exceeded" })),
        )
            .into_response();
    }

    let _query = req["q"].as_str().unwrap_or("unknown");
    Json(json!({
        "grounding": {
            "generic": [
                {
                    "url": "https://example.com/weather-tokyo",
                    "title": "Tokyo Weather Today",
                    "snippets": ["Sunny with a high of 25°C in Tokyo today."]
                }
            ],
            "map": []
        },
        "sources": {
            "https://example.com/weather-tokyo": {
                "title": "Tokyo Weather Today",
                "hostname": "example.com"
            }
        }
    }))
    .into_response()
}
