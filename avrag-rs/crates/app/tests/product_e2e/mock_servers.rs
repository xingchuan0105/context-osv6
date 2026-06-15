//! Mock HTTP servers for Product E2E (LLM, Embedding, Search, Paddle OCR).

pub use super::mock_rag_state::{
    reset_mock_rag_state, set_mock_emit_memory_tool, set_mock_rag_codegen_chunk_id,
    set_mock_rag_codegen_chunk_ids, set_mock_rag_codegen_doc_id, set_mock_rag_codegen_query,
    set_mock_rag_multiround_profile, set_mock_rag_skill_request_memory, set_mock_rag_skip_codegen,
};

use super::mock_rag_state::read_mock_rag_state;

use super::persistent_runtime::{bind_persistent_listener, spawn_persistent};
use axum::{
    Json, Router,
    extract::{Multipart, Path, Query},
    response::IntoResponse,
    routing::{get, post},
};
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

fn latest_user_message_content(messages: &[serde_json::Value]) -> Option<&str> {
    messages
        .iter()
        .rev()
        .find(|message| message.get("role").and_then(|role| role.as_str()) == Some("user"))
        .and_then(|message| message.get("content").and_then(|content| content.as_str()))
}

fn dense_search_query_from_messages(messages: &[serde_json::Value]) -> Option<String> {
    let content = latest_user_message_content(messages)?;
    let query = content.trim().trim_start_matches("[prior_user_query]").trim();
    if query.is_empty() {
        None
    } else {
        Some(query.to_string())
    }
}

fn resolve_dense_search_query(messages: &[serde_json::Value]) -> String {
    dense_search_query_from_messages(messages)
        .or_else(|| read_mock_rag_state(|state| state.codegen_query.clone()))
        .unwrap_or_else(|| "antifragility".to_string())
}

fn format_mock_rag_codegen_response_for_query(query: &str) -> String {
    let query_json =
        serde_json::to_string(query).unwrap_or_else(|_| "\"antifragility\"".to_string());
    format!(
        r#"<code language="python">
chunks = await client.dense_search(query={query_json}, top_k=10)
import json
print(json.dumps(chunks))
</code>"#
    )
}

/// Build mock codegen body that exercises the sandbox retrieval bridge.
///
/// The query defaults to `"antifragility"`, which matches the standard smoke fixture
/// `antifragile.txt`. Override via [`set_mock_rag_codegen_query`] when using other fixtures.
pub fn format_mock_rag_codegen_response(_chunk_id: &str) -> String {
    format_mock_rag_codegen_response_for_query(
        &read_mock_rag_state(|state| {
            state
                .codegen_query
                .clone()
                .unwrap_or_else(|| "antifragility".to_string())
        }),
    )
}

/// Round0 multiround codegen: fetch document profile (sections + metadata).
pub fn format_mock_rag_doc_profile_codegen(doc_id: &str) -> String {
    let doc_id_json = serde_json::to_string(doc_id).unwrap_or_else(|_| "\"doc\"".to_string());
    format!(
        r#"<code language="python">
profile = await client.doc_profile(doc_ids=[{doc_id_json}])
import json
print(json.dumps(profile))
</code>"#
    )
}

/// Round1 multiround codegen: fetch chunk body by id (`chunk_fetch` → `index_lookup`).
pub fn format_mock_rag_chunk_fetch_codegen(chunk_id: &str) -> String {
    let chunk_json = serde_json::to_string(chunk_id).unwrap_or_else(|_| "\"chunk\"".to_string());
    format!(
        r#"<code language="python">
chunks = await client.chunk_fetch(chunk_id={chunk_json})
import json
print(json.dumps(chunks))
</code>"#
    )
}

fn count_code_execution_results(messages: &[serde_json::Value]) -> usize {
    messages
        .iter()
        .filter(|message| {
            message
                .get("content")
                .and_then(|content| content.as_str())
                .is_some_and(|content| content.contains("<code_execution_result>"))
        })
        .count()
}

fn mock_memory_tool_call(tool: &str) -> Option<serde_json::Value> {
    let (id, arguments) = match tool {
        "conversation_history_load" => (
            "call_mem_history_0",
            serde_json::to_string(&json!({"query": "antifragility", "scope": "notebook", "limit": 20}))
                .unwrap_or_else(|_| "{}".to_string()),
        ),
        "user_profile_load" => ("call_mem_profile_0", "{}".to_string()),
        _ => return None,
    };
    Some(json!({
        "id": id,
        "type": "function",
        "function": {
            "name": tool,
            "arguments": arguments,
        }
    }))
}

fn mock_rag_retrieve_codegen_content(messages: &[serde_json::Value]) -> String {
    if read_mock_rag_state(|state| state.skip_codegen) {
        return String::new();
    }
    if read_mock_rag_state(|state| state.multiround_profile) {
        let rounds = count_code_execution_results(messages);
        return match rounds {
            0 => {
                let doc_id = read_mock_rag_state(|state| {
                    state
                        .codegen_doc_id
                        .clone()
                        .unwrap_or_else(|| "00000000-0000-4000-8000-000000000001".to_string())
                });
                format_mock_rag_doc_profile_codegen(&doc_id)
            }
            1 => {
                let chunk_id = read_mock_rag_state(|state| {
                    state
                        .codegen_chunk_id
                        .clone()
                        .unwrap_or_else(|| "00000000-0000-4000-8000-000000000001".to_string())
                });
                format_mock_rag_chunk_fetch_codegen(&chunk_id)
            }
            _ => String::new(),
        };
    }
    if messages_have_code_execution_result(messages) {
        String::new()
    } else {
        format_mock_rag_codegen_response_for_query(&resolve_dense_search_query(messages))
    }
}

fn try_memory_tool_response(tool_names: &[String], has_tool_results: bool) -> Option<axum::response::Response> {
    if has_tool_results {
        return None;
    }
    let requested = read_mock_rag_state(|state| state.emit_memory_tool.clone())?;
    if !tool_names.iter().any(|name| name == &requested) {
        return None;
    }
    let tool_call = mock_memory_tool_call(&requested)?;
    Some(
        axum::Json(json!({
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
        .into_response(),
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

    let (listener, base_url) = bind_persistent_listener().await;

    let (abort_tx, abort_rx) = tokio::sync::oneshot::channel::<()>();
    spawn_persistent(async move {
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

    let flag_mm = embedding_should_503.clone();
    let call_count_mm = embedding_call_count.clone();
    let app = Router::new()
        .route(
            "/embeddings",
            post(move |req| mock_embedding_handler(req, flag.clone(), call_count.clone())),
        )
        .fallback(post(move |req| {
            mock_dashscope_multimodal_embedding_handler(req, flag_mm.clone(), call_count_mm.clone())
        }));

    let (listener, base_url) = bind_persistent_listener().await;

    let (abort_tx, abort_rx) = tokio::sync::oneshot::channel::<()>();
    spawn_persistent(async move {
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
            .or_else(|| read_mock_rag_state(|state| state.codegen_chunk_id.clone()))
            .unwrap_or_else(|| {
                let preview: String = transcript.chars().take(500).collect();
                panic!(
                    "mock RAG synthesis could not resolve chunk_id; transcript preview: {preview}"
                )
            })]
    } else {
        chunk_ids
    };
    for pinned in read_mock_rag_state(|state| state.codegen_chunk_ids.clone()).iter() {
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

fn messages_have_memory_skill_request(messages: &[serde_json::Value]) -> bool {
    messages.iter().any(|message| {
        message.get("role").and_then(|r| r.as_str()) == Some("assistant")
            && message
                .get("content")
                .and_then(|c| c.as_str())
                .is_some_and(|content| {
                    content.contains("\"skill_request\"") && content.contains("\"memory\"")
                })
    })
}

fn messages_have_code_execution_result(messages: &[serde_json::Value]) -> bool {
    messages.iter().any(|message| {
        message
            .get("content")
            .and_then(|content| content.as_str())
            .is_some_and(|content| content.contains("<code_execution_result>"))
    })
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

    let is_synthesis_contract = system_prompt.contains("internal_answer_v1")
        || system_prompt.contains("internal_search_answer_v1");
    let is_rag_retrieve = !is_synthesis_contract && system_prompt.contains("检索 → 评估 → 合成");

    // ReAct loop: after tool results, stop iterating and proceed to synthesis.
    if !is_stream && !tool_names.is_empty() && has_tool_results {
        return axum::Json(json!({
            "choices": [{"message": {"role": "assistant", "content": ""}}],
            "usage": {"prompt_tokens": 50, "completion_tokens": 1, "total_tokens": 51},
            "model": "mock-llm"
        }))
        .into_response();
    }

    // Memory tool injection (explicit toggle) takes priority over codegen on retrieve rounds.
    if !is_stream && !tool_names.is_empty() {
        if let Some(response) = try_memory_tool_response(&tool_names, has_tool_results) {
            return response;
        }
    }

    // On-demand memory cluster disclosure (smoke helper): one-shot skill_request before tools attach.
    if !is_stream
        && is_rag_retrieve
        && read_mock_rag_state(|state| state.skill_request_memory)
        && !messages_have_memory_skill_request(&messages)
    {
        return axum::Json(json!({
            "choices": [{"message": {"role": "assistant", "content": r#"{"skill_request":["memory"]}"#}}],
            "usage": {"prompt_tokens": 40, "completion_tokens": 8, "total_tokens": 48},
            "model": "mock-llm"
        }))
        .into_response();
    }

    // RAG retrieve: codegen wins over generic native-tool routing even when tool_pool is non-empty.
    if !is_stream && is_rag_retrieve {
        let content = mock_rag_retrieve_codegen_content(&messages);
        return axum::Json(json!({
            "choices": [{"message": {"role": "assistant", "content": content}}],
            "usage": {"prompt_tokens": 40, "completion_tokens": content.len().max(1), "total_tokens": 41},
            "model": "mock-llm"
        }))
        .into_response();
    }

    // ReAct loop: first tools-enabled turn should emit native tool calls (search, etc.).
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
async fn mock_dashscope_multimodal_embedding_handler(
    Json(req): Json<serde_json::Value>,
    embedding_should_503: Arc<AtomicBool>,
    embedding_call_count: Arc<AtomicUsize>,
) -> axum::response::Response {
    embedding_call_count.fetch_add(1, Ordering::SeqCst);

    if embedding_should_503.load(Ordering::SeqCst) {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "code": "ServiceUnavailable", "message": "embedding service unavailable" })),
        )
            .into_response();
    }

    let dim = req["parameters"]["dimension"]
        .as_u64()
        .or_else(|| req["parameters"]["dimensions"].as_u64())
        .unwrap_or(1024) as usize;
    let fused = req["parameters"]["enable_fusion"].as_bool().unwrap_or(false);
    let contents_len = req["input"]["contents"]
        .as_array()
        .map(|arr| arr.len())
        .unwrap_or(1)
        .max(1);
    let embedding_type = if fused || contents_len > 1 {
        "fusion"
    } else {
        "text"
    };
    // Stable vector so multimodal dense retrieval always matches indexed chunks.
    let embedding: Vec<f32> = (0..dim)
        .map(|j| 0.1_f32 + (j % 10) as f32 * 0.01)
        .collect();

    Json(json!({
        "output": {
            "embeddings": [{
                "index": 0,
                "embedding": embedding,
                "type": embedding_type
            }]
        },
        "usage": {
            "input_tokens": 10,
            "input_tokens_details": {
                "image_tokens": 0,
                "text_tokens": 10
            },
            "output_tokens": 1,
            "total_tokens": 11
        }
    }))
    .into_response()
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

/// Runtime toggles for the mock Brave Search server.
#[derive(Clone)]
pub(crate) struct MockSearchControls {
    pub should_429: Arc<AtomicBool>,
    pub should_empty: Arc<AtomicBool>,
    pub delay_ms: Arc<AtomicU64>,
}

impl MockSearchControls {
    pub fn new() -> Self {
        Self {
            should_429: Arc::new(AtomicBool::new(false)),
            should_empty: Arc::new(AtomicBool::new(false)),
            delay_ms: Arc::new(AtomicU64::new(0)),
        }
    }
}

/// Start a mock Brave Search HTTP server on an ephemeral port.
///
/// Returns (base_url, abort_sender, controls).
pub(crate) async fn start_mock_search_server() -> (
    String,
    tokio::sync::oneshot::Sender<()>,
    MockSearchControls,
) {
    let controls = MockSearchControls::new();
    let flag = controls.clone();
    let flag2 = controls.clone();
    let flag3 = controls.clone();
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

    let (listener, base_url) = bind_persistent_listener().await;

    let (abort_tx, abort_rx) = tokio::sync::oneshot::channel::<()>();
    spawn_persistent(async move {
        let server = axum::serve(listener, app);
        tokio::select! {
            _ = server => {},
            _ = abort_rx => {},
        }
    });

    (base_url, abort_tx, controls)
}

#[derive(Debug, serde::Deserialize)]
struct MockNewsQuery {
    q: Option<String>,
}

fn mock_news_search_handler(
    params: MockNewsQuery,
    controls: MockSearchControls,
) -> axum::response::Response {
    if controls.should_429.load(Ordering::SeqCst) {
        return (
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(json!({ "error": "rate limit exceeded" })),
        )
            .into_response();
    }

    if controls.should_empty.load(Ordering::SeqCst) {
        return Json(json!({ "results": [] })).into_response();
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
    controls: MockSearchControls,
) -> axum::response::Response {
    if controls.should_429.load(Ordering::SeqCst) {
        return (
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(json!({ "error": "rate limit exceeded" })),
        )
            .into_response();
    }

    let delay_ms = controls.delay_ms.load(Ordering::SeqCst);
    if delay_ms > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
    }

    if controls.should_empty.load(Ordering::SeqCst) {
        return Json(json!({
            "grounding": { "generic": [], "map": [] },
            "sources": {}
        }))
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

/// OCR text returned by the mock Paddle Jobs API for image ingest contract tests.
pub(crate) const MOCK_PADDLE_IMAGE_OCR_TEXT: &str =
    "Paddle image contract OCR text for product E2E.";

/// JSONL body mimicking Paddle AI Studio layout parsing output (searchable text).
pub(crate) fn mock_paddle_ocr_result_jsonl() -> String {
    format!(
        r#"{{"layoutParsingResults":[{{"markdown":{{"text":"{text}","images":{{}}}}}}]}}"#,
        text = MOCK_PADDLE_IMAGE_OCR_TEXT
    )
}

async fn mock_paddle_submit_job(
    mut multipart: Multipart,
    jobs_submitted: Arc<AtomicUsize>,
    base_url: Arc<String>,
) -> axum::response::Response {
    while let Ok(Some(field)) = multipart.next_field().await {
        let _ = field.bytes().await;
    }
    let job_id = format!("mock-paddle-{}", jobs_submitted.fetch_add(1, Ordering::SeqCst) + 1);
    let json_url = format!("{base_url}/results/{job_id}");
    Json(json!({
        "data": {
            "jobId": job_id,
            "resultUrl": { "jsonUrl": json_url }
        }
    }))
    .into_response()
}

async fn mock_paddle_poll_job(
    Path(job_id): Path<String>,
    base_url: Arc<String>,
) -> axum::response::Response {
    Json(json!({
        "data": {
            "state": "done",
            "resultUrl": {
                "jsonUrl": format!("{base_url}/results/{job_id}")
            }
        }
    }))
    .into_response()
}

async fn mock_paddle_result_json(Path(job_id): Path<String>) -> axum::response::Response {
    let _ = job_id;
    mock_paddle_ocr_result_jsonl().into_response()
}

/// Start a mock Paddle OCR Jobs HTTP server (submit → poll → jsonUrl fetch).
///
/// Returns (base_url, abort_sender, jobs_submitted_counter).
pub(crate) async fn start_mock_paddle_ocr_server() -> (
    String,
    tokio::sync::oneshot::Sender<()>,
    Arc<AtomicUsize>,
) {
    let jobs_submitted = Arc::new(AtomicUsize::new(0));
    let (listener, base_url) = bind_persistent_listener().await;
    let base_url = Arc::new(base_url);
    let submit_base = base_url.clone();
    let poll_base = base_url.clone();
    let jobs_for_submit = jobs_submitted.clone();

    let app = Router::new()
        .route(
            "/jobs",
            post(move |multipart| {
                mock_paddle_submit_job(multipart, jobs_for_submit.clone(), submit_base.clone())
            }),
        )
        .route(
            "/jobs/{job_id}",
            get(move |path| mock_paddle_poll_job(path, poll_base.clone())),
        )
        .route("/results/{job_id}", get(mock_paddle_result_json));

    let (abort_tx, abort_rx) = tokio::sync::oneshot::channel::<()>();
    spawn_persistent(async move {
        let server = axum::serve(listener, app);
        tokio::select! {
            _ = server => {},
            _ = abort_rx => {},
        }
    });

    (
        Arc::try_unwrap(base_url).unwrap_or_else(|arc| (*arc).clone()),
        abort_tx,
        jobs_submitted,
    )
}

/// Fixed cell text returned by the mock Office Parser xlsx endpoint.
pub(crate) const MOCK_OFFICE_XLSX_TEXT: &str = "Revenue Q1 42";

/// Fixed paragraph text returned by the mock Office Parser docx endpoint.
pub(crate) const MOCK_OFFICE_DOCX_TEXT: &str = "Phase0 mini docx ingest probe";

/// Fixed slide text returned by the mock Office Parser pptx endpoint.
pub(crate) const MOCK_OFFICE_PPTX_TEXT: &str = "Phase0 mini pptx ingest probe";

fn mock_office_xlsx_document_ir(document_id: &str) -> ingestion::ir::DocumentIr {
    use ingestion::ir::{
        BlockIr, BlockModality, BlockType, DocumentIr, DocumentType, PageIr, ParseBackend,
        SourceLocator,
    };
    DocumentIr {
        document_id: document_id.to_string(),
        title: "contract-xlsx.xlsx".to_string(),
        doc_type: DocumentType::Xlsx,
        primary_backend: ParseBackend::PoiXlsx,
        backend_version: Some("mock-office-parser".to_string()),
        language: Some("en".to_string()),
        metadata: Default::default(),
        pages: vec![PageIr {
            page_number: 1,
            text_char_count: MOCK_OFFICE_XLSX_TEXT.len(),
            image_count: 0,
            ..Default::default()
        }],
        blocks: vec![BlockIr {
            block_id: "sheet-a1".to_string(),
            page: Some(1),
            block_type: BlockType::SheetCellRange,
            modality: BlockModality::TextOnly,
            text: MOCK_OFFICE_XLSX_TEXT.to_string(),
            alt_text: None,
            asset_refs: vec![],
            caption: None,
            section_path: vec![],
            source_locator: SourceLocator {
                page: Some(1),
                ..Default::default()
            },
            parser_backend: ParseBackend::PoiXlsx,
            metadata: Default::default(),
        }],
        assets: vec![],
        warnings: vec![],
    }
}

async fn mock_office_parse_xlsx(mut multipart: Multipart) -> axum::response::Response {
    let mut document_id = "mock-doc".to_string();
    while let Ok(Some(part)) = multipart.next_field().await {
        match part.name().unwrap_or_default() {
            "document_id" => {
                if let Ok(text) = part.text().await {
                    if !text.trim().is_empty() {
                        document_id = text;
                    }
                }
            }
            _ => {}
        }
    }
    let body = ingestion::parser::OfficeParserParseResponse {
        document_ir: mock_office_xlsx_document_ir(&document_id),
        warnings: vec![],
        stats: ingestion::parser::OfficeParserParseStats {
            duration_ms: 1,
            block_count: 1,
            asset_count: 0,
        },
    };
    Json(body).into_response()
}

fn mock_office_docx_document_ir(document_id: &str) -> ingestion::ir::DocumentIr {
    use ingestion::ir::{
        BlockIr, BlockModality, BlockType, DocumentIr, DocumentType, PageIr, ParseBackend,
        SourceLocator,
    };
    DocumentIr {
        document_id: document_id.to_string(),
        title: "phase0-mini.docx".to_string(),
        doc_type: DocumentType::Docx,
        primary_backend: ParseBackend::Docx4jDocx,
        backend_version: Some("mock-office-parser".to_string()),
        language: Some("en".to_string()),
        metadata: Default::default(),
        pages: vec![PageIr {
            page_number: 1,
            text_char_count: MOCK_OFFICE_DOCX_TEXT.len(),
            image_count: 0,
            ..Default::default()
        }],
        blocks: vec![BlockIr {
            block_id: "docx-block-1".to_string(),
            page: Some(1),
            block_type: BlockType::Paragraph,
            modality: BlockModality::TextOnly,
            text: MOCK_OFFICE_DOCX_TEXT.to_string(),
            alt_text: None,
            asset_refs: vec![],
            caption: None,
            section_path: vec![],
            source_locator: SourceLocator {
                page: Some(1),
                ..Default::default()
            },
            parser_backend: ParseBackend::Docx4jDocx,
            metadata: Default::default(),
        }],
        assets: vec![],
        warnings: vec![],
    }
}

async fn mock_office_parse_docx(mut multipart: Multipart) -> axum::response::Response {
    let mut document_id = "mock-doc".to_string();
    while let Ok(Some(part)) = multipart.next_field().await {
        match part.name().unwrap_or_default() {
            "document_id" => {
                if let Ok(text) = part.text().await {
                    if !text.trim().is_empty() {
                        document_id = text;
                    }
                }
            }
            _ => {}
        }
    }
    let body = ingestion::parser::OfficeParserParseResponse {
        document_ir: mock_office_docx_document_ir(&document_id),
        warnings: vec![],
        stats: ingestion::parser::OfficeParserParseStats {
            duration_ms: 1,
            block_count: 1,
            asset_count: 0,
        },
    };
    Json(body).into_response()
}

fn mock_office_pptx_document_ir(document_id: &str) -> ingestion::ir::DocumentIr {
    use ingestion::ir::{
        AssetIr, AssetKind, BlockIr, BlockModality, BlockType, DocumentIr, DocumentType, PageIr,
        ParseBackend, SourceLocator,
    };
    DocumentIr {
        document_id: document_id.to_string(),
        title: "phase0-mini.pptx".to_string(),
        doc_type: DocumentType::Pptx,
        primary_backend: ParseBackend::PoiPptx,
        backend_version: Some("mock-office-parser".to_string()),
        language: Some("en".to_string()),
        metadata: Default::default(),
        pages: vec![PageIr {
            page_number: 1,
            text_char_count: MOCK_OFFICE_PPTX_TEXT.len(),
            image_count: 1,
            backend: ParseBackend::PoiPptx,
            ..Default::default()
        }],
        blocks: vec![
            BlockIr {
                block_id: "slide-1-text".to_string(),
                page: Some(1),
                block_type: BlockType::SlideText,
                modality: BlockModality::TextOnly,
                text: MOCK_OFFICE_PPTX_TEXT.to_string(),
                alt_text: None,
                asset_refs: vec![],
                caption: None,
                section_path: vec![],
                source_locator: SourceLocator {
                    page: Some(1),
                    slide_index: Some(1),
                    ..Default::default()
                },
                parser_backend: ParseBackend::PoiPptx,
                metadata: Default::default(),
            },
            BlockIr {
                block_id: "slide-1-image".to_string(),
                page: Some(1),
                block_type: BlockType::SlideImage,
                modality: BlockModality::ImageWithContext,
                text: "Phase0 mini slide".to_string(),
                alt_text: Some("Phase0 mini slide render".to_string()),
                asset_refs: vec!["slide-render-1".to_string()],
                caption: Some("Phase0 mini slide".to_string()),
                section_path: vec![],
                source_locator: SourceLocator {
                    page: Some(1),
                    slide_index: Some(1),
                    ..Default::default()
                },
                parser_backend: ParseBackend::PoiPptx,
                metadata: Default::default(),
            },
        ],
        assets: vec![AssetIr {
            asset_id: "slide-render-1".to_string(),
            page: Some(1),
            asset_kind: AssetKind::SlideRender,
            storage_path: "mock-assets/slide-1.png".to_string(),
            mime_type: Some("image/png".to_string()),
            width: Some(1280),
            height: Some(720),
            parser_backend: ParseBackend::PoiPptx,
            metadata: Default::default(),
        }],
        warnings: vec![],
    }
}

async fn mock_office_parse_pptx(mut multipart: Multipart) -> axum::response::Response {
    let mut document_id = "mock-doc".to_string();
    while let Ok(Some(part)) = multipart.next_field().await {
        match part.name().unwrap_or_default() {
            "document_id" => {
                if let Ok(text) = part.text().await {
                    if !text.trim().is_empty() {
                        document_id = text;
                    }
                }
            }
            _ => {}
        }
    }
    let body = ingestion::parser::OfficeParserParseResponse {
        document_ir: mock_office_pptx_document_ir(&document_id),
        warnings: vec![],
        stats: ingestion::parser::OfficeParserParseStats {
            duration_ms: 1,
            block_count: 2,
            asset_count: 1,
        },
    };
    Json(body).into_response()
}

async fn mock_office_healthz() -> axum::response::Response {
    Json(ingestion::parser::OfficeParserHealthz {
        ok: true,
        service: "mock-office-parser".to_string(),
    })
    .into_response()
}

/// Start a mock Office Parser HTTP server (`/v1/parse/{docx,pptx,xlsx}`, `/v1/healthz`).
pub(crate) async fn start_mock_office_parser_server() -> (
    String,
    tokio::sync::oneshot::Sender<()>,
) {
    let (listener, base_url) = bind_persistent_listener().await;
    let app = Router::new()
        .route("/v1/healthz", get(mock_office_healthz))
        .route("/v1/parse/docx", post(mock_office_parse_docx))
        .route("/v1/parse/pptx", post(mock_office_parse_pptx))
        .route("/v1/parse/xlsx", post(mock_office_parse_xlsx));
    let (abort_tx, abort_rx) = tokio::sync::oneshot::channel::<()>();
    spawn_persistent(async move {
        let server = axum::serve(listener, app);
        tokio::select! {
            _ = server => {},
            _ = abort_rx => {},
        }
    });
    (base_url, abort_tx)
}
